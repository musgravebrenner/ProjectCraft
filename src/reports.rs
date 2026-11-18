//! Built-in reports. Each returns a table: {"title", "summary", "columns", "rows"}.

use crate::model::*;
use crate::schedule;
use chrono::{Datelike, Duration, NaiveDateTime};
use serde_json::{json, Value};

pub const REPORTS: &[(&str, &str)] = &[
    ("projectOverview", "Project Overview"),
    ("criticalTasks", "Critical Tasks"),
    ("lateTasks", "Late Tasks"),
    ("milestones", "Milestone Report"),
    ("upcoming", "Upcoming Tasks"),
    ("costOverview", "Cost Overview"),
    ("cashFlow", "Cash Flow"),
    ("resourceOverview", "Resource Overview"),
    ("workOverview", "Work Overview"),
    ("overallocated", "Overallocated Resources"),
    ("slipping", "Slipping Tasks"),
    ("deadlines", "Deadlines"),
    ("notStarted", "Tasks Not Started"),
    ("completed", "Completed Tasks"),
];

fn table(title: &str, summary: Value, columns: &[&str], rows: Vec<Vec<String>>) -> Value {
    json!({"title": title, "summary": summary, "columns": columns, "rows": rows})
}

fn task_rows(p: &Project, pred: impl Fn(&Task) -> bool) -> Vec<Vec<String>> {
    p.tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| pred(t))
        .map(|(i, t)| {
            vec![
                (i + 1).to_string(),
                t.name.clone(),
                t.duration.format(&p.settings),
                fmt_date(t.start),
                fmt_date(t.finish),
                format!("{}%", fmt_num(t.percent_complete)),
                p.resource_names(t.uid),
            ]
        })
        .collect()
}
const TASK_COLS: &[&str] = &["ID", "Name", "Duration", "Start", "Finish", "% Complete", "Resources"];

pub fn overview(p: &Project) -> Value {
    let leaves: Vec<&Task> = p.tasks.iter().filter(|t| !t.summary && t.active).collect();
    let total_min: f64 = leaves.iter().map(|t| t.duration.minutes.max(1) as f64).sum();
    let done: f64 = leaves.iter().map(|t| t.duration.minutes.max(1) as f64 * t.percent_complete / 100.0).sum();
    let work: i64 = leaves.iter().map(|t| t.work).sum();
    let cost: f64 = leaves.iter().map(|t| t.cost).sum();
    let base_cost: f64 = leaves.iter().filter_map(|t| t.baseline(0)).map(|b| b.cost).sum();
    let dur = Dur { minutes: p.cal().work_between(p.cal().snap_forward(p.start), p.finish), unit: DurUnit::Days, ..Default::default() };
    json!({
        "title": p.title,
        "start": fmt_datetime(p.start),
        "finish": fmt_datetime(p.finish),
        "duration": dur.format(&p.settings),
        "tasks": leaves.len(),
        "milestones": leaves.iter().filter(|t| t.is_milestone()).count(),
        "summaryTasks": p.tasks.iter().filter(|t| t.summary).count(),
        "resources": p.resources.len(),
        "percentComplete": if total_min > 0.0 { (done / total_min * 100.0).round() } else { 0.0 },
        "workHours": work as f64 / 60.0,
        "cost": cost,
        "baselineCost": base_cost,
        "criticalTasks": leaves.iter().filter(|t| t.critical).count(),
        "lateTasks": leaves.iter().filter(|t| t.late || t.missed_deadline).count(),
        "overallocatedResources": p.resources.iter().filter(|r| r.overallocated).count(),
        "statusDate": p.status_date.map(fmt_datetime),
        "calendar": p.calendar,
        "warnings": p.warnings,
    })
}

pub fn build(p: &Project, id: &str, now: NaiveDateTime) -> Result<Value, String> {
    let title = REPORTS.iter().find(|(k, _)| *k == id).map(|(_, t)| *t).ok_or_else(|| format!("no report '{id}'"))?;
    let v = match id {
        "projectOverview" => {
            let o = overview(p);
            let rows: Vec<Vec<String>> = o
                .as_object()
                .unwrap()
                .iter()
                .filter(|(k, _)| *k != "warnings")
                .map(|(k, v)| vec![k.clone(), crate::fields::value_as_text(v)])
                .collect();
            table(title, o, &["Measure", "Value"], rows)
        }
        "criticalTasks" => table(title, json!({}), TASK_COLS, task_rows(p, |t| t.critical && !t.summary)),
        "lateTasks" => table(title, json!({}), TASK_COLS, task_rows(p, |t| (t.late || t.missed_deadline) && !t.summary)),
        "milestones" => table(title, json!({}), TASK_COLS, task_rows(p, |t| t.is_milestone())),
        "upcoming" => {
            let horizon = now + Duration::days(14);
            table(title, json!({"from": fmt_datetime(now), "to": fmt_datetime(horizon)}), TASK_COLS, task_rows(p, |t| !t.summary && t.percent_complete < 100.0 && t.start <= horizon && t.finish >= now))
        }
        "notStarted" => table(title, json!({}), TASK_COLS, task_rows(p, |t| !t.summary && t.percent_complete <= 0.0)),
        "completed" => table(title, json!({}), TASK_COLS, task_rows(p, |t| !t.summary && t.percent_complete >= 100.0)),
        "deadlines" => {
            let rows = p
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.deadline.is_some())
                .map(|(i, t)| {
                    vec![(i + 1).to_string(), t.name.clone(), fmt_date(t.deadline.unwrap()), fmt_date(t.finish), if t.missed_deadline { "Missed".into() } else { "On track".into() }]
                })
                .collect();
            table(title, json!({}), &["ID", "Name", "Deadline", "Finish", "Status"], rows)
        }
        "costOverview" => {
            let rows = p
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.outline_level == 1 || !t.summary)
                .map(|(i, t)| {
                    let b = t.baseline(0).map(|b| b.cost).unwrap_or(0.0);
                    vec![
                        (i + 1).to_string(),
                        t.name.clone(),
                        p.fmt_cost(t.cost),
                        p.fmt_cost(b),
                        p.fmt_cost(t.cost - b),
                        p.fmt_cost(t.cost * t.percent_complete / 100.0),
                        p.fmt_cost(t.cost * (1.0 - t.percent_complete / 100.0)),
                    ]
                })
                .collect();
            let total: f64 = p.tasks.iter().filter(|t| !t.summary).map(|t| t.cost).sum();
            table(title, json!({"totalCost": total}), &["ID", "Name", "Cost", "Baseline Cost", "Variance", "Actual Cost", "Remaining Cost"], rows)
        }
        "cashFlow" => {
            // cost per calendar week, spread evenly over each task's working time
            let mut rows = vec![];
            let cal = p.cal();
            let mut wk = cal.snap_forward(p.start).date();
            while wk.weekday().num_days_from_monday() != 0 {
                wk = wk.pred_opt().unwrap();
            }
            let mut cumulative = 0.0;
            let mut guard = 0;
            while wk.and_hms_opt(0, 0, 0).unwrap() <= p.finish && guard < 520 {
                let a = wk.and_hms_opt(0, 0, 0).unwrap();
                let b = a + Duration::days(7);
                let mut c = 0.0;
                for t in p.tasks.iter().filter(|t| !t.summary && t.active) {
                    let tot = cal.work_between(t.start, t.finish);
                    if tot <= 0 {
                        if t.start >= a && t.start < b {
                            c += t.cost;
                        }
                        continue;
                    }
                    let lo = a.max(t.start);
                    let hi = b.min(t.finish);
                    if hi > lo {
                        c += t.cost * cal.work_between(lo, hi) as f64 / tot as f64;
                    }
                }
                cumulative += c;
                rows.push(vec![format!("Week of {}", wk.format("%-m/%-d/%y")), p.fmt_cost(c), p.fmt_cost(cumulative)]);
                wk += Duration::days(7);
                guard += 1;
            }
            table(title, json!({"total": cumulative}), &["Week", "Cost", "Cumulative Cost"], rows)
        }
        "resourceOverview" | "workOverview" | "overallocated" => {
            let rows = p
                .resources
                .iter()
                .enumerate()
                .filter(|(_, r)| id != "overallocated" || r.overallocated)
                .map(|(i, r)| {
                    let asg = p.assignments_of_resource(r.uid).count();
                    vec![
                        (i + 1).to_string(),
                        r.name.clone(),
                        r.kind.title().to_string(),
                        p.fmt_work(r.work),
                        p.fmt_cost(r.cost),
                        format!("{}%", fmt_num(r.peak * 100.0)),
                        format!("{}%", fmt_num(r.max_units * 100.0)),
                        asg.to_string(),
                        if r.overallocated { "Yes".into() } else { "No".into() },
                    ]
                })
                .collect();
            table(title, json!({}), &["ID", "Resource", "Type", "Work", "Cost", "Peak", "Max Units", "Assignments", "Overallocated"], rows)
        }
        "slipping" => {
            let cal = p.cal();
            let rows = p
                .tasks
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    let b = t.baseline(0)?;
                    let bf = b.finish?;
                    let var = cal.work_between(bf, t.finish);
                    if var <= 0 || t.summary {
                        return None;
                    }
                    let d = Dur { minutes: var, unit: DurUnit::Days, ..Default::default() };
                    Some(vec![(i + 1).to_string(), t.name.clone(), fmt_date(bf), fmt_date(t.finish), d.format(&p.settings)])
                })
                .collect();
            table(title, json!({"note": "Compares against Baseline; save one first with project.baseline.set.0"}), &["ID", "Name", "Baseline Finish", "Finish", "Finish Variance"], rows)
        }
        _ => return Err(format!("no report '{id}'")),
    };
    let _ = schedule::assignment_work_in;
    Ok(v)
}

/// Compare two versions of a plan, matching tasks by name.
pub fn compare(this: &Project, other: &Project) -> Value {
    let key = |t: &Task| t.name.trim().to_lowercase();
    let mut rows = vec![];
    let (mut changed, mut added, mut removed) = (0, 0, 0);
    for t in &this.tasks {
        match other.tasks.iter().find(|o| key(o) == key(t)) {
            Some(o) => {
                let mut diffs = vec![];
                if o.start != t.start {
                    diffs.push("start");
                }
                if o.finish != t.finish {
                    diffs.push("finish");
                }
                if o.duration.minutes != t.duration.minutes {
                    diffs.push("duration");
                }
                if (o.percent_complete - t.percent_complete).abs() > 0.01 {
                    diffs.push("% complete");
                }
                if other.resource_names(o.uid) != this.resource_names(t.uid) {
                    diffs.push("resources");
                }
                if !diffs.is_empty() {
                    changed += 1;
                }
                rows.push(vec![
                    t.name.clone(),
                    if diffs.is_empty() { "Same".into() } else { format!("Changed: {}", diffs.join(", ")) },
                    fmt_date(t.start),
                    fmt_date(o.start),
                    fmt_date(t.finish),
                    fmt_date(o.finish),
                    t.duration.format(&this.settings),
                    o.duration.format(&other.settings),
                ]);
            }
            None => {
                added += 1;
                rows.push(vec![t.name.clone(), "Only in this plan".into(), fmt_date(t.start), String::new(), fmt_date(t.finish), String::new(), t.duration.format(&this.settings), String::new()]);
            }
        }
    }
    for o in &other.tasks {
        if !this.tasks.iter().any(|t| key(t) == key(o)) {
            removed += 1;
            rows.push(vec![o.name.clone(), "Only in the other plan".into(), String::new(), fmt_date(o.start), String::new(), fmt_date(o.finish), String::new(), o.duration.format(&other.settings)]);
        }
    }
    table(
        "Compare Projects",
        json!({"thisPlan": this.title, "otherPlan": other.title, "changed": changed, "onlyInThisPlan": added, "onlyInOtherPlan": removed, "thisFinish": fmt_datetime(this.finish), "otherFinish": fmt_datetime(other.finish)}),
        &["Task", "Status", "Start", "Other Start", "Finish", "Other Finish", "Duration", "Other Duration"],
        rows,
    )
}

/// Render a report table as Markdown text.
pub fn to_markdown(v: &Value) -> String {
    let mut out = format!("# {}\n\n", v["title"].as_str().unwrap_or("Report"));
    let cols: Vec<String> = v["columns"].as_array().map(|a| a.iter().map(crate::fields::value_as_text).collect()).unwrap_or_default();
    if !cols.is_empty() {
        out.push_str(&format!("| {} |\n|{}|\n", cols.join(" | "), cols.iter().map(|_| "---").collect::<Vec<_>>().join("|")));
        if let Some(rows) = v["rows"].as_array() {
            for r in rows {
                let cells: Vec<String> = r.as_array().map(|a| a.iter().map(crate::fields::value_as_text).collect()).unwrap_or_default();
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
        }
    }
    out
}
