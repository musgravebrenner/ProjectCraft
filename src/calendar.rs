//! Working-time calendars and the time arithmetic the scheduler uses.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

/// A working shift as minutes after midnight, `[start, end)`. `end` may be 1440.
pub type Shift = (u32, u32);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CalException {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// Empty = nonworking for those days.
    #[serde(default)]
    pub shifts: Vec<Shift>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Calendar {
    pub name: String,
    /// Shifts for each weekday, Sunday first (as the original stores its pattern).
    pub week: [Vec<Shift>; 7],
    #[serde(default)]
    pub exceptions: Vec<CalException>,
}

const MAX_DAYS: i32 = 366 * 30;

pub fn hm(h: u32, m: u32) -> u32 {
    h * 60 + m
}

impl Calendar {
    /// Standard: Mon–Fri 08:00–12:00, 13:00–17:00 (pattern 0111110 in the original's resources).
    pub fn standard() -> Calendar {
        let day = vec![(hm(8, 0), hm(12, 0)), (hm(13, 0), hm(17, 0))];
        Calendar {
            name: "Standard".into(),
            week: [vec![], day.clone(), day.clone(), day.clone(), day.clone(), day, vec![]],
            exceptions: vec![],
        }
    }
    pub fn twenty_four_hours() -> Calendar {
        let day = vec![(0, 1440)];
        Calendar {
            name: "24 Hours".into(),
            week: [day.clone(), day.clone(), day.clone(), day.clone(), day.clone(), day.clone(), day],
            exceptions: vec![],
        }
    }
    pub fn night_shift() -> Calendar {
        let mid = vec![(0, hm(3, 0)), (hm(4, 0), hm(8, 0)), (hm(23, 0), 1440)];
        Calendar {
            name: "Night Shift".into(),
            week: [
                vec![],
                vec![(hm(23, 0), 1440)],
                mid.clone(),
                mid.clone(),
                mid.clone(),
                mid,
                vec![(0, hm(3, 0)), (hm(4, 0), hm(8, 0))],
            ],
            exceptions: vec![],
        }
    }
    pub fn base_calendars() -> Vec<Calendar> {
        vec![Calendar::standard(), Calendar::twenty_four_hours(), Calendar::night_shift()]
    }

    pub fn shifts_on(&self, d: NaiveDate) -> &[Shift] {
        for e in self.exceptions.iter().rev() {
            if d >= e.start && d <= e.end {
                return &e.shifts;
            }
        }
        &self.week[d.weekday().num_days_from_sunday() as usize]
    }
    pub fn is_working_day(&self, d: NaiveDate) -> bool {
        !self.shifts_on(d).is_empty()
    }
    pub fn working_minutes_on(&self, d: NaiveDate) -> i64 {
        self.shifts_on(d).iter().map(|(s, e)| (*e as i64) - (*s as i64)).sum()
    }
    fn has_any_work(&self) -> bool {
        self.week.iter().any(|d| !d.is_empty())
    }

    /// Earliest working moment at or after `t`.
    pub fn snap_forward(&self, t: NaiveDateTime) -> NaiveDateTime {
        if !self.has_any_work() {
            return t;
        }
        let mut d = t.date();
        let mut m = minute_of(t);
        for _ in 0..MAX_DAYS {
            for &(s, e) in self.shifts_on(d) {
                if m < e {
                    return at(d, s.max(m));
                }
            }
            d = d.succ_opt().unwrap();
            m = 0;
        }
        t
    }
    /// Latest working moment at or before `t` (a finish moment: may be a shift end).
    pub fn snap_backward(&self, t: NaiveDateTime) -> NaiveDateTime {
        if !self.has_any_work() {
            return t;
        }
        let (mut d, mut m) = day_min_end(t);
        for _ in 0..MAX_DAYS {
            for &(s, e) in self.shifts_on(d).iter().rev() {
                if m > s {
                    return at(d, e.min(m));
                }
            }
            d = d.pred_opt().unwrap();
            m = 1440;
        }
        t
    }
    /// Moment reached after `minutes` of working time from `t` (negative goes back).
    pub fn add_work(&self, t: NaiveDateTime, minutes: i64) -> NaiveDateTime {
        if minutes < 0 {
            return self.sub_work(t, -minutes);
        }
        if minutes == 0 {
            return self.snap_forward(t);
        }
        if !self.has_any_work() {
            return t + Duration::minutes(minutes);
        }
        let mut d = t.date();
        let mut m = minute_of(t);
        let mut rem = minutes;
        for _ in 0..MAX_DAYS {
            for &(s, e) in self.shifts_on(d) {
                if m >= e {
                    continue;
                }
                let st = s.max(m);
                let avail = (e - st) as i64;
                if rem <= avail {
                    return at(d, st + rem as u32);
                }
                rem -= avail;
                m = e;
            }
            d = d.succ_opt().unwrap();
            m = 0;
        }
        t + Duration::minutes(minutes)
    }
    /// Moment `minutes` of working time before `t`.
    pub fn sub_work(&self, t: NaiveDateTime, minutes: i64) -> NaiveDateTime {
        if minutes < 0 {
            return self.add_work(t, -minutes);
        }
        if minutes == 0 {
            return self.snap_backward(t);
        }
        if !self.has_any_work() {
            return t - Duration::minutes(minutes);
        }
        let (mut d, mut m) = day_min_end(t);
        let mut rem = minutes;
        for _ in 0..MAX_DAYS {
            for &(s, e) in self.shifts_on(d).iter().rev() {
                if m <= s {
                    continue;
                }
                let en = e.min(m);
                let avail = (en - s) as i64;
                if rem <= avail {
                    return at(d, en - rem as u32);
                }
                rem -= avail;
                m = s;
            }
            d = d.pred_opt().unwrap();
            m = 1440;
        }
        t - Duration::minutes(minutes)
    }
    /// Working minutes between `a` and `b` (negative when b < a).
    pub fn work_between(&self, a: NaiveDateTime, b: NaiveDateTime) -> i64 {
        if b < a {
            return -self.work_between(b, a);
        }
        if !self.has_any_work() {
            return (b - a).num_minutes();
        }
        let mut total = 0i64;
        let mut d = a.date();
        let end_d = b.date();
        let mut guard = 0;
        while d <= end_d && guard < MAX_DAYS {
            let lo = if d == a.date() { minute_of(a) } else { 0 };
            let hi = if d == end_d { minute_of(b) } else { 1440 };
            for &(s, e) in self.shifts_on(d) {
                let x0 = s.max(lo);
                let x1 = e.min(hi);
                if x1 > x0 {
                    total += (x1 - x0) as i64;
                }
            }
            d = d.succ_opt().unwrap();
            guard += 1;
        }
        total
    }
    pub fn day_start(&self, d: NaiveDate) -> Option<NaiveDateTime> {
        self.shifts_on(d).first().map(|(s, _)| at(d, *s))
    }
    pub fn day_end(&self, d: NaiveDate) -> Option<NaiveDateTime> {
        self.shifts_on(d).last().map(|(_, e)| at(d, *e))
    }
    pub fn shifts_text(shifts: &[Shift]) -> String {
        if shifts.is_empty() {
            return "Nonworking".into();
        }
        shifts.iter().map(|(s, e)| format!("{:02}:{:02}-{:02}:{:02}", s / 60, s % 60, e / 60, e % 60)).collect::<Vec<_>>().join(",")
    }
    pub fn parse_shifts(text: &str) -> Result<Vec<Shift>, String> {
        let t = text.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("nonworking") || t.eq_ignore_ascii_case("off") {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for part in t.split(',') {
            let (a, b) = part.split_once('-').ok_or_else(|| format!("'{part}' is not a shift like 08:00-12:00"))?;
            let s = parse_hm(a)?;
            let e = parse_hm(b)?;
            if e <= s {
                return Err(format!("shift '{part}' ends before it starts"));
            }
            out.push((s, e));
        }
        out.sort();
        Ok(out)
    }
}

fn parse_hm(s: &str) -> Result<u32, String> {
    let s = s.trim();
    let (h, m) = s.split_once(':').unwrap_or((s, "0"));
    let h: u32 = h.trim().parse().map_err(|_| format!("bad time '{s}'"))?;
    let m: u32 = m.trim().parse().map_err(|_| format!("bad time '{s}'"))?;
    if h > 24 || m > 59 || (h == 24 && m > 0) {
        return Err(format!("bad time '{s}'"));
    }
    Ok(h * 60 + m)
}

pub fn minute_of(t: NaiveDateTime) -> u32 {
    t.hour() * 60 + t.minute()
}

/// A moment expressed as (day, minute) where midnight counts as minute 1440 of the previous day.
fn day_min_end(t: NaiveDateTime) -> (NaiveDate, u32) {
    let m = minute_of(t);
    if m == 0 {
        (t.date().pred_opt().unwrap(), 1440)
    } else {
        (t.date(), m)
    }
}

pub fn at(d: NaiveDate, minute: u32) -> NaiveDateTime {
    if minute >= 1440 {
        d.succ_opt().unwrap().and_time(NaiveTime::MIN)
    } else {
        d.and_time(NaiveTime::from_num_seconds_from_midnight_opt(minute * 60, 0).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dt(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
    #[test]
    fn standard_week() {
        let c = Calendar::standard();
        // Monday 2026-11-16 08:00 + 1 day = Monday 17:00
        assert_eq!(c.add_work(dt("2026-11-16 08:00"), 480), dt("2026-11-16 17:00"));
        // + 5 days = Friday 17:00
        assert_eq!(c.add_work(dt("2026-11-16 08:00"), 2400), dt("2026-11-20 17:00"));
        // snap forward from Friday 17:00 = Monday 08:00
        assert_eq!(c.snap_forward(dt("2026-11-20 17:00")), dt("2026-11-23 08:00"));
        assert_eq!(c.sub_work(dt("2026-11-20 17:00"), 2400), dt("2026-11-16 08:00"));
        assert_eq!(c.work_between(dt("2026-11-16 08:00"), dt("2026-11-20 17:00")), 2400);
        assert_eq!(c.snap_backward(dt("2026-11-23 08:00")), dt("2026-11-20 17:00"));
    }
}
