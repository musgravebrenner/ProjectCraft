//! The field catalog: every task and resource column, how it is shown, read as JSON and set.
//! Columns, setters, sorting, find/replace, exports and the command registry all use it.

use crate::model::*;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Duration,
    Date,
    Percent,
    Number,
    Integer,
    Flag,
    Choice,
    Cost,
    Work,
}

#[derive(Clone, Copy, Debug)]
pub struct FieldDef {
    pub id: &'static str,
    pub title: &'static str,
    pub kind: FieldKind,
    pub editable: bool,
    pub width: f32,
}

const fn f(id: &'static str, title: &'static str, kind: FieldKind, editable: bool, width: f32) -> FieldDef {
    FieldDef { id, title, kind, editable, width }
}

use FieldKind::*;

pub const TASK_FIELDS: &[FieldDef] = &[
    f("id", "ID", Integer, false, 34.0),
    f("indicators", "Indicators", Text, false, 44.0),
    f("uid", "Unique ID", Integer, false, 60.0),
    f("mode", "Task Mode", Choice, true, 46.0),
    f("name", "Task Name", Text, true, 210.0),
    f("duration", "Duration", Duration, true, 72.0),
    f("start", "Start", Date, true, 96.0),
    f("finish", "Finish", Date, true, 96.0),
    f("predecessors", "Predecessors", Text, true, 90.0),
    f("successors", "Successors", Text, false, 90.0),
    f("resourceNames", "Resource Names", Text, true, 140.0),
    f("work", "Work", Work, true, 70.0),
    f("cost", "Cost", Cost, false, 84.0),
    f("fixedCost", "Fixed Cost", Cost, true, 80.0),
    f("percentComplete", "% Complete", Percent, true, 74.0),
    f("actualStart", "Actual Start", Date, true, 96.0),
    f("actualFinish", "Actual Finish", Date, true, 96.0),
    f("actualDuration", "Actual Duration", Duration, false, 90.0),
    f("remainingDuration", "Remaining Duration", Duration, false, 110.0),
    f("baselineStart", "Baseline Start", Date, false, 96.0),
    f("baselineFinish", "Baseline Finish", Date, false, 96.0),
    f("baselineDuration", "Baseline Duration", Duration, false, 100.0),
    f("baselineWork", "Baseline Work", Work, false, 90.0),
    f("baselineCost", "Baseline Cost", Cost, false, 90.0),
    f("startVariance", "Start Variance", Duration, false, 90.0),
    f("finishVariance", "Finish Variance", Duration, false, 90.0),
    f("earlyStart", "Early Start", Date, false, 96.0),
    f("earlyFinish", "Early Finish", Date, false, 96.0),
    f("lateStart", "Late Start", Date, false, 96.0),
    f("lateFinish", "Late Finish", Date, false, 96.0),
    f("totalSlack", "Total Slack", Duration, false, 80.0),
    f("freeSlack", "Free Slack", Duration, false, 80.0),
    f("critical", "Critical", Flag, false, 60.0),
    f("constraintType", "Constraint Type", Choice, true, 150.0),
    f("constraintDate", "Constraint Date", Date, true, 96.0),
    f("deadline", "Deadline", Date, true, 96.0),
    f("taskType", "Type", Choice, true, 100.0),
    f("effortDriven", "Effort Driven", Flag, true, 80.0),
    f("priority", "Priority", Integer, true, 60.0),
    f("milestone", "Milestone", Flag, true, 66.0),
    f("summary", "Summary", Flag, false, 66.0),
    f("outlineLevel", "Outline Level", Integer, false, 80.0),
    f("outlineNumber", "Outline Number", Text, false, 90.0),
    f("wbs", "WBS", Text, false, 60.0),
    f("calendar", "Task Calendar", Text, true, 100.0),
    f("active", "Active", Flag, true, 56.0),
    f("estimated", "Estimated", Flag, true, 66.0),
    f("onTimeline", "Display on Timeline", Flag, true, 90.0),
    f("hideBar", "Hide Bar", Flag, true, 60.0),
    f("notes", "Notes", Text, true, 160.0),
    f("hyperlink", "Hyperlink", Text, true, 120.0),
    f("text1", "Text1", Text, true, 90.0),
    f("text2", "Text2", Text, true, 90.0),
    f("text3", "Text3", Text, true, 90.0),
    f("text4", "Text4", Text, true, 90.0),
    f("text5", "Text5", Text, true, 90.0),
    f("number1", "Number1", Number, true, 70.0),
    f("number2", "Number2", Number, true, 70.0),
    f("number3", "Number3", Number, true, 70.0),
    f("number4", "Number4", Number, true, 70.0),
    f("number5", "Number5", Number, true, 70.0),
    f("flag1", "Flag1", Flag, true, 56.0),
    f("flag2", "Flag2", Flag, true, 56.0),
    f("flag3", "Flag3", Flag, true, 56.0),
    f("flag4", "Flag4", Flag, true, 56.0),
    f("flag5", "Flag5", Flag, true, 56.0),
];

pub const RESOURCE_FIELDS: &[FieldDef] = &[
    f("id", "ID", Integer, false, 34.0),
    f("name", "Resource Name", Text, true, 160.0),
    f("type", "Type", Choice, true, 74.0),
    f("materialLabel", "Material Label", Text, true, 90.0),
    f("initials", "Initials", Text, true, 60.0),
    f("group", "Group", Text, true, 90.0),
    f("maxUnits", "Max. Units", Percent, true, 76.0),
    f("stdRate", "Std. Rate", Cost, true, 86.0),
    f("ovtRate", "Ovt. Rate", Cost, true, 86.0),
    f("costPerUse", "Cost/Use", Cost, true, 76.0),
    f("calendar", "Base Calendar", Text, true, 96.0),
    f("code", "Code", Text, true, 60.0),
    f("email", "Email Address", Text, true, 140.0),
    f("work", "Work", Work, false, 80.0),
    f("cost", "Cost", Cost, false, 90.0),
    f("peak", "Peak", Percent, false, 60.0),
    f("overallocated", "Overallocated", Flag, false, 86.0),
    f("notes", "Notes", Text, true, 140.0),
    f("text1", "Text1", Text, true, 90.0),
    f("text2", "Text2", Text, true, 90.0),
    f("text3", "Text3", Text, true, 90.0),
];

pub fn task_field(id: &str) -> Option<&'static FieldDef> {
    TASK_FIELDS.iter().find(|f| f.id.eq_ignore_ascii_case(id))
}
pub fn resource_field(id: &str) -> Option<&'static FieldDef> {
    RESOURCE_FIELDS.iter().find(|f| f.id.eq_ignore_ascii_case(id))
}

fn idx_suffix(id: &str, prefix: &str) -> Option<usize> {
    id.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=5).contains(n)).map(|n| n - 1)
}

fn opt_date(d: Option<chrono::NaiveDateTime>) -> String {
    d.map(fmt_date).unwrap_or_else(|| "NA".into())
}
fn opt_date_json(d: Option<chrono::NaiveDateTime>) -> Value {
    d.map(|d| json!(fmt_datetime(d))).unwrap_or(Value::Null)
}

fn dur_of(minutes: i64, s: &Settings) -> Dur {
    Dur { minutes, unit: s.duration_unit, elapsed: false, estimated: false }
}

/// Text shown in a cell.
pub fn task_text(p: &Project, i: usize, field: &str) -> String {
    let t = &p.tasks[i];
    let s = &p.settings;
    match field {
        "id" => (i + 1).to_string(),
        "indicators" => {
            // what the icons in the Indicators column mean, as text (for exports and agents)
            let mut v: Vec<&str> = vec![];
            if t.percent_complete >= 100.0 && !t.summary {
                v.push("complete");
            }
            if !t.notes.is_empty() {
                v.push("notes");
            }
            if t.missed_deadline || t.total_slack < 0 {
                v.push("conflict");
            } else if t.constraint.needs_date() {
                v.push("constraint");
            }
            if !t.active {
                v.push("inactive");
            }
            v.join(", ")
        }
        "uid" => t.uid.to_string(),
        "mode" => if t.manual { "Manually Scheduled".into() } else { "Auto Scheduled".into() },
        "name" => t.name.clone(),
        "duration" => t.duration.format(s),
        "start" => fmt_date(t.start),
        "finish" => fmt_date(t.finish),
        "predecessors" => p.predecessors_text(t),
        "successors" => p.successors_text(t.uid),
        "resourceNames" => p.resource_names(t.uid),
        "work" => p.fmt_work(t.work),
        "cost" => p.fmt_cost(t.cost),
        "fixedCost" => p.fmt_cost(t.fixed_cost),
        "percentComplete" => format!("{}%", fmt_num(t.percent_complete)),
        "actualStart" => opt_date(t.actual_start.or(if t.percent_complete > 0.0 { Some(t.start) } else { None })),
        "actualFinish" => opt_date(t.actual_finish.or(if t.percent_complete >= 100.0 { Some(t.finish) } else { None })),
        "actualDuration" => {
            let d = Dur { minutes: (t.duration.minutes as f64 * t.percent_complete / 100.0).round() as i64, ..t.duration };
            d.format(s)
        }
        "remainingDuration" => {
            let d = Dur { minutes: (t.duration.minutes as f64 * (1.0 - t.percent_complete / 100.0)).round() as i64, ..t.duration };
            d.format(s)
        }
        "baselineStart" => opt_date(t.baseline(0).and_then(|b| b.start)),
        "baselineFinish" => opt_date(t.baseline(0).and_then(|b| b.finish)),
        "baselineDuration" => t.baseline(0).map(|b| b.duration.format(s)).unwrap_or_else(|| "0 days".into()),
        "baselineWork" => p.fmt_work(t.baseline(0).map(|b| b.work).unwrap_or(0)),
        "baselineCost" => p.fmt_cost(t.baseline(0).map(|b| b.cost).unwrap_or(0.0)),
        "startVariance" => dur_of(variance(p, i, true), s).format(s),
        "finishVariance" => dur_of(variance(p, i, false), s).format(s),
        "earlyStart" => fmt_date(t.early_start),
        "earlyFinish" => fmt_date(t.early_finish),
        "lateStart" => fmt_date(t.late_start),
        "lateFinish" => fmt_date(t.late_finish),
        "totalSlack" => dur_of(t.total_slack, s).format(s),
        "freeSlack" => dur_of(t.free_slack, s).format(s),
        "critical" => yes_no(t.critical),
        "constraintType" => t.constraint.title().into(),
        "constraintDate" => if t.constraint.needs_date() { opt_date(t.constraint_date) } else { "NA".into() },
        "deadline" => opt_date(t.deadline),
        "taskType" => t.task_type.title().into(),
        "effortDriven" => yes_no(t.effort_driven),
        "priority" => t.priority.to_string(),
        "milestone" => yes_no(t.is_milestone()),
        "summary" => yes_no(t.summary),
        "outlineLevel" => t.outline_level.to_string(),
        "outlineNumber" | "wbs" => p.outline_number(i),
        "calendar" => t.calendar.clone().unwrap_or_else(|| "None".into()),
        "active" => yes_no(t.active),
        "estimated" => yes_no(t.duration.estimated),
        "onTimeline" => yes_no(t.on_timeline),
        "hideBar" => yes_no(t.hide_bar),
        "notes" => t.notes.lines().next().unwrap_or("").to_string(),
        "hyperlink" => t.hyperlink.clone(),
        _ => {
            if let Some(k) = idx_suffix(field, "text") {
                t.text[k].clone()
            } else if let Some(k) = idx_suffix(field, "number") {
                fmt_num(t.number[k])
            } else if let Some(k) = idx_suffix(field, "flag") {
                yes_no(t.flag[k])
            } else {
                String::new()
            }
        }
    }
}

fn yes_no(b: bool) -> String {
    if b { "Yes".into() } else { "No".into() }
}

fn variance(p: &Project, i: usize, start: bool) -> i64 {
    let t = &p.tasks[i];
    let cal = p.cal();
    match t.baseline(0) {
        Some(b) => {
            let base = if start { b.start } else { b.finish };
            match base {
                Some(bd) => cal.work_between(bd, if start { t.start } else { t.finish }),
                None => 0,
            }
        }
        None => 0,
    }
}

/// Machine value (for JSON and sorting).
pub fn task_value(p: &Project, i: usize, field: &str) -> Value {
    let t = &p.tasks[i];
    match field {
        "id" => json!(i + 1),
        "uid" => json!(t.uid),
        "mode" => json!(if t.manual { "manual" } else { "auto" }),
        "duration" => json!({"minutes": t.duration.minutes, "text": t.duration.format(&p.settings), "days": t.duration.minutes as f64 / p.settings.minutes_per_day as f64}),
        "start" => json!(fmt_datetime(t.start)),
        "finish" => json!(fmt_datetime(t.finish)),
        "work" => json!({"minutes": t.work, "hours": t.work as f64 / 60.0}),
        "cost" => json!(t.cost),
        "fixedCost" => json!(t.fixed_cost),
        "percentComplete" => json!(t.percent_complete),
        "actualStart" => opt_date_json(t.actual_start),
        "actualFinish" => opt_date_json(t.actual_finish),
        "baselineStart" => opt_date_json(t.baseline(0).and_then(|b| b.start)),
        "baselineFinish" => opt_date_json(t.baseline(0).and_then(|b| b.finish)),
        "baselineCost" => json!(t.baseline(0).map(|b| b.cost).unwrap_or(0.0)),
        "baselineWork" => json!(t.baseline(0).map(|b| b.work).unwrap_or(0)),
        "baselineDuration" => json!(t.baseline(0).map(|b| b.duration.minutes).unwrap_or(0)),
        "startVariance" => json!(variance(p, i, true)),
        "finishVariance" => json!(variance(p, i, false)),
        "earlyStart" => json!(fmt_datetime(t.early_start)),
        "earlyFinish" => json!(fmt_datetime(t.early_finish)),
        "lateStart" => json!(fmt_datetime(t.late_start)),
        "lateFinish" => json!(fmt_datetime(t.late_finish)),
        "totalSlack" => json!(t.total_slack),
        "freeSlack" => json!(t.free_slack),
        "critical" => json!(t.critical),
        "constraintType" => json!(t.constraint.abbr()),
        "constraintDate" => opt_date_json(t.constraint_date),
        "deadline" => opt_date_json(t.deadline),
        "taskType" => json!(t.task_type.id()),
        "effortDriven" => json!(t.effort_driven),
        "priority" => json!(t.priority),
        "milestone" => json!(t.is_milestone()),
        "summary" => json!(t.summary),
        "outlineLevel" => json!(t.outline_level),
        "active" => json!(t.active),
        "estimated" => json!(t.duration.estimated),
        "onTimeline" => json!(t.on_timeline),
        "hideBar" => json!(t.hide_bar),
        "notes" => json!(t.notes),
        _ => {
            if let Some(k) = idx_suffix(field, "number") {
                json!(t.number[k])
            } else if let Some(k) = idx_suffix(field, "flag") {
                json!(t.flag[k])
            } else {
                json!(task_text(p, i, field))
            }
        }
    }
}

pub fn task_json(p: &Project, i: usize) -> Value {
    let mut m = serde_json::Map::new();
    for fd in TASK_FIELDS {
        if fd.id.starts_with("text") || fd.id.starts_with("number") || fd.id.starts_with("flag") {
            continue;
        }
        m.insert(fd.id.to_string(), task_value(p, i, fd.id));
    }
    Value::Object(m)
}

pub fn value_as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) != 0.0),
        Value::String(s) => match s.trim().to_lowercase().as_str() {
            "yes" | "true" | "1" | "on" | "y" => Ok(true),
            "no" | "false" | "0" | "off" | "n" | "" => Ok(false),
            o => Err(format!("'{o}' is not yes/no")),
        },
        _ => Err("expected yes/no".into()),
    }
}

fn parse_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => {
            let c: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            c.parse::<f64>().map_err(|_| format!("'{s}' is not a number"))
        }
        Value::Null => Ok(0.0),
        _ => Err("expected a number".into()),
    }
}

pub fn parse_percent(v: &Value) -> Result<f64, String> {
    let x = parse_f64(v)?;
    Ok(x.clamp(0.0, 100.0))
}

fn parse_opt_date(v: &Value, default_min: u32) -> Result<Option<chrono::NaiveDateTime>, String> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) if s.trim().is_empty() || s.trim().eq_ignore_ascii_case("na") => Ok(None),
        other => parse_date(&value_as_text(other), default_min).map(Some),
    }
}

pub fn parse_dur_value(v: &Value, s: &Settings) -> Result<Dur, String> {
    match v {
        Value::Number(n) => Ok(Dur { minutes: (n.as_f64().unwrap_or(0.0) * s.minutes_per_day as f64).round() as i64, unit: DurUnit::Days, elapsed: false, estimated: false }),
        Value::Object(o) if o.contains_key("minutes") => Ok(Dur { minutes: o["minutes"].as_i64().unwrap_or(0), unit: s.duration_unit, ..Default::default() }),
        other => Dur::parse(&value_as_text(other), s.duration_unit, s),
    }
}

pub fn parse_work_value(v: &Value, s: &Settings) -> Result<i64, String> {
    match v {
        Value::Number(n) => Ok((n.as_f64().unwrap_or(0.0) * 60.0).round() as i64),
        other => Ok(Dur::parse(&value_as_text(other), s.work_unit, s)?.minutes),
    }
}

/// Set a simple task field. Fields with scheduling side effects (duration, work, start, finish,
/// predecessors, resource names) are handled by the command layer, which calls back here for the rest.
pub fn set_task_simple(p: &mut Project, i: usize, field: &str, v: &Value) -> Result<(), String> {
    let s = p.settings.clone();
    let t = &mut p.tasks[i];
    match field {
        "name" => t.name = value_as_text(v),
        "mode" => {
            let txt = value_as_text(v).to_lowercase();
            let manual = txt.starts_with("manual") || txt == "true";
            if manual && !t.manual {
                t.manual_start = Some(t.start);
            }
            t.manual = manual;
        }
        "fixedCost" => t.fixed_cost = parse_f64(v)?,
        "percentComplete" => {
            let pc = parse_percent(v)?;
            t.percent_complete = pc;
            if pc > 0.0 && t.actual_start.is_none() {
                t.actual_start = Some(t.start);
            }
            if pc <= 0.0 {
                t.actual_start = None;
            }
            if pc >= 100.0 {
                t.actual_finish = Some(t.finish);
            } else {
                t.actual_finish = None;
            }
        }
        "actualStart" => {
            t.actual_start = parse_opt_date(v, s.default_start_min)?;
            if t.actual_start.is_some() && t.percent_complete <= 0.0 {
                t.percent_complete = 1.0;
            }
        }
        "actualFinish" => {
            t.actual_finish = parse_opt_date(v, s.default_finish_min)?;
            if t.actual_finish.is_some() {
                t.percent_complete = 100.0;
                if t.actual_start.is_none() {
                    t.actual_start = Some(t.start);
                }
            }
        }
        "constraintType" => {
            let c = ConstraintKind::parse(&value_as_text(v)).ok_or("unknown constraint type (ASAP, ALAP, MSO, MFO, SNET, SNLT, FNET, FNLT)")?;
            t.constraint = c;
            if c.needs_date() && t.constraint_date.is_none() {
                t.constraint_date = Some(match c {
                    ConstraintKind::MFO | ConstraintKind::FNET | ConstraintKind::FNLT => t.finish,
                    _ => t.start,
                });
            }
            if !c.needs_date() {
                t.constraint_date = None;
            }
        }
        "constraintDate" => {
            t.constraint_date = parse_opt_date(v, s.default_start_min)?;
            if t.constraint_date.is_some() && !t.constraint.needs_date() {
                t.constraint = ConstraintKind::SNET;
            }
        }
        "deadline" => t.deadline = parse_opt_date(v, s.default_finish_min)?,
        "taskType" => t.task_type = TaskType::parse(&value_as_text(v)).ok_or("unknown task type (fixedUnits, fixedDuration, fixedWork)")?,
        "effortDriven" => t.effort_driven = parse_bool(v)?,
        "priority" => t.priority = parse_f64(v)?.clamp(0.0, 1000.0) as u16,
        "milestone" => t.milestone = parse_bool(v)?,
        "calendar" => {
            let n = value_as_text(v);
            t.calendar = if n.is_empty() || n.eq_ignore_ascii_case("none") { None } else { Some(n) };
        }
        "active" => t.active = parse_bool(v)?,
        "estimated" => t.duration.estimated = parse_bool(v)?,
        "onTimeline" => t.on_timeline = parse_bool(v)?,
        "hideBar" => t.hide_bar = parse_bool(v)?,
        "notes" => t.notes = value_as_text(v),
        "hyperlink" => t.hyperlink = value_as_text(v),
        _ => {
            if let Some(k) = idx_suffix(field, "text") {
                t.text[k] = value_as_text(v);
            } else if let Some(k) = idx_suffix(field, "number") {
                t.number[k] = parse_f64(v)?;
            } else if let Some(k) = idx_suffix(field, "flag") {
                t.flag[k] = parse_bool(v)?;
            } else {
                return Err(format!("field '{field}' can't be set"));
            }
        }
    }
    if let Some(cal) = &p.tasks[i].calendar {
        if p.calendar_named(cal).is_none() {
            let c = cal.clone();
            p.tasks[i].calendar = None;
            return Err(format!("no calendar named '{c}'"));
        }
    }
    Ok(())
}

pub fn resource_text(p: &Project, i: usize, field: &str) -> String {
    let r = &p.resources[i];
    match field {
        "id" => (i + 1).to_string(),
        "name" => r.name.clone(),
        "type" => r.kind.title().into(),
        "materialLabel" => r.material_label.clone(),
        "initials" => r.initials.clone(),
        "group" => r.group.clone(),
        "maxUnits" => if r.kind == ResourceKind::Work { format!("{}%", fmt_num(r.max_units * 100.0)) } else { String::new() },
        "stdRate" => match r.kind {
            ResourceKind::Work => format!("{}/hr", p.fmt_cost(r.std_rate)),
            ResourceKind::Material => p.fmt_cost(r.std_rate),
            ResourceKind::Cost => String::new(),
        },
        "ovtRate" => if r.kind == ResourceKind::Work { format!("{}/hr", p.fmt_cost(r.ovt_rate)) } else { String::new() },
        "costPerUse" => if r.kind == ResourceKind::Cost { String::new() } else { p.fmt_cost(r.cost_per_use) },
        "calendar" => r.calendar.clone().unwrap_or_else(|| p.calendar.clone()),
        "code" => r.code.clone(),
        "email" => r.email.clone(),
        "work" => if r.kind == ResourceKind::Work { p.fmt_work(r.work) } else { String::new() },
        "cost" => p.fmt_cost(r.cost),
        "peak" => format!("{}%", fmt_num(r.peak * 100.0)),
        "overallocated" => yes_no(r.overallocated),
        "notes" => r.notes.lines().next().unwrap_or("").to_string(),
        "text1" => r.text[0].clone(),
        "text2" => r.text[1].clone(),
        "text3" => r.text[2].clone(),
        _ => String::new(),
    }
}

pub fn resource_json(p: &Project, i: usize) -> Value {
    let r = &p.resources[i];
    json!({
        "id": i + 1, "uid": r.uid, "name": r.name, "type": r.kind.title(), "initials": r.initials,
        "group": r.group, "maxUnits": r.max_units, "stdRate": r.std_rate, "ovtRate": r.ovt_rate,
        "costPerUse": r.cost_per_use, "calendar": resource_text(p, i, "calendar"), "email": r.email,
        "workHours": r.work as f64 / 60.0, "cost": r.cost, "peak": r.peak, "overallocated": r.overallocated,
        "materialLabel": r.material_label, "code": r.code,
    })
}

pub fn set_resource_field(p: &mut Project, i: usize, field: &str, v: &Value) -> Result<(), String> {
    let r = &mut p.resources[i];
    match field {
        "name" => {
            let n = value_as_text(v);
            if n.trim().is_empty() {
                return Err("a resource needs a name".into());
            }
            r.name = n
        }
        "type" => r.kind = ResourceKind::parse(&value_as_text(v)).ok_or("type must be Work, Material or Cost")?,
        "materialLabel" => r.material_label = value_as_text(v),
        "initials" => r.initials = value_as_text(v),
        "group" => r.group = value_as_text(v),
        "maxUnits" => {
            let x = parse_f64(v)?;
            // "100%" or 100 means 1.0; values <= 10 without % are taken as fractions
            let txt = value_as_text(v);
            r.max_units = if txt.contains('%') || x > 10.0 { x / 100.0 } else { x };
        }
        "stdRate" => r.std_rate = parse_f64(v)?,
        "ovtRate" => r.ovt_rate = parse_f64(v)?,
        "costPerUse" => r.cost_per_use = parse_f64(v)?,
        "calendar" => {
            let n = value_as_text(v);
            r.calendar = if n.is_empty() { None } else { Some(n) };
        }
        "code" => r.code = value_as_text(v),
        "email" => r.email = value_as_text(v),
        "notes" => r.notes = value_as_text(v),
        "text1" => r.text[0] = value_as_text(v),
        "text2" => r.text[1] = value_as_text(v),
        "text3" => r.text[2] = value_as_text(v),
        _ => return Err(format!("resource field '{field}' can't be set")),
    }
    Ok(())
}
