//! The project data: tasks, resources, assignments, calendars and settings.

use crate::calendar::Calendar;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum DurUnit {
    Minutes,
    Hours,
    #[default]
    Days,
    Weeks,
    Months,
}

impl DurUnit {
    pub const ALL: [DurUnit; 5] = [DurUnit::Minutes, DurUnit::Hours, DurUnit::Days, DurUnit::Weeks, DurUnit::Months];

    pub fn id(self) -> &'static str {
        match self {
            DurUnit::Minutes => "minutes",
            DurUnit::Hours => "hours",
            DurUnit::Days => "days",
            DurUnit::Weeks => "weeks",
            DurUnit::Months => "months",
        }
    }
    pub fn from_id(s: &str) -> Option<DurUnit> {
        DurUnit::ALL.into_iter().find(|u| u.id() == s)
    }
    /// The file code the original stores (minute 3, hour 5, day 7, week 9, month 11).
    pub fn code(self) -> i32 {
        match self {
            DurUnit::Minutes => 3,
            DurUnit::Hours => 5,
            DurUnit::Days => 7,
            DurUnit::Weeks => 9,
            DurUnit::Months => 11,
        }
    }
    pub fn from_code(c: i32) -> DurUnit {
        match c {
            3 | 4 => DurUnit::Minutes,
            5 | 6 => DurUnit::Hours,
            9 | 10 => DurUnit::Weeks,
            11 | 12 => DurUnit::Months,
            _ => DurUnit::Days,
        }
    }
    fn label(self, plural: bool) -> &'static str {
        match (self, plural) {
            (DurUnit::Minutes, false) => "min",
            (DurUnit::Minutes, true) => "mins",
            (DurUnit::Hours, false) => "hr",
            (DurUnit::Hours, true) => "hrs",
            (DurUnit::Days, false) => "day",
            (DurUnit::Days, true) => "days",
            (DurUnit::Weeks, false) => "wk",
            (DurUnit::Weeks, true) => "wks",
            (DurUnit::Months, false) => "mon",
            (DurUnit::Months, true) => "mons",
        }
    }
}

/// A length of time. `minutes` is working time unless `elapsed` is set (then wall-clock).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Dur {
    pub minutes: i64,
    #[serde(default)]
    pub unit: DurUnit,
    #[serde(default)]
    pub elapsed: bool,
    #[serde(default)]
    pub estimated: bool,
}

impl Dur {
    pub fn days(d: f64, s: &Settings) -> Dur {
        Dur { minutes: (d * s.minutes_per_day as f64).round() as i64, unit: DurUnit::Days, elapsed: false, estimated: false }
    }
    pub fn zero() -> Dur {
        Dur::default()
    }
    pub fn unit_minutes(unit: DurUnit, elapsed: bool, s: &Settings) -> f64 {
        if elapsed {
            match unit {
                DurUnit::Minutes => 1.0,
                DurUnit::Hours => 60.0,
                DurUnit::Days => 1440.0,
                DurUnit::Weeks => 10080.0,
                DurUnit::Months => 43200.0,
            }
        } else {
            match unit {
                DurUnit::Minutes => 1.0,
                DurUnit::Hours => 60.0,
                DurUnit::Days => s.minutes_per_day as f64,
                DurUnit::Weeks => s.minutes_per_week as f64,
                DurUnit::Months => (s.days_per_month * s.minutes_per_day) as f64,
            }
        }
    }
    pub fn value_in(&self, unit: DurUnit, s: &Settings) -> f64 {
        self.minutes as f64 / Dur::unit_minutes(unit, self.elapsed, s)
    }
    pub fn format(&self, s: &Settings) -> String {
        let v = self.value_in(self.unit, s);
        let num = fmt_num(v);
        let plural = (v - 1.0).abs() > 1e-9;
        format!(
            "{} {}{}{}",
            num,
            if self.elapsed { "e" } else { "" },
            self.unit.label(plural),
            if self.estimated { "?" } else { "" }
        )
    }
    /// Parse "5d", "3 days", "2.5w", "4h", "30m", "1mo", "5ed", "1d?" (estimated). A bare number
    /// uses `default_unit`.
    pub fn parse(text: &str, default_unit: DurUnit, s: &Settings) -> Result<Dur, String> {
        let mut t = text.trim().to_lowercase();
        if t.is_empty() {
            return Err("empty duration".into());
        }
        let estimated = t.ends_with('?');
        if estimated {
            t.pop();
        }
        let t = t.trim();
        let split = t
            .char_indices()
            .find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+'))))
            .map(|(i, _)| i)
            .unwrap_or(t.len());
        let (num, rest) = t.split_at(split);
        let value: f64 = num.trim().parse().map_err(|_| format!("'{text}' is not a duration (try 5d, 3h, 2w)"))?;
        let mut unit_s = rest.trim();
        let mut elapsed = false;
        if unit_s.starts_with('e') && unit_s.len() > 1 {
            elapsed = true;
            unit_s = &unit_s[1..];
        }
        let unit = match unit_s {
            "" => default_unit,
            "m" | "min" | "mins" | "minute" | "minutes" => DurUnit::Minutes,
            "h" | "hr" | "hrs" | "hour" | "hours" => DurUnit::Hours,
            "d" | "dy" | "day" | "days" => DurUnit::Days,
            "w" | "wk" | "wks" | "week" | "weeks" => DurUnit::Weeks,
            "mo" | "mon" | "mons" | "month" | "months" => DurUnit::Months,
            other => return Err(format!("unknown duration unit '{other}'")),
        };
        let minutes = (value * Dur::unit_minutes(unit, elapsed, s)).round() as i64;
        Ok(Dur { minutes, unit, elapsed, estimated })
    }
}

pub fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum LinkKind {
    #[default]
    FS,
    SS,
    FF,
    SF,
}

impl LinkKind {
    pub const ALL: [LinkKind; 4] = [LinkKind::FS, LinkKind::SS, LinkKind::FF, LinkKind::SF];
    pub fn abbr(self) -> &'static str {
        match self {
            LinkKind::FS => "FS",
            LinkKind::SS => "SS",
            LinkKind::FF => "FF",
            LinkKind::SF => "SF",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            LinkKind::FS => "Finish-to-Start",
            LinkKind::SS => "Start-to-Start",
            LinkKind::FF => "Finish-to-Finish",
            LinkKind::SF => "Start-to-Finish",
        }
    }
    pub fn from_abbr(s: &str) -> Option<LinkKind> {
        LinkKind::ALL.into_iter().find(|k| k.abbr().eq_ignore_ascii_case(s))
    }
    /// Code used in the XML interchange format (FF 0, FS 1, SF 2, SS 3).
    pub fn xml_code(self) -> i32 {
        match self {
            LinkKind::FF => 0,
            LinkKind::FS => 1,
            LinkKind::SF => 2,
            LinkKind::SS => 3,
        }
    }
    pub fn from_xml_code(c: i32) -> LinkKind {
        match c {
            0 => LinkKind::FF,
            2 => LinkKind::SF,
            3 => LinkKind::SS,
            _ => LinkKind::FS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ConstraintKind {
    #[default]
    ASAP,
    ALAP,
    MSO,
    MFO,
    SNET,
    SNLT,
    FNET,
    FNLT,
}

impl ConstraintKind {
    /// In the original's code order 0..7.
    pub const ALL: [ConstraintKind; 8] = [
        ConstraintKind::ASAP,
        ConstraintKind::ALAP,
        ConstraintKind::MSO,
        ConstraintKind::MFO,
        ConstraintKind::SNET,
        ConstraintKind::SNLT,
        ConstraintKind::FNET,
        ConstraintKind::FNLT,
    ];
    pub fn abbr(self) -> &'static str {
        match self {
            ConstraintKind::ASAP => "ASAP",
            ConstraintKind::ALAP => "ALAP",
            ConstraintKind::MSO => "MSO",
            ConstraintKind::MFO => "MFO",
            ConstraintKind::SNET => "SNET",
            ConstraintKind::SNLT => "SNLT",
            ConstraintKind::FNET => "FNET",
            ConstraintKind::FNLT => "FNLT",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            ConstraintKind::ASAP => "As Soon As Possible",
            ConstraintKind::ALAP => "As Late As Possible",
            ConstraintKind::MSO => "Must Start On",
            ConstraintKind::MFO => "Must Finish On",
            ConstraintKind::SNET => "Start No Earlier Than",
            ConstraintKind::SNLT => "Start No Later Than",
            ConstraintKind::FNET => "Finish No Earlier Than",
            ConstraintKind::FNLT => "Finish No Later Than",
        }
    }
    pub fn code(self) -> i32 {
        ConstraintKind::ALL.iter().position(|c| *c == self).unwrap_or(0) as i32
    }
    pub fn from_code(c: i32) -> ConstraintKind {
        ConstraintKind::ALL.get(c as usize).copied().unwrap_or_default()
    }
    pub fn parse(s: &str) -> Option<ConstraintKind> {
        let s = s.trim();
        ConstraintKind::ALL
            .into_iter()
            .find(|c| c.abbr().eq_ignore_ascii_case(s) || c.title().eq_ignore_ascii_case(s))
            .or_else(|| s.parse::<i32>().ok().map(ConstraintKind::from_code))
    }
    pub fn needs_date(self) -> bool {
        !matches!(self, ConstraintKind::ASAP | ConstraintKind::ALAP)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum TaskType {
    #[default]
    FixedUnits,
    FixedDuration,
    FixedWork,
}

impl TaskType {
    pub const ALL: [TaskType; 3] = [TaskType::FixedUnits, TaskType::FixedDuration, TaskType::FixedWork];
    pub fn title(self) -> &'static str {
        match self {
            TaskType::FixedUnits => "Fixed Units",
            TaskType::FixedDuration => "Fixed Duration",
            TaskType::FixedWork => "Fixed Work",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            TaskType::FixedUnits => "fixedUnits",
            TaskType::FixedDuration => "fixedDuration",
            TaskType::FixedWork => "fixedWork",
        }
    }
    pub fn code(self) -> i32 {
        TaskType::ALL.iter().position(|c| *c == self).unwrap_or(0) as i32
    }
    pub fn parse(s: &str) -> Option<TaskType> {
        let k: String = s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        TaskType::ALL
            .into_iter()
            .find(|t| t.id().to_lowercase() == k || t.title().replace(' ', "").to_lowercase() == k)
            .or_else(|| s.trim().parse::<usize>().ok().and_then(|i| TaskType::ALL.get(i).copied()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ResourceKind {
    #[default]
    Work,
    Material,
    Cost,
}

impl ResourceKind {
    pub const ALL: [ResourceKind; 3] = [ResourceKind::Work, ResourceKind::Material, ResourceKind::Cost];
    pub fn title(self) -> &'static str {
        match self {
            ResourceKind::Work => "Work",
            ResourceKind::Material => "Material",
            ResourceKind::Cost => "Cost",
        }
    }
    pub fn parse(s: &str) -> Option<ResourceKind> {
        ResourceKind::ALL.into_iter().find(|k| k.title().eq_ignore_ascii_case(s.trim()))
    }
    /// XML interchange code (material 0, work 1, cost 2).
    pub fn xml_code(self) -> i32 {
        match self {
            ResourceKind::Material => 0,
            ResourceKind::Work => 1,
            ResourceKind::Cost => 2,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tasks, resources, assignments
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub pred: u32,
    #[serde(default)]
    pub kind: LinkKind,
    #[serde(default)]
    pub lag: Dur,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct Baseline {
    pub start: Option<NaiveDateTime>,
    pub finish: Option<NaiveDateTime>,
    pub duration: Dur,
    pub work: i64,
    pub cost: f64,
}

/// Text formatting for a row or a kind of row (Font group, Text Styles dialog). Unset parts
/// fall back to the view's text styles and then the defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct TextStyle {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub color: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
    /// Points.
    pub size: Option<f32>,
    /// "sans" or "mono".
    pub font: Option<String>,
}

impl TextStyle {
    /// `over` wins where it says something.
    pub fn merged(&self, over: &TextStyle) -> TextStyle {
        TextStyle {
            bold: over.bold.or(self.bold),
            italic: over.italic.or(self.italic),
            underline: over.underline.or(self.underline),
            color: over.color.or(self.color),
            background: over.background.or(self.background),
            size: over.size.or(self.size),
            font: over.font.clone().or_else(|| self.font.clone()),
        }
    }
    pub fn is_empty(&self) -> bool {
        *self == TextStyle::default()
    }
}

/// "#3366cc", "3366cc", "red"... -> RGB.
pub fn parse_color(s: &str) -> Result<[u8; 3], String> {
    let t = s.trim().trim_start_matches('#').to_lowercase();
    let named = match t.as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([192, 0, 0]),
        "green" => Some([0, 128, 0]),
        "blue" => Some([0, 90, 200]),
        "yellow" => Some([255, 230, 0]),
        "orange" => Some([237, 125, 49]),
        "purple" => Some([112, 48, 160]),
        "gray" | "grey" => Some([128, 128, 128]),
        _ => None,
    };
    if let Some(c) = named {
        return Ok(c);
    }
    if t.len() == 6 {
        if let Ok(v) = u32::from_str_radix(&t, 16) {
            return Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        }
    }
    Err(format!("'{s}' is not a colour (use #rrggbb or a name like red)"))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub uid: u32,
    pub name: String,
    #[serde(default = "one")]
    pub outline_level: u8,
    pub duration: Dur,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub manual_start: Option<NaiveDateTime>,
    #[serde(default)]
    pub constraint: ConstraintKind,
    #[serde(default)]
    pub constraint_date: Option<NaiveDateTime>,
    #[serde(default)]
    pub deadline: Option<NaiveDateTime>,
    #[serde(default)]
    pub task_type: TaskType,
    #[serde(default)]
    pub effort_driven: bool,
    #[serde(default)]
    pub milestone: bool,
    #[serde(default)]
    pub percent_complete: f64,
    #[serde(default)]
    pub actual_start: Option<NaiveDateTime>,
    #[serde(default)]
    pub actual_finish: Option<NaiveDateTime>,
    #[serde(default = "five_hundred")]
    pub priority: u16,
    #[serde(default)]
    pub fixed_cost: f64,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub hyperlink: String,
    #[serde(default)]
    pub predecessors: Vec<Link>,
    #[serde(default)]
    pub calendar: Option<String>,
    #[serde(default = "yes")]
    pub active: bool,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub level_delay_to: Option<NaiveDateTime>,
    #[serde(default)]
    pub text: [String; 5],
    #[serde(default)]
    pub number: [f64; 5],
    #[serde(default)]
    pub flag: [bool; 5],
    #[serde(default)]
    pub baselines: Vec<Option<Baseline>>,
    #[serde(default)]
    pub bar_color: Option<[u8; 3]>,
    /// Font and colours of this task's row (Task tab > Font, Format Painter).
    #[serde(default)]
    pub style: Option<TextStyle>,
    /// Shown on the Timeline (Task tab > Add to Timeline).
    #[serde(default)]
    pub on_timeline: bool,
    /// No bar in the Gantt chart (Task Information > Hide Bar).
    #[serde(default)]
    pub hide_bar: bool,

    // ---- computed by the scheduler ----
    #[serde(default)]
    pub start: NaiveDateTime,
    #[serde(default)]
    pub finish: NaiveDateTime,
    #[serde(default)]
    pub early_start: NaiveDateTime,
    #[serde(default)]
    pub early_finish: NaiveDateTime,
    #[serde(default)]
    pub late_start: NaiveDateTime,
    #[serde(default)]
    pub late_finish: NaiveDateTime,
    #[serde(default)]
    pub total_slack: i64,
    #[serde(default)]
    pub free_slack: i64,
    #[serde(default)]
    pub critical: bool,
    #[serde(default)]
    pub summary: bool,
    #[serde(default)]
    pub work: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub late: bool,
    #[serde(default)]
    pub missed_deadline: bool,
}

fn one() -> u8 {
    1
}
fn five_hundred() -> u16 {
    500
}
fn yes() -> bool {
    true
}

impl Task {
    pub fn new(uid: u32, name: &str, duration: Dur) -> Task {
        Task {
            uid,
            name: name.to_string(),
            outline_level: 1,
            duration,
            manual: false,
            manual_start: None,
            constraint: ConstraintKind::ASAP,
            constraint_date: None,
            deadline: None,
            task_type: TaskType::FixedUnits,
            effort_driven: false,
            milestone: false,
            percent_complete: 0.0,
            actual_start: None,
            actual_finish: None,
            priority: 500,
            fixed_cost: 0.0,
            notes: String::new(),
            hyperlink: String::new(),
            predecessors: vec![],
            calendar: None,
            active: true,
            collapsed: false,
            level_delay_to: None,
            text: Default::default(),
            number: [0.0; 5],
            flag: [false; 5],
            baselines: vec![],
            bar_color: None,
            style: None,
            on_timeline: false,
            hide_bar: false,
            start: NaiveDateTime::default(),
            finish: NaiveDateTime::default(),
            early_start: NaiveDateTime::default(),
            early_finish: NaiveDateTime::default(),
            late_start: NaiveDateTime::default(),
            late_finish: NaiveDateTime::default(),
            total_slack: 0,
            free_slack: 0,
            critical: false,
            summary: false,
            work: 0,
            cost: 0.0,
            late: false,
            missed_deadline: false,
        }
    }
    pub fn is_milestone(&self) -> bool {
        self.milestone || (!self.summary && self.duration.minutes == 0)
    }
    pub fn baseline(&self, n: usize) -> Option<&Baseline> {
        self.baselines.get(n).and_then(|b| b.as_ref())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Resource {
    pub uid: u32,
    pub name: String,
    #[serde(default)]
    pub initials: String,
    #[serde(default)]
    pub kind: ResourceKind,
    #[serde(default)]
    pub material_label: String,
    #[serde(default)]
    pub group: String,
    #[serde(default = "one_f")]
    pub max_units: f64,
    #[serde(default)]
    pub std_rate: f64,
    #[serde(default)]
    pub ovt_rate: f64,
    #[serde(default)]
    pub cost_per_use: f64,
    #[serde(default)]
    pub calendar: Option<String>,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub text: [String; 3],
    // computed
    #[serde(default)]
    pub work: i64,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub peak: f64,
    #[serde(default)]
    pub overallocated: bool,
}

fn one_f() -> f64 {
    1.0
}

impl Resource {
    pub fn new(uid: u32, name: &str) -> Resource {
        let initials: String = name.split_whitespace().filter_map(|w| w.chars().next()).collect();
        Resource {
            uid,
            name: name.to_string(),
            initials,
            kind: ResourceKind::Work,
            material_label: String::new(),
            group: String::new(),
            max_units: 1.0,
            std_rate: 0.0,
            ovt_rate: 0.0,
            cost_per_use: 0.0,
            calendar: None,
            code: String::new(),
            email: String::new(),
            notes: String::new(),
            text: Default::default(),
            work: 0,
            cost: 0.0,
            peak: 0.0,
            overallocated: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Assignment {
    pub uid: u32,
    pub task: u32,
    pub resource: u32,
    /// 1.0 = 100 %. For material resources: quantity.
    pub units: f64,
    /// Work in minutes (work resources).
    #[serde(default)]
    pub work: i64,
    /// Amount for cost resources.
    #[serde(default)]
    pub cost_amount: f64,
    // computed
    #[serde(default)]
    pub cost: f64,
}

// ---------------------------------------------------------------------------------------------
// Settings and project
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub minutes_per_day: i64,
    pub minutes_per_week: i64,
    pub days_per_month: i64,
    pub default_start_min: u32,
    pub default_finish_min: u32,
    pub default_task_type: TaskType,
    pub new_tasks_effort_driven: bool,
    pub new_tasks_manual: bool,
    pub honor_constraints: bool,
    pub critical_slack: i64,
    pub duration_unit: DurUnit,
    pub work_unit: DurUnit,
    pub currency: String,
    pub auto_link: bool,
    pub auto_calc: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            minutes_per_day: 480,
            minutes_per_week: 2400,
            days_per_month: 20,
            default_start_min: 480,
            default_finish_min: 1020,
            default_task_type: TaskType::FixedUnits,
            new_tasks_effort_driven: false,
            new_tasks_manual: false,
            honor_constraints: true,
            critical_slack: 0,
            duration_unit: DurUnit::Days,
            work_unit: DurUnit::Hours,
            currency: "$".into(),
            auto_link: true,
            auto_calc: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub title: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub manager: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub notes: String,
    pub start: NaiveDateTime,
    #[serde(default)]
    pub status_date: Option<NaiveDateTime>,
    pub calendar: String,
    pub calendars: Vec<Calendar>,
    #[serde(default)]
    pub settings: Settings,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub assignments: Vec<Assignment>,
    #[serde(default)]
    pub next_uid: u32,
    #[serde(default)]
    pub next_res_uid: u32,
    #[serde(default)]
    pub next_asg_uid: u32,
    #[serde(default)]
    pub baseline_saved: Vec<Option<NaiveDateTime>>,
    // computed
    #[serde(default)]
    pub finish: NaiveDateTime,
    #[serde(default)]
    pub circular: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Project {
    pub fn new(title: &str, start: NaiveDate) -> Project {
        let settings = Settings::default();
        let start = start.and_time(NaiveTime::from_num_seconds_from_midnight_opt(settings.default_start_min * 60, 0).unwrap());
        let mut p = Project {
            title: title.to_string(),
            subject: String::new(),
            manager: String::new(),
            company: String::new(),
            notes: String::new(),
            start,
            status_date: None,
            calendar: "Standard".into(),
            calendars: Calendar::base_calendars(),
            settings,
            tasks: vec![],
            resources: vec![],
            assignments: vec![],
            next_uid: 1,
            next_res_uid: 1,
            next_asg_uid: 1,
            baseline_saved: vec![None; 11],
            finish: start,
            circular: false,
            warnings: vec![],
        };
        crate::schedule::schedule(&mut p);
        p
    }

    pub fn cal(&self) -> &Calendar {
        self.calendar_named(&self.calendar).unwrap_or(&self.calendars[0])
    }
    pub fn calendar_named(&self, name: &str) -> Option<&Calendar> {
        self.calendars.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }
    pub fn task_cal(&self, t: &Task) -> &Calendar {
        t.calendar.as_deref().and_then(|n| self.calendar_named(n)).unwrap_or_else(|| self.cal())
    }
    pub fn task_index(&self, uid: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.uid == uid)
    }
    pub fn task_by_uid(&self, uid: u32) -> Option<&Task> {
        self.tasks.iter().find(|t| t.uid == uid)
    }
    pub fn resource_index(&self, uid: u32) -> Option<usize> {
        self.resources.iter().position(|r| r.uid == uid)
    }
    pub fn resource_by_uid(&self, uid: u32) -> Option<&Resource> {
        self.resources.iter().find(|r| r.uid == uid)
    }
    pub fn resource_by_name(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name.eq_ignore_ascii_case(name.trim()))
    }
    pub fn alloc_uid(&mut self) -> u32 {
        let m = self.tasks.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
        let u = self.next_uid.max(m);
        self.next_uid = u + 1;
        u
    }
    pub fn alloc_res_uid(&mut self) -> u32 {
        let m = self.resources.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
        let u = self.next_res_uid.max(m);
        self.next_res_uid = u + 1;
        u
    }
    pub fn alloc_asg_uid(&mut self) -> u32 {
        let m = self.assignments.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
        let u = self.next_asg_uid.max(m);
        self.next_asg_uid = u + 1;
        u
    }
    /// Index range of the descendants of task `i` (empty for non-summary tasks).
    pub fn descendants(&self, i: usize) -> std::ops::Range<usize> {
        let lvl = self.tasks[i].outline_level;
        let mut j = i + 1;
        while j < self.tasks.len() && self.tasks[j].outline_level > lvl {
            j += 1;
        }
        i + 1..j
    }
    pub fn parent(&self, i: usize) -> Option<usize> {
        let lvl = self.tasks[i].outline_level;
        (0..i).rev().find(|&j| self.tasks[j].outline_level < lvl)
    }
    pub fn is_summary(&self, i: usize) -> bool {
        i + 1 < self.tasks.len() && self.tasks[i + 1].outline_level > self.tasks[i].outline_level
    }
    pub fn assignments_of_task(&self, uid: u32) -> impl Iterator<Item = &Assignment> {
        self.assignments.iter().filter(move |a| a.task == uid)
    }
    pub fn assignments_of_resource(&self, uid: u32) -> impl Iterator<Item = &Assignment> {
        self.assignments.iter().filter(move |a| a.resource == uid)
    }
    /// Outline number like "1.2.3".
    pub fn outline_number(&self, i: usize) -> String {
        let mut counters: Vec<u32> = vec![];
        for (j, t) in self.tasks.iter().enumerate().take(i + 1) {
            let lvl = t.outline_level.max(1) as usize;
            counters.truncate(lvl);
            while counters.len() < lvl {
                counters.push(0);
            }
            counters[lvl - 1] += 1;
            if j == i {
                break;
            }
        }
        counters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(".")
    }
    pub fn resource_names(&self, uid: u32) -> String {
        let mut out = vec![];
        for a in self.assignments_of_task(uid) {
            if let Some(r) = self.resource_by_uid(a.resource) {
                match r.kind {
                    ResourceKind::Work => {
                        if (a.units - 1.0).abs() < 1e-6 {
                            out.push(r.name.clone())
                        } else {
                            out.push(format!("{}[{}%]", r.name, fmt_num(a.units * 100.0)))
                        }
                    }
                    ResourceKind::Material => out.push(format!(
                        "{}[{} {}]",
                        r.name,
                        fmt_num(a.units),
                        if r.material_label.is_empty() { "units" } else { &r.material_label }
                    )),
                    ResourceKind::Cost => out.push(format!("{}[{}{}]", r.name, self.settings.currency, fmt_num(a.cost_amount))),
                }
            }
        }
        out.join(",")
    }
    pub fn predecessors_text(&self, t: &Task) -> String {
        t.predecessors
            .iter()
            .filter_map(|l| {
                let id = self.task_index(l.pred)? + 1;
                let mut s = id.to_string();
                if l.kind != LinkKind::FS || l.lag.minutes != 0 {
                    s.push_str(l.kind.abbr());
                }
                if l.lag.minutes != 0 {
                    let f = l.lag.format(&self.settings);
                    if l.lag.minutes > 0 {
                        s.push('+');
                    }
                    s.push_str(&f);
                }
                Some(s)
            })
            .collect::<Vec<_>>()
            .join(",")
    }
    pub fn successors_text(&self, uid: u32) -> String {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.predecessors.iter().any(|l| l.pred == uid))
            .map(|(i, _)| (i + 1).to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
    pub fn fmt_work(&self, minutes: i64) -> String {
        let u = self.settings.work_unit;
        let v = minutes as f64 / Dur::unit_minutes(u, false, &self.settings);
        let plural = (v - 1.0).abs() > 1e-9;
        format!("{} {}", fmt_num(v), u.label(plural))
    }
    pub fn fmt_cost(&self, v: f64) -> String {
        fmt_money(&self.settings.currency, v)
    }
}

pub fn fmt_money(cur: &str, v: f64) -> String {
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as i64;
    let whole = cents / 100;
    let frac = cents % 100;
    let mut digits = whole.to_string();
    let mut grouped = String::new();
    while digits.len() > 3 {
        let tail = digits.split_off(digits.len() - 3);
        grouped = format!(",{}{}", tail, grouped);
    }
    grouped = format!("{}{}", digits, grouped);
    format!("{}{}{}.{:02}", if neg { "-" } else { "" }, cur, grouped, frac)
}

// ---------------------------------------------------------------------------------------------
// Date parsing / formatting
// ---------------------------------------------------------------------------------------------

/// How dates are shown and read (File > Options > Language): 0 month/day/year, 1 day/month/year,
/// 2 year-month-day.
pub static DATE_STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn date_style() -> u8 {
    DATE_STYLE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn fmt_date(d: NaiveDateTime) -> String {
    match date_style() {
        1 => d.format("%a %-d/%-m/%y").to_string(),
        2 => d.format("%a %Y-%m-%d").to_string(),
        _ => d.format("%a %-m/%-d/%y").to_string(),
    }
}
pub fn fmt_date_short(d: NaiveDateTime) -> String {
    match date_style() {
        1 => d.format("%-d/%-m").to_string(),
        2 => d.format("%m-%d").to_string(),
        _ => d.format("%-m/%-d").to_string(),
    }
}
pub fn fmt_datetime(d: NaiveDateTime) -> String {
    d.format("%Y-%m-%d %H:%M").to_string()
}

/// Parse a date: "2026-11-16", "2026-11-16 08:00", "2026-11-16T08:00:00", "11/16/2026", "11/16/26".
/// Without a time, `default_min` minutes after midnight is used.
pub fn parse_date(s: &str, default_min: u32) -> Result<NaiveDateTime, String> {
    let s = s.trim();
    let s = s
        .trim_start_matches(|c: char| c.is_alphabetic())
        .trim_start_matches(|c: char| c == ',' || c == ' ');
    let dmy = date_style() == 1;
    let with_time: [&str; 6] = if dmy {
        ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M", "%d/%m/%Y %H:%M", "%d/%m/%y %H:%M"]
    } else {
        ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M", "%m/%d/%Y %H:%M", "%m/%d/%y %H:%M"]
    };
    for f in with_time {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, f) {
            return Ok(d);
        }
    }
    let date_only: [&str; 3] = if dmy { ["%Y-%m-%d", "%d/%m/%Y", "%d/%m/%y"] } else { ["%Y-%m-%d", "%m/%d/%Y", "%m/%d/%y"] };
    for f in date_only {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Ok(d.and_time(NaiveTime::from_num_seconds_from_midnight_opt(default_min.min(1439) * 60, 0).unwrap()));
        }
    }
    Err(format!("'{s}' is not a date (use 2026-11-16 or 2026-11-16 08:00)"))
}
