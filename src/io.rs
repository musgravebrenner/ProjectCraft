//! Files: the native .plan.json format, the common XML project interchange format (import and
//! export), CSV tables, an SVG picture of the Gantt chart, HTML and Markdown summaries.

use crate::calendar::{CalException, Calendar};
use crate::fields;
use crate::model::*;
use crate::reports;
use crate::schedule;
use crate::session::{Row, Session, ViewState};
use chrono::{Datelike, Duration, NaiveDateTime};
use serde_json::{json, Value};
use std::path::Path;

pub fn ext(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.ends_with(".plan.json") {
        return "plan.json".into();
    }
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn is_project_format(path: &Path) -> bool {
    matches!(ext(path).as_str(), "plan.json" | "json" | "xml")
}

pub fn save(p: &Project, view: &ViewState, path: &Path) -> Result<(), String> {
    let v = json!({"format": "plancraft", "version": 1, "project": p, "view": view});
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
}

/// Whether a path is a binary .mpp project file (by its extension or its container's signature).
pub fn is_mpp(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    ext == "mpp" || ext == "mpt" || crate::mpp::is_compound_file(path)
}

pub fn load(path: &Path) -> Result<(Project, Option<ViewState>), String> {
    if is_mpp(path) {
        let (xml, _) = crate::mpp::to_xml(path)?;
        let mut p = import_xml(&xml)?;
        schedule::schedule(&mut p);
        return Ok((p, None));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))?;
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if t.starts_with('<') {
        let mut p = import_xml(t)?;
        schedule::schedule(&mut p);
        return Ok((p, None));
    }
    let v: Value = serde_json::from_str(t).map_err(|e| format!("{} is not a project file: {e}", path.display()))?;
    let (pv, vv) = if v.get("project").is_some() { (v["project"].clone(), v.get("view").cloned()) } else { (v, None) };
    let mut p: Project = serde_json::from_value(pv).map_err(|e| format!("{} is not a project file: {e}", path.display()))?;
    if p.calendars.is_empty() {
        p.calendars = Calendar::base_calendars();
    }
    schedule::schedule(&mut p);
    let view = vv.and_then(|v| serde_json::from_value(v).ok());
    Ok((p, view))
}

/// Write by extension: project formats, or any export.
pub fn write_any(s: &Session, path: &Path) -> Result<(), String> {
    match ext(path).as_str() {
        "plan.json" | "json" => save(&s.project, &s.view, path),
        "xml" => export(s, "xml", path),
        "csv" => export(s, "csv", path),
        "svg" => export(s, "svg", path),
        "html" | "htm" => export(s, "html", path),
        "md" | "markdown" | "txt" => export(s, "markdown", path),
        "png" => Err("PNG pictures need the window (file.exportPicture); from the command line write .svg".into()),
        "" => save(&s.project, &s.view, &path.with_extension("plan.json")),
        e => Err(format!("unknown file type .{e} (use .plan.json, .xml, .csv, .svg, .html or .md)")),
    }
}

pub fn export(s: &Session, fmt: &str, path: &Path) -> Result<(), String> {
    let text = match fmt {
        "csv" => tasks_csv(s),
        "resourcesCsv" => resources_csv(&s.project),
        "xml" => export_xml(&s.project),
        "svg" => gantt_svg(s),
        "json" => serde_json::to_string_pretty(&json!({"format": "plancraft", "version": 1, "project": s.project, "view": s.view})).map_err(|e| e.to_string())?,
        "html" => html_report(s),
        "markdown" => markdown_report(s),
        f => return Err(format!("unknown export format '{f}'")),
    };
    std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn tasks_csv(s: &Session) -> String {
    let p = &s.project;
    let cols: Vec<&str> = s.view.columns.iter().map(|c| c.as_str()).filter(|c| *c != "mode").collect();
    let mut out = cols.iter().map(|c| csv_cell(fields::task_field(c).map(|f| f.title).unwrap_or(c))).collect::<Vec<_>>().join(",");
    out.push('\n');
    for i in 0..p.tasks.len() {
        let row: Vec<String> = cols
            .iter()
            .map(|c| {
                let mut t = fields::task_text(p, i, c);
                if *c == "name" {
                    t = format!("{}{}", "  ".repeat(p.tasks[i].outline_level.saturating_sub(1) as usize), t);
                }
                csv_cell(&t)
            })
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

pub fn resources_csv(p: &Project) -> String {
    let cols = ["id", "name", "type", "initials", "group", "maxUnits", "stdRate", "ovtRate", "costPerUse", "work", "cost", "overallocated"];
    let mut out = cols.iter().map(|c| fields::resource_field(c).map(|f| f.title).unwrap_or(c).to_string()).collect::<Vec<_>>().join(",");
    out.push('\n');
    for i in 0..p.resources.len() {
        out.push_str(&cols.iter().map(|c| csv_cell(&fields::resource_text(p, i, c))).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------------------------
// XML interchange
// ---------------------------------------------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
fn xdate(d: NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%S").to_string()
}
fn xdur(minutes: i64) -> String {
    let m = minutes.abs();
    format!("{}PT{}H{}M0S", if minutes < 0 { "-" } else { "" }, m / 60, m % 60)
}

pub fn export_xml(p: &Project) -> String {
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Project xmlns=\"http://schemas.microsoft.com/project\">\n");
    let tag = |o: &mut String, ind: usize, k: &str, v: &str| o.push_str(&format!("{}<{k}>{}</{k}>\n", " ".repeat(ind), esc(v)));
    tag(&mut o, 1, "Name", &p.title);
    tag(&mut o, 1, "Title", &p.title);
    tag(&mut o, 1, "Subject", &p.subject);
    tag(&mut o, 1, "Company", &p.company);
    tag(&mut o, 1, "Manager", &p.manager);
    tag(&mut o, 1, "ScheduleFromStart", "1");
    tag(&mut o, 1, "StartDate", &xdate(p.start));
    tag(&mut o, 1, "FinishDate", &xdate(p.finish));
    if let Some(sd) = p.status_date {
        tag(&mut o, 1, "StatusDate", &xdate(sd));
    }
    tag(&mut o, 1, "DefaultStartTime", &format!("{:02}:{:02}:00", p.settings.default_start_min / 60, p.settings.default_start_min % 60));
    tag(&mut o, 1, "DefaultFinishTime", &format!("{:02}:{:02}:00", p.settings.default_finish_min / 60, p.settings.default_finish_min % 60));
    tag(&mut o, 1, "MinutesPerDay", &p.settings.minutes_per_day.to_string());
    tag(&mut o, 1, "MinutesPerWeek", &p.settings.minutes_per_week.to_string());
    tag(&mut o, 1, "DaysPerMonth", &p.settings.days_per_month.to_string());
    tag(&mut o, 1, "DefaultTaskType", &p.settings.default_task_type.code().to_string());
    tag(&mut o, 1, "DurationFormat", &p.settings.duration_unit.code().to_string());
    tag(&mut o, 1, "CurrencySymbol", &p.settings.currency);
    let cal_uid = |name: &str| p.calendars.iter().position(|c| c.name == name).map(|i| i + 1).unwrap_or(1);
    tag(&mut o, 1, "CalendarUID", &cal_uid(&p.calendar).to_string());
    o.push_str(" <Calendars>\n");
    for (ci, c) in p.calendars.iter().enumerate() {
        o.push_str("  <Calendar>\n");
        tag(&mut o, 3, "UID", &(ci + 1).to_string());
        tag(&mut o, 3, "Name", &c.name);
        tag(&mut o, 3, "IsBaseCalendar", "1");
        o.push_str("   <WeekDays>\n");
        for (d, shifts) in c.week.iter().enumerate() {
            o.push_str("    <WeekDay>\n");
            tag(&mut o, 5, "DayType", &(d + 1).to_string());
            tag(&mut o, 5, "DayWorking", if shifts.is_empty() { "0" } else { "1" });
            if !shifts.is_empty() {
                o.push_str("     <WorkingTimes>\n");
                for (a, b) in shifts {
                    o.push_str(&format!(
                        "      <WorkingTime><FromTime>{:02}:{:02}:00</FromTime><ToTime>{:02}:{:02}:00</ToTime></WorkingTime>\n",
                        a / 60,
                        a % 60,
                        (b / 60) % 24,
                        b % 60
                    ));
                }
                o.push_str("     </WorkingTimes>\n");
            }
            o.push_str("    </WeekDay>\n");
        }
        o.push_str("   </WeekDays>\n");
        if !c.exceptions.is_empty() {
            o.push_str("   <Exceptions>\n");
            for e in &c.exceptions {
                o.push_str("    <Exception>\n");
                o.push_str(&format!(
                    "     <TimePeriod><FromDate>{}T00:00:00</FromDate><ToDate>{}T23:59:00</ToDate></TimePeriod>\n",
                    e.start, e.end
                ));
                tag(&mut o, 5, "Name", &e.name);
                tag(&mut o, 5, "DayWorking", if e.shifts.is_empty() { "0" } else { "1" });
                if !e.shifts.is_empty() {
                    o.push_str("     <WorkingTimes>\n");
                    for (a, b) in &e.shifts {
                        o.push_str(&format!("      <WorkingTime><FromTime>{:02}:{:02}:00</FromTime><ToTime>{:02}:{:02}:00</ToTime></WorkingTime>\n", a / 60, a % 60, (b / 60) % 24, b % 60));
                    }
                    o.push_str("     </WorkingTimes>\n");
                }
                o.push_str("    </Exception>\n");
            }
            o.push_str("   </Exceptions>\n");
        }
        o.push_str("  </Calendar>\n");
    }
    o.push_str(" </Calendars>\n <Tasks>\n");
    for (i, t) in p.tasks.iter().enumerate() {
        o.push_str("  <Task>\n");
        tag(&mut o, 3, "UID", &t.uid.to_string());
        tag(&mut o, 3, "ID", &(i + 1).to_string());
        tag(&mut o, 3, "Name", &t.name);
        tag(&mut o, 3, "Active", if t.active { "1" } else { "0" });
        tag(&mut o, 3, "Manual", if t.manual { "1" } else { "0" });
        tag(&mut o, 3, "Type", &t.task_type.code().to_string());
        tag(&mut o, 3, "WBS", &p.outline_number(i));
        tag(&mut o, 3, "OutlineNumber", &p.outline_number(i));
        tag(&mut o, 3, "OutlineLevel", &t.outline_level.to_string());
        tag(&mut o, 3, "Priority", &t.priority.to_string());
        tag(&mut o, 3, "Start", &xdate(t.start));
        tag(&mut o, 3, "Finish", &xdate(t.finish));
        tag(&mut o, 3, "Duration", &xdur(t.duration.minutes));
        tag(&mut o, 3, "DurationFormat", &(t.duration.unit.code() + if t.duration.elapsed { 1 } else { 0 } + if t.duration.estimated { 32 } else { 0 }).to_string());
        tag(&mut o, 3, "Work", &xdur(t.work));
        tag(&mut o, 3, "EffortDriven", if t.effort_driven { "1" } else { "0" });
        tag(&mut o, 3, "Estimated", if t.duration.estimated { "1" } else { "0" });
        tag(&mut o, 3, "Milestone", if t.is_milestone() { "1" } else { "0" });
        tag(&mut o, 3, "Summary", if t.summary { "1" } else { "0" });
        tag(&mut o, 3, "Critical", if t.critical { "1" } else { "0" });
        tag(&mut o, 3, "PercentComplete", &fmt_num(t.percent_complete.round()));
        // other schedulers derive % complete from actual and remaining duration
        if t.percent_complete > 0.0 && !t.summary {
            let actual = (t.duration.minutes as f64 * t.percent_complete / 100.0).round() as i64;
            tag(&mut o, 3, "ActualDuration", &xdur(actual));
            tag(&mut o, 3, "RemainingDuration", &xdur(t.duration.minutes - actual));
        }
        tag(&mut o, 3, "Cost", &cents(t.cost));
        tag(&mut o, 3, "FixedCost", &cents(t.fixed_cost));
        if let Some(a) = t.actual_start {
            tag(&mut o, 3, "ActualStart", &xdate(a));
        }
        if let Some(a) = t.actual_finish {
            tag(&mut o, 3, "ActualFinish", &xdate(a));
        }
        tag(&mut o, 3, "ConstraintType", &t.constraint.code().to_string());
        if let Some(cd) = t.constraint_date {
            tag(&mut o, 3, "ConstraintDate", &xdate(cd));
        }
        if let Some(dl) = t.deadline {
            tag(&mut o, 3, "Deadline", &xdate(dl));
        }
        if let Some(c) = &t.calendar {
            tag(&mut o, 3, "CalendarUID", &cal_uid(c).to_string());
        }
        tag(&mut o, 3, "TotalSlack", &(t.total_slack * 10).to_string());
        tag(&mut o, 3, "FreeSlack", &(t.free_slack * 10).to_string());
        if !t.notes.is_empty() {
            tag(&mut o, 3, "Notes", &t.notes);
        }
        if !t.hyperlink.is_empty() {
            tag(&mut o, 3, "Hyperlink", &t.hyperlink);
        }
        for l in &t.predecessors {
            o.push_str("   <PredecessorLink>\n");
            tag(&mut o, 4, "PredecessorUID", &l.pred.to_string());
            tag(&mut o, 4, "Type", &l.kind.xml_code().to_string());
            tag(&mut o, 4, "LinkLag", &(l.lag.minutes * 10).to_string());
            tag(&mut o, 4, "LagFormat", &l.lag.unit.code().to_string());
            o.push_str("   </PredecessorLink>\n");
        }
        if let Some(b) = t.baseline(0) {
            o.push_str("   <Baseline>\n");
            tag(&mut o, 4, "Number", "0");
            if let Some(s) = b.start {
                tag(&mut o, 4, "Start", &xdate(s));
            }
            if let Some(f) = b.finish {
                tag(&mut o, 4, "Finish", &xdate(f));
            }
            tag(&mut o, 4, "Duration", &xdur(b.duration.minutes));
            tag(&mut o, 4, "Work", &xdur(b.work));
            tag(&mut o, 4, "Cost", &cents(b.cost));
            o.push_str("   </Baseline>\n");
        }
        o.push_str("  </Task>\n");
    }
    o.push_str(" </Tasks>\n <Resources>\n");
    for (i, r) in p.resources.iter().enumerate() {
        o.push_str("  <Resource>\n");
        tag(&mut o, 3, "UID", &r.uid.to_string());
        tag(&mut o, 3, "ID", &(i + 1).to_string());
        tag(&mut o, 3, "Name", &r.name);
        tag(&mut o, 3, "Type", &r.kind.xml_code().to_string());
        // other schedulers mark cost resources with this flag rather than by Type
        if r.kind == ResourceKind::Cost {
            tag(&mut o, 3, "IsCostResource", "1");
        }
        tag(&mut o, 3, "Initials", &r.initials);
        tag(&mut o, 3, "Group", &r.group);
        if !r.material_label.is_empty() {
            tag(&mut o, 3, "MaterialLabel", &r.material_label);
        }
        if !r.email.is_empty() {
            tag(&mut o, 3, "EmailAddress", &r.email);
        }
        tag(&mut o, 3, "MaxUnits", &format!("{:.2}", r.max_units));
        tag(&mut o, 3, "StandardRate", &format!("{:.2}", r.std_rate));
        tag(&mut o, 3, "OvertimeRate", &format!("{:.2}", r.ovt_rate));
        tag(&mut o, 3, "CostPerUse", &cents(r.cost_per_use));
        tag(&mut o, 3, "Work", &xdur(r.work));
        tag(&mut o, 3, "Cost", &cents(r.cost));
        tag(&mut o, 3, "OverAllocated", if r.overallocated { "1" } else { "0" });
        o.push_str("  </Resource>\n");
    }
    o.push_str(" </Resources>\n <Assignments>\n");
    for a in &p.assignments {
        o.push_str("  <Assignment>\n");
        tag(&mut o, 3, "UID", &a.uid.to_string());
        tag(&mut o, 3, "TaskUID", &a.task.to_string());
        tag(&mut o, 3, "ResourceUID", &a.resource.to_string());
        tag(&mut o, 3, "Units", &format!("{}", a.units));
        tag(&mut o, 3, "Work", &xdur(a.work));
        tag(&mut o, 3, "Cost", &cents(if a.cost_amount != 0.0 { a.cost_amount } else { a.cost }));
        o.push_str("  </Assignment>\n");
    }
    o.push_str(" </Assignments>\n</Project>\n");
    o
}

/// A tiny XML element tree (enough for the interchange format: elements and text).
#[derive(Debug, Default, Clone)]
pub struct El {
    pub name: String,
    pub text: String,
    pub children: Vec<El>,
}

impl El {
    pub fn child(&self, name: &str) -> Option<&El> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    pub fn s(&self, name: &str) -> Option<String> {
        self.child(name).map(|c| c.text.trim().to_string()).filter(|t| !t.is_empty())
    }
    pub fn i(&self, name: &str) -> Option<i64> {
        self.s(name).and_then(|t| t.parse::<f64>().ok()).map(|f| f.round() as i64)
    }
    pub fn f(&self, name: &str) -> Option<f64> {
        self.s(name).and_then(|t| t.parse().ok())
    }
    pub fn b(&self, name: &str) -> Option<bool> {
        self.s(name).map(|t| t == "1" || t.eq_ignore_ascii_case("true"))
    }
}

fn unesc(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#13;", "\r").replace("&#10;", "\n").replace("&amp;", "&")
}

pub fn parse_xml(text: &str) -> Result<El, String> {
    let mut stack: Vec<El> = vec![El { name: "#root".into(), ..Default::default() }];
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if text[i..].starts_with("<!--") {
                i = text[i..].find("-->").map(|k| i + k + 3).ok_or("unclosed comment")?;
                continue;
            }
            if text[i..].starts_with("<![CDATA[") {
                let end = text[i..].find("]]>").map(|k| i + k).ok_or("unclosed CDATA")?;
                stack.last_mut().unwrap().text.push_str(&text[i + 9..end]);
                i = end + 3;
                continue;
            }
            let end = text[i..].find('>').map(|k| i + k).ok_or("unclosed tag")?;
            let inner = &text[i + 1..end];
            i = end + 1;
            if inner.starts_with('?') || inner.starts_with('!') {
                continue;
            }
            if let Some(name) = inner.strip_prefix('/') {
                let el = stack.pop().ok_or("unbalanced XML")?;
                if el.name != name.trim() {
                    return Err(format!("XML tag <{}> closed by </{}>", el.name, name.trim()));
                }
                stack.last_mut().ok_or("unbalanced XML")?.children.push(el);
                continue;
            }
            let self_close = inner.ends_with('/');
            let inner = inner.trim_end_matches('/');
            let name = inner.split_whitespace().next().unwrap_or("").to_string();
            let el = El { name, ..Default::default() };
            if self_close {
                stack.last_mut().unwrap().children.push(el);
            } else {
                stack.push(el);
            }
        } else {
            let end = text[i..].find('<').map(|k| i + k).unwrap_or(bytes.len());
            stack.last_mut().unwrap().text.push_str(&unesc(&text[i..end]));
            i = end;
        }
    }
    if stack.len() != 1 {
        return Err("XML ended before all tags were closed".into());
    }
    stack.pop().unwrap().children.into_iter().next().ok_or_else(|| "empty XML".into())
}

fn pdate(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S").ok().or_else(|| parse_date(s, 480).ok())
}

/// "PT40H0M0S" -> minutes.
fn pdur(s: &str) -> i64 {
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let mut total = 0f64;
    let mut num = String::new();
    let mut in_time = false;
    for c in s.chars() {
        match c {
            'P' => {}
            'T' => in_time = true,
            '0'..='9' | '.' => num.push(c),
            'H' => {
                total += num.parse::<f64>().unwrap_or(0.0) * 60.0;
                num.clear()
            }
            'M' if in_time => {
                total += num.parse::<f64>().unwrap_or(0.0);
                num.clear()
            }
            'S' => {
                total += num.parse::<f64>().unwrap_or(0.0) / 60.0;
                num.clear()
            }
            'D' => {
                total += num.parse::<f64>().unwrap_or(0.0) * 1440.0;
                num.clear()
            }
            _ => num.clear(),
        }
    }
    let m = total.round() as i64;
    if neg {
        -m
    } else {
        m
    }
}

fn ptime(s: &str) -> u32 {
    let mut it = s.trim().split(':');
    let h: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let m: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    h * 60 + m
}

/// Money in the XML interchange format is written in cents (the desktop program's own exports
/// show a $429,744.00 plan as 42974400).
fn cents(amount: f64) -> String {
    format!("{:.0}", amount * 100.0)
}

pub fn import_xml(text: &str) -> Result<Project, String> {
    let root = parse_xml(text)?;
    if root.name != "Project" {
        return Err("this XML file is not a project (no <Project> element)".into());
    }
    let start = root.s("StartDate").and_then(|s| pdate(&s)).unwrap_or_else(|| chrono::Local::now().naive_local());
    let mut p = Project::new(&root.s("Title").or_else(|| root.s("Name")).unwrap_or_else(|| "Imported Project".into()), start.date());
    p.start = start;
    p.subject = root.s("Subject").unwrap_or_default();
    p.company = root.s("Company").unwrap_or_default();
    p.manager = root.s("Manager").unwrap_or_default();
    p.status_date = root.s("StatusDate").and_then(|s| pdate(&s));
    if let Some(v) = root.i("MinutesPerDay") {
        p.settings.minutes_per_day = v.max(1);
    }
    if let Some(v) = root.i("MinutesPerWeek") {
        p.settings.minutes_per_week = v.max(1);
    }
    if let Some(v) = root.i("DaysPerMonth") {
        p.settings.days_per_month = v.max(1);
    }
    if let Some(v) = root.s("DefaultStartTime") {
        p.settings.default_start_min = ptime(&v);
    }
    if let Some(v) = root.s("DefaultFinishTime") {
        p.settings.default_finish_min = ptime(&v);
    }
    if let Some(v) = root.s("CurrencySymbol") {
        p.settings.currency = v;
    }
    // calendars
    let mut cal_names: Vec<(i64, String)> = vec![];
    if let Some(cals) = root.child("Calendars") {
        let mut list = vec![];
        for c in cals.all("Calendar") {
            let name = c.s("Name").unwrap_or_else(|| "Calendar".into());
            cal_names.push((c.i("UID").unwrap_or(0), name.clone()));
            if c.b("IsBaseCalendar") == Some(false) {
                continue;
            }
            let mut cal = Calendar::standard();
            cal.name = name;
            if let Some(wd) = c.child("WeekDays") {
                for d in wd.all("WeekDay") {
                    let dt = d.i("DayType").unwrap_or(0);
                    let shifts: Vec<(u32, u32)> = d
                        .child("WorkingTimes")
                        .map(|w| {
                            w.all("WorkingTime")
                                .map(|x| {
                                    let a = ptime(&x.s("FromTime").unwrap_or_default());
                                    let mut b = ptime(&x.s("ToTime").unwrap_or_default());
                                    if b <= a {
                                        b = 1440;
                                    }
                                    (a, b)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let working = d.b("DayWorking").unwrap_or(false);
                    if (1..=7).contains(&dt) {
                        cal.week[(dt - 1) as usize] = if working { shifts } else { vec![] };
                    } else if dt == 0 {
                        if let Some(tp) = d.child("TimePeriod") {
                            if let (Some(a), Some(b)) = (tp.s("FromDate").and_then(|s| pdate(&s)), tp.s("ToDate").and_then(|s| pdate(&s))) {
                                cal.exceptions.push(CalException { name: "Exception".into(), start: a.date(), end: b.date(), shifts: if working { shifts } else { vec![] } });
                            }
                        }
                    }
                }
            }
            if let Some(ex) = c.child("Exceptions") {
                for e in ex.all("Exception") {
                    if let Some(tp) = e.child("TimePeriod") {
                        if let (Some(a), Some(b)) = (tp.s("FromDate").and_then(|s| pdate(&s)), tp.s("ToDate").and_then(|s| pdate(&s))) {
                            let working = e.b("DayWorking").unwrap_or(false);
                            let shifts = e
                                .child("WorkingTimes")
                                .map(|w| w.all("WorkingTime").map(|x| (ptime(&x.s("FromTime").unwrap_or_default()), ptime(&x.s("ToTime").unwrap_or_default()).max(1))).collect())
                                .unwrap_or_default();
                            cal.exceptions.push(CalException { name: e.s("Name").unwrap_or_else(|| "Exception".into()), start: a.date(), end: b.date(), shifts: if working { shifts } else { vec![] } });
                        }
                    }
                }
            }
            list.push(cal);
        }
        if !list.is_empty() {
            p.calendars = list;
        }
    }
    let cal_name = |uid: i64| cal_names.iter().find(|(u, _)| *u == uid).map(|(_, n)| n.clone());
    if let Some(cu) = root.i("CalendarUID") {
        if let Some(n) = cal_name(cu) {
            if p.calendar_named(&n).is_some() {
                p.calendar = n;
            }
        }
    }
    if p.calendar_named(&p.calendar).is_none() {
        p.calendar = p.calendars[0].name.clone();
    }
    let settings = p.settings.clone();
    if let Some(tasks) = root.child("Tasks") {
        for t in tasks.all("Task") {
            if t.b("IsNull") == Some(true) {
                continue;
            }
            let level = t.i("OutlineLevel").unwrap_or(1);
            if level == 0 {
                continue; // project summary task
            }
            let uid = t.i("UID").unwrap_or(0) as u32;
            let fmt = t.i("DurationFormat").unwrap_or(7) as i32;
            let dur_min = t.s("Duration").map(|d| pdur(&d)).unwrap_or(0);
            let mut task = Task::new(uid, &t.s("Name").unwrap_or_default(), Dur { minutes: dur_min, unit: DurUnit::from_code(fmt & 0x1f), elapsed: matches!(fmt & 0x1f, 4 | 6 | 8 | 10 | 12), estimated: t.b("Estimated").unwrap_or(false) });
            task.outline_level = level.clamp(1, 9) as u8;
            task.manual = t.b("Manual").unwrap_or(false);
            task.active = t.b("Active").unwrap_or(true);
            task.task_type = TaskType::ALL.get(t.i("Type").unwrap_or(0) as usize).copied().unwrap_or_default();
            task.effort_driven = t.b("EffortDriven").unwrap_or(false);
            task.milestone = t.b("Milestone").unwrap_or(false) && dur_min == 0;
            task.priority = t.i("Priority").unwrap_or(500).clamp(0, 1000) as u16;
            task.percent_complete = t.f("PercentComplete").unwrap_or(0.0);
            task.fixed_cost = t.f("FixedCost").unwrap_or(0.0) / 100.0;
            task.actual_start = t.s("ActualStart").and_then(|s| pdate(&s));
            task.actual_finish = t.s("ActualFinish").and_then(|s| pdate(&s));
            task.constraint = ConstraintKind::from_code(t.i("ConstraintType").unwrap_or(0) as i32);
            task.constraint_date = t.s("ConstraintDate").and_then(|s| pdate(&s));
            if !task.constraint.needs_date() {
                task.constraint_date = None;
            }
            task.deadline = t.s("Deadline").and_then(|s| pdate(&s));
            task.notes = t.s("Notes").unwrap_or_default();
            task.hyperlink = t.s("Hyperlink").unwrap_or_default();
            task.calendar = t.i("CalendarUID").filter(|u| *u > 0).and_then(cal_name);
            if task.manual {
                task.manual_start = t.s("Start").and_then(|s| pdate(&s));
            }
            for l in t.all("PredecessorLink") {
                let lag = l.i("LinkLag").unwrap_or(0) / 10;
                task.predecessors.push(Link {
                    pred: l.i("PredecessorUID").unwrap_or(0) as u32,
                    kind: LinkKind::from_xml_code(l.i("Type").unwrap_or(1) as i32),
                    lag: Dur { minutes: lag, unit: DurUnit::from_code(l.i("LagFormat").unwrap_or(7) as i32), ..Default::default() },
                });
            }
            task.baselines = vec![None; 11];
            for bl in t.all("Baseline") {
                let n = bl.i("Number").unwrap_or(0).clamp(0, 10) as usize;
                task.baselines[n] = Some(Baseline {
                    start: bl.s("Start").and_then(|s| pdate(&s)),
                    finish: bl.s("Finish").and_then(|s| pdate(&s)),
                    duration: Dur { minutes: bl.s("Duration").map(|d| pdur(&d)).unwrap_or(0), unit: settings.duration_unit, ..Default::default() },
                    work: bl.s("Work").map(|d| pdur(&d)).unwrap_or(0),
                    cost: bl.f("Cost").unwrap_or(0.0) / 100.0,
                });
            }
            p.tasks.push(task);
        }
    }
    // drop links to tasks that aren't there (e.g. the project summary)
    let uids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for t in p.tasks.iter_mut() {
        t.predecessors.retain(|l| uids.contains(&l.pred) && l.pred != t.uid);
    }
    if let Some(res) = root.child("Resources") {
        for r in res.all("Resource") {
            let uid = r.i("UID").unwrap_or(0) as u32;
            let name = match r.s("Name") {
                Some(n) => n,
                None => continue,
            };
            let mut x = Resource::new(uid, &name);
            x.kind = match r.i("Type").unwrap_or(1) {
                0 => ResourceKind::Material,
                2 => ResourceKind::Cost,
                _ => ResourceKind::Work,
            };
            if r.b("IsCostResource") == Some(true) {
                x.kind = ResourceKind::Cost;
            }
            x.initials = r.s("Initials").unwrap_or(x.initials);
            x.group = r.s("Group").unwrap_or_default();
            x.material_label = r.s("MaterialLabel").unwrap_or_default();
            x.email = r.s("EmailAddress").unwrap_or_default();
            x.max_units = r.f("MaxUnits").unwrap_or(1.0);
            x.std_rate = r.f("StandardRate").unwrap_or(0.0);
            x.ovt_rate = r.f("OvertimeRate").unwrap_or(0.0);
            x.cost_per_use = r.f("CostPerUse").unwrap_or(0.0) / 100.0;
            p.resources.push(x);
        }
    }
    if let Some(asg) = root.child("Assignments") {
        for a in asg.all("Assignment") {
            let task = a.i("TaskUID").unwrap_or(-1);
            let res = a.i("ResourceUID").unwrap_or(-1);
            if task < 0 || res < 0 || !uids.contains(&(task as u32)) || p.resource_by_uid(res as u32).is_none() {
                continue;
            }
            let kind = p.resource_by_uid(res as u32).unwrap().kind;
            let au = p.alloc_asg_uid();
            p.assignments.push(Assignment {
                uid: a.i("UID").map(|u| u as u32).unwrap_or(au),
                task: task as u32,
                resource: res as u32,
                units: a.f("Units").unwrap_or(1.0),
                work: a.s("Work").map(|d| pdur(&d)).unwrap_or(0),
                cost_amount: if kind == ResourceKind::Cost { a.f("Cost").unwrap_or(0.0) / 100.0 } else { 0.0 },
                cost: 0.0,
            });
        }
    }
    p.next_uid = p.tasks.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
    p.next_res_uid = p.resources.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
    p.next_asg_uid = p.assignments.iter().map(|t| t.uid).max().unwrap_or(0) + 1;
    Ok(p)
}

// ---------------------------------------------------------------------------------------------
// SVG Gantt chart
// ---------------------------------------------------------------------------------------------

fn rgb(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn gantt_svg(s: &Session) -> String {
    let p = &s.project;
    let rows = s.task_rows();
    let row_h = 24.0;
    let table_w = 360.0;
    let header_h = 44.0;
    let start = p.cal().snap_forward(p.start).date() - Duration::days(2);
    let end = p.finish.date() + Duration::days(4);
    let days = (end - start).num_days().max(7) as f64;
    let ppd = (900.0 / days).clamp(4.0, 40.0);
    let chart_w = days * ppd;
    let w = table_w + chart_w;
    let h = header_h + rows.len() as f64 * row_h + 10.0;
    let bc = &s.view.bar_colors;
    let x_of = |d: NaiveDateTime| table_w + (d - start.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as f64 / 1440.0 * ppd;
    let mut o = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"Segoe UI, Arial, sans-serif\" font-size=\"11\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n"
    );
    // nonworking shading and day grid
    let cal = p.cal();
    let mut d = start;
    while d < end {
        let x = x_of(d.and_hms_opt(0, 0, 0).unwrap());
        if !cal.is_working_day(d) {
            o.push_str(&format!("<rect x=\"{x:.1}\" y=\"{header_h}\" width=\"{ppd:.1}\" height=\"{:.0}\" fill=\"#eeeeee\"/>\n", h - header_h));
        }
        if d.weekday().num_days_from_monday() == 0 {
            o.push_str(&format!("<line x1=\"{x:.1}\" y1=\"0\" x2=\"{x:.1}\" y2=\"{h:.0}\" stroke=\"#d0d0d0\"/>\n"));
            o.push_str(&format!("<text x=\"{:.1}\" y=\"16\" fill=\"#333\">{}</text>\n", x + 3.0, esc(&d.format("%b %-d, '%y").to_string())));
        }
        if ppd >= 14.0 {
            o.push_str(&format!("<text x=\"{:.1}\" y=\"36\" fill=\"#666\" font-size=\"10\">{}</text>\n", x + ppd / 2.0 - 3.0, &d.format("%a").to_string()[..1]));
        }
        d += Duration::days(1);
    }
    o.push_str(&format!("<line x1=\"0\" y1=\"{header_h}\" x2=\"{w:.0}\" y2=\"{header_h}\" stroke=\"#999\"/>\n"));
    o.push_str(&format!("<line x1=\"{table_w}\" y1=\"0\" x2=\"{table_w}\" y2=\"{h:.0}\" stroke=\"#999\"/>\n"));
    o.push_str(&format!("<text x=\"6\" y=\"30\" font-weight=\"bold\">ID</text><text x=\"34\" y=\"30\" font-weight=\"bold\">Task Name</text><text x=\"270\" y=\"30\" font-weight=\"bold\">Duration</text>\n"));
    let mut row_y = std::collections::HashMap::new();
    for (k, r) in rows.iter().enumerate() {
        let y = header_h + k as f64 * row_h;
        match r {
            Row::Group(g) => {
                o.push_str(&format!("<rect x=\"0\" y=\"{y}\" width=\"{table_w}\" height=\"{row_h}\" fill=\"#fff3c4\"/><text x=\"6\" y=\"{:.0}\" font-weight=\"bold\">{}</text>\n", y + 16.0, esc(g)));
            }
            Row::ProjectSummary => {
                o.push_str(&format!("<text x=\"34\" y=\"{:.0}\" font-weight=\"bold\">{}</text>\n", y + 16.0, esc(&p.title)));
            }
            Row::Task(i) => {
                let t = &p.tasks[*i];
                row_y.insert(t.uid, y);
                let indent = (t.outline_level as f64 - 1.0) * 12.0;
                o.push_str(&format!(
                    "<text x=\"6\" y=\"{:.0}\" fill=\"#555\">{}</text><text x=\"{:.0}\" y=\"{:.0}\"{}>{}</text><text x=\"270\" y=\"{:.0}\">{}</text>\n",
                    y + 16.0,
                    i + 1,
                    34.0 + indent,
                    y + 16.0,
                    if t.summary { " font-weight=\"bold\"" } else { "" },
                    esc(&t.name),
                    y + 16.0,
                    esc(&t.duration.format(&p.settings))
                ));
                let x0 = x_of(t.start);
                let x1 = x_of(t.finish).max(x0 + 1.0);
                if t.summary {
                    o.push_str(&format!("<rect x=\"{x0:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"5\" fill=\"{}\"/>\n", y + 8.0, x1 - x0, rgb(bc.summary)));
                    o.push_str(&format!("<polygon points=\"{x0:.1},{:.1} {:.1},{:.1} {x0:.1},{:.1}\" fill=\"{}\"/>\n", y + 8.0, x0 + 5.0, y + 13.0, y + 18.0, rgb(bc.summary)));
                    o.push_str(&format!("<polygon points=\"{x1:.1},{:.1} {:.1},{:.1} {x1:.1},{:.1}\" fill=\"{}\"/>\n", y + 8.0, x1 - 5.0, y + 13.0, y + 18.0, rgb(bc.summary)));
                } else if t.is_milestone() {
                    let cx = x0;
                    let cy = y + 12.0;
                    o.push_str(&format!("<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"{}\"/>\n", cx, cy - 6.0, cx + 6.0, cy, cx, cy + 6.0, cx - 6.0, cy, rgb(bc.milestone)));
                    o.push_str(&format!("<text x=\"{:.1}\" y=\"{:.0}\" fill=\"#333\">{}</text>\n", cx + 9.0, y + 16.0, esc(&fmt_date_short(t.start))));
                } else {
                    let col = t.bar_color.unwrap_or(if s.view.show_critical && t.critical { bc.critical } else if t.manual { bc.manual } else { bc.task });
                    o.push_str(&format!("<rect x=\"{x0:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"12\" rx=\"2\" fill=\"{}\" stroke=\"#4a6c96\" stroke-width=\"0.6\"/>\n", y + 6.0, x1 - x0, rgb(col)));
                    if t.percent_complete > 0.0 {
                        o.push_str(&format!("<rect x=\"{x0:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"4\" fill=\"{}\"/>\n", y + 10.0, (x1 - x0) * t.percent_complete / 100.0, rgb(bc.progress)));
                    }
                    let names = p.resource_names(t.uid);
                    if !names.is_empty() {
                        o.push_str(&format!("<text x=\"{:.1}\" y=\"{:.0}\" fill=\"#333\">{}</text>\n", x1 + 4.0, y + 16.0, esc(&names)));
                    }
                }
            }
        }
    }
    // link lines (finish-to-start style elbows)
    for t in &p.tasks {
        let Some(&ys) = row_y.get(&t.uid) else { continue };
        for l in &t.predecessors {
            let (Some(&yp), Some(pt)) = (row_y.get(&l.pred), p.task_by_uid(l.pred)) else { continue };
            let xf = x_of(pt.finish);
            let xs = x_of(t.start);
            let xa = (xs + 3.0).max(xf + 4.0);
            let (y0, y1) = if ys > yp { (yp + 12.0, ys + 6.0) } else { (yp + 12.0, ys + 18.0) };
            let tip = if ys > yp { -1.0 } else { 1.0 };
            o.push_str(&format!(
                "<polyline points=\"{xf:.1},{y0:.1} {xa:.1},{y0:.1} {xa:.1},{y1:.1}\" fill=\"none\" stroke=\"#3c5a82\" stroke-width=\"0.8\"/>\n<polygon points=\"{:.1},{:.1} {:.1},{:.1} {xa:.1},{y1:.1}\" fill=\"#3c5a82\"/>\n",
                xa - 3.0,
                y1 + tip * 4.0,
                xa + 3.0,
                y1 + tip * 4.0
            ));
        }
    }
    o.push_str("</svg>\n");
    o
}

// ---------------------------------------------------------------------------------------------
// HTML / Markdown
// ---------------------------------------------------------------------------------------------

pub fn markdown_report(s: &Session) -> String {
    let p = &s.project;
    let o = reports::overview(p);
    let mut out = format!("# {}\n\n", p.title);
    for k in ["start", "finish", "duration", "percentComplete", "workHours", "cost", "tasks", "resources", "criticalTasks", "lateTasks"] {
        out.push_str(&format!("- **{}**: {}\n", k, fields::value_as_text(&o[k])));
    }
    out.push_str("\n## Tasks\n\n| ID | Task | Duration | Start | Finish | % | Resources |\n|---|---|---|---|---|---|---|\n");
    for (i, t) in p.tasks.iter().enumerate() {
        out.push_str(&format!(
            "| {} | {}{} | {} | {} | {} | {}% | {} |\n",
            i + 1,
            "&nbsp;&nbsp;".repeat(t.outline_level.saturating_sub(1) as usize),
            if t.summary { format!("**{}**", t.name) } else { t.name.clone() },
            t.duration.format(&p.settings),
            fmt_date(t.start),
            fmt_date(t.finish),
            fmt_num(t.percent_complete),
            p.resource_names(t.uid)
        ));
    }
    if !p.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for w in &p.warnings {
            out.push_str(&format!("- {w}\n"));
        }
    }
    out
}

pub fn html_report(s: &Session) -> String {
    let p = &s.project;
    let mut rows = String::new();
    for (i, t) in p.tasks.iter().enumerate() {
        rows.push_str(&format!(
            "<tr class=\"{}\"><td>{}</td><td style=\"padding-left:{}px\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}%</td><td>{}</td></tr>\n",
            if t.summary { "sum" } else if t.critical { "crit" } else { "" },
            i + 1,
            6 + (t.outline_level as usize - 1) * 16,
            esc(&t.name),
            esc(&t.duration.format(&p.settings)),
            esc(&fmt_date(t.start)),
            esc(&fmt_date(t.finish)),
            fmt_num(t.percent_complete),
            esc(&p.resource_names(t.uid))
        ));
    }
    let o = reports::overview(p);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title><style>body{{font-family:Segoe UI,Arial,sans-serif;margin:24px;color:#222}}h1{{color:#2b6a3f}}table{{border-collapse:collapse}}td,th{{border:1px solid #ddd;padding:4px 8px;font-size:13px}}th{{background:#eef3ee;text-align:left}}tr.sum td{{font-weight:600}}tr.crit td{{color:#b03030}}.kpi{{display:inline-block;margin:0 24px 16px 0}}.kpi b{{display:block;font-size:20px}}</style></head><body><h1>{title}</h1>\
<div class=\"kpi\">Start<b>{start}</b></div><div class=\"kpi\">Finish<b>{finish}</b></div><div class=\"kpi\">Duration<b>{dur}</b></div><div class=\"kpi\">Complete<b>{pct}%</b></div><div class=\"kpi\">Cost<b>{cost}</b></div>\
<h2>Gantt chart</h2>{svg}<h2>Tasks</h2><table><tr><th>ID</th><th>Task</th><th>Duration</th><th>Start</th><th>Finish</th><th>%</th><th>Resources</th></tr>{rows}</table></body></html>",
        title = esc(&p.title),
        start = esc(&fmt_date(p.cal().snap_forward(p.start))),
        finish = esc(&fmt_date(p.finish)),
        dur = esc(&fields::value_as_text(&o["duration"])),
        pct = fields::value_as_text(&o["percentComplete"]),
        cost = esc(&p.fmt_cost(o["cost"].as_f64().unwrap_or(0.0))),
        svg = gantt_svg(s),
        rows = rows
    )
}
