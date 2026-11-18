//! The one command registry. Every menu item, ribbon button, dialog button, CLI `--cmd`, JSON
//! control message and MCP tool call runs a command from here.

use crate::calendar::{Calendar, CalException};
use crate::fields::{self, parse_dur_value, parse_percent, parse_work_value, value_as_text, RESOURCE_FIELDS, TASK_FIELDS};
use crate::model::*;
use crate::reports;
use crate::schedule;
use crate::session::*;
use chrono::{Duration, Local, NaiveDateTime};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub type Handler = Arc<dyn Fn(&mut Session, &Value) -> Result<Value, String> + Send + Sync>;

#[derive(Clone, Serialize, Debug)]
pub struct Param {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub help: String,
}

#[derive(Clone, Serialize)]
pub struct Command {
    pub id: String,
    pub title: String,
    pub category: String,
    pub help: String,
    pub params: Vec<Param>,
    pub mutates: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    #[serde(skip)]
    handler: Handler,
}

pub struct Registry {
    pub commands: Vec<Command>,
    index: HashMap<String, usize>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

impl Registry {
    pub fn get() -> &'static Registry {
        REGISTRY.get_or_init(build)
    }
    pub fn find(&self, id: &str) -> Option<&Command> {
        self.index.get(id).or_else(|| self.index.get(&id.to_lowercase())).map(|&i| &self.commands[i])
    }
    pub fn len(&self) -> usize {
        self.commands.len()
    }
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
    pub fn categories(&self) -> Vec<String> {
        let mut c: Vec<String> = vec![];
        for cmd in &self.commands {
            if !c.contains(&cmd.category) {
                c.push(cmd.category.clone());
            }
        }
        c
    }
    /// Run a command. Mutating commands are undoable and reschedule the project afterwards;
    /// a failing one leaves the project exactly as it was.
    pub fn run(&self, s: &mut Session, id: &str, params: &Value) -> Result<Value, String> {
        let cmd = self.find(id).ok_or_else(|| {
            let want = id.to_lowercase();
            let last = want.split('.').next_back().unwrap_or(&want).to_string();
            let mut scored: Vec<(usize, &str)> = self
                .commands
                .iter()
                .map(|c| {
                    let l = c.id.to_lowercase();
                    let mut d = edit_distance(&l, &want);
                    if last.len() > 2 && l.contains(&last) {
                        d = d.saturating_sub(3);
                    }
                    (d, c.id.as_str())
                })
                .filter(|(d, _)| *d <= (want.len() / 3).max(2))
                .collect();
            scored.sort();
            let close: Vec<&str> = scored.into_iter().take(5).map(|(_, c)| c).collect();
            if close.is_empty() {
                format!("unknown command '{id}' (list them with `plancraft-cli commands`)")
            } else {
                format!("unknown command '{id}'. Did you mean: {}", close.join(", "))
            }
        })?;
        // agents sometimes send the parameters as a JSON string; accept that too
        let params = match params {
            Value::Null => json!({}),
            Value::String(t) if t.trim().is_empty() => json!({}),
            Value::String(t) => serde_json::from_str::<Value>(t).map_err(|e| format!("params is a string but not JSON ({e})"))?,
            other => other.clone(),
        };
        if !params.is_object() {
            return Err("params must be a JSON object".into());
        }
        for p in &cmd.params {
            if p.required && params.get(&p.name).map(|v| v.is_null()).unwrap_or(true) {
                return Err(format!("{} needs the '{}' parameter ({})", cmd.id, p.name, p.help));
            }
        }
        let before = if cmd.mutates { Some(s.project.clone()) } else { None };
        let res = (cmd.handler)(s, &params);
        match (&res, before) {
            (Ok(_), Some(b)) => {
                if s.project.settings.auto_calc {
                    schedule::schedule(&mut s.project);
                }
                if s.project != b {
                    s.undo.push((cmd.title.clone(), b));
                    let cap = if s.headless { 200 } else { crate::prefs::get().undo_levels.max(1) };
                    while s.undo.len() > cap {
                        s.undo.remove(0);
                    }


                    s.redo.clear();
                    s.dirty = true;
                }
            }
            (Err(_), Some(b)) => {
                s.project = b;
            }
            _ => {}
        }
        if res.is_ok() {
            s.log.push(format!("{} {}", cmd.id, params));
            if s.log.len() > 1000 {
                s.log.remove(0);
            }
        }
        res
    }
    pub fn describe(&self, id: &str) -> Option<Value> {
        self.find(id).map(|c| serde_json::to_value(c).unwrap())
    }
    pub fn list_json(&self, filter: Option<&str>) -> Value {
        let f = filter.map(|s| s.to_lowercase());
        Value::Array(
            self.commands
                .iter()
                .filter(|c| f.as_ref().map(|f| c.id.to_lowercase().contains(f) || c.title.to_lowercase().contains(f) || c.category == *f).unwrap_or(true))
                .map(|c| json!({"id": c.id, "title": c.title, "category": c.category, "mutates": c.mutates, "params": c.params.iter().map(|p| &p.name).collect::<Vec<_>>()}))
                .collect(),
        )
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

// ---------------------------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------------------------

struct B {
    cmds: Vec<Command>,
}

type P<'a> = (&'a str, &'a str, bool, &'a str);

impl B {
    fn add<F>(&mut self, id: &str, title: &str, help: &str, params: &[P], mutates: bool, h: F)
    where
        F: Fn(&mut Session, &Value) -> Result<Value, String> + Send + Sync + 'static,
    {
        let category = id.split('.').next().unwrap_or("app").to_string();
        self.cmds.push(Command {
            id: id.to_string(),
            title: title.to_string(),
            category,
            help: help.to_string(),
            params: params.iter().map(|(n, k, r, h)| Param { name: n.to_string(), kind: k.to_string(), required: *r, help: h.to_string() }).collect(),
            mutates,
            shortcut: None,
            handler: Arc::new(h),
        });
    }
    fn key(&mut self, shortcut: &str) {
        if let Some(c) = self.cmds.last_mut() {
            c.shortcut = Some(shortcut.to_string());
        }
    }
}

const IDS: P = ("ids", "integer[]", false, "task IDs (row numbers); default: the selected tasks");

// ---------------------------------------------------------------------------------------------
// Parameter helpers
// ---------------------------------------------------------------------------------------------

fn pstr(v: &Value, k: &str) -> Option<String> {
    v.get(k).filter(|x| !x.is_null()).map(value_as_text)
}
fn req(v: &Value, k: &str) -> Result<String, String> {
    pstr(v, k).ok_or_else(|| format!("missing '{k}'"))
}
fn pf64(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.trim().trim_end_matches('%').parse().ok())))
}
fn pbool(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(|x| x.as_bool().or_else(|| x.as_str().map(|s| matches!(s.to_lowercase().as_str(), "true" | "yes" | "1" | "on"))))
}

/// Task indices from "ids" / "id" / "uid" / "task" (a name), or the selection.
fn targets(s: &Session, v: &Value) -> Result<Vec<usize>, String> {
    let p = &s.project;
    let mut out = vec![];
    let push_id = |out: &mut Vec<usize>, id: i64| -> Result<(), String> {
        if id < 1 || id as usize > p.tasks.len() {
            return Err(format!("there is no task {id} (the project has {} tasks)", p.tasks.len()));
        }
        out.push(id as usize - 1);
        Ok(())
    };
    if let Some(ids) = v.get("ids") {
        match ids {
            Value::Array(a) => {
                for x in a {
                    let id = x.as_i64().or_else(|| x.as_str().and_then(|s| s.trim().parse().ok())).ok_or("ids must be numbers")?;
                    push_id(&mut out, id)?;
                }
            }
            Value::Number(n) => push_id(&mut out, n.as_i64().unwrap_or(0))?,
            Value::String(st) => {
                for part in st.split(',') {
                    let part = part.trim();
                    if let Some((a, b)) = part.split_once('-') {
                        let a: i64 = a.trim().parse().map_err(|_| "bad id range")?;
                        let b: i64 = b.trim().parse().map_err(|_| "bad id range")?;
                        for id in a..=b {
                            push_id(&mut out, id)?;
                        }
                    } else if !part.is_empty() {
                        push_id(&mut out, part.parse().map_err(|_| format!("'{part}' is not a task ID"))?)?;
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(id) = v.get("id").and_then(|x| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))) {
        push_id(&mut out, id)?;
    }
    if let Some(uid) = v.get("uid").and_then(|x| x.as_u64()) {
        out.push(p.task_index(uid as u32).ok_or_else(|| format!("no task with unique ID {uid}"))?);
    }
    if let Some(name) = v.get("task").and_then(|x| x.as_str()) {
        match p.tasks.iter().position(|t| t.name.eq_ignore_ascii_case(name)) {
            Some(i) => out.push(i),
            None => return Err(format!("no task named '{name}'")),
        }
    }
    if out.is_empty() {
        out = s.view.selected.iter().filter_map(|u| p.task_index(*u)).collect();
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn need_targets(s: &Session, v: &Value) -> Result<Vec<usize>, String> {
    let t = targets(s, v)?;
    if t.is_empty() {
        Err("no task given: pass \"ids\":[...] or \"id\":n, or select tasks first".into())
    } else {
        Ok(t)
    }
}

fn resource_target(s: &Session, v: &Value, key: &str) -> Result<usize, String> {
    let p = &s.project;
    if let Some(x) = v.get(key) {
        if let Some(n) = x.as_i64() {
            if n >= 1 && (n as usize) <= p.resources.len() {
                return Ok(n as usize - 1);
            }
            return Err(format!("there is no resource {n}"));
        }
        if let Some(name) = x.as_str() {
            if let Ok(n) = name.parse::<usize>() {
                if n >= 1 && n <= p.resources.len() {
                    return Ok(n - 1);
                }
            }
            return p.resources.iter().position(|r| r.name.eq_ignore_ascii_case(name.trim())).ok_or_else(|| format!("no resource named '{name}'"));
        }
    }
    if let Some(u) = s.view.selected_resources.iter().next() {
        if let Some(i) = p.resource_index(*u) {
            return Ok(i);
        }
    }
    Err(format!("missing '{key}' (a resource name or ID)"))
}

fn ok() -> Result<Value, String> {
    Ok(json!({"ok": true}))
}

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

// ---------------------------------------------------------------------------------------------
// Task editing logic (shared with the GUI through commands)
// ---------------------------------------------------------------------------------------------

pub fn work_units(p: &Project, uid: u32) -> f64 {
    p.assignments
        .iter()
        .filter(|a| a.task == uid && p.resource_by_uid(a.resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false))
        .map(|a| a.units)
        .sum()
}

fn work_asg_indices(p: &Project, uid: u32) -> Vec<usize> {
    p.assignments
        .iter()
        .enumerate()
        .filter(|(_, a)| a.task == uid && p.resource_by_uid(a.resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false))
        .map(|(i, _)| i)
        .collect()
}

pub fn set_duration(p: &mut Project, i: usize, d: Dur) -> Result<(), String> {
    if p.tasks[i].summary {
        return Err(format!("'{}' is a summary task: its duration comes from its subtasks", p.tasks[i].name));
    }
    if d.minutes < 0 {
        return Err("duration can't be negative".into());
    }
    let uid = p.tasks[i].uid;
    let tt = p.tasks[i].task_type;
    p.tasks[i].duration = d;
    for ai in work_asg_indices(p, uid) {
        let a = &mut p.assignments[ai];
        if tt == TaskType::FixedWork && d.minutes > 0 && a.work > 0 {
            a.units = a.work as f64 / d.minutes as f64;
        } else {
            a.work = (d.minutes as f64 * a.units).round() as i64;
        }
    }
    Ok(())
}

pub fn set_work(p: &mut Project, i: usize, w: i64) -> Result<(), String> {
    let t = &p.tasks[i];
    if t.summary {
        return Err("a summary task's work comes from its subtasks".into());
    }
    let uid = t.uid;
    let units = work_units(p, uid);
    if units <= 0.0 {
        return Err(format!("'{}' has no work resource: assign one before setting work", t.name));
    }
    let tt = t.task_type;
    let dur = t.duration.minutes;
    let idx = work_asg_indices(p, uid);
    for &ai in &idx {
        let a = &mut p.assignments[ai];
        a.work = (w as f64 * a.units / units).round() as i64;
    }
    if tt == TaskType::FixedDuration && dur > 0 {
        for &ai in &idx {
            let a = &mut p.assignments[ai];
            a.units = a.work as f64 / dur as f64;
        }
    } else {
        let nd = (w as f64 / units).round() as i64;
        p.tasks[i].duration.minutes = nd;
        p.tasks[i].duration.estimated = false;
    }
    Ok(())
}

pub fn assign(p: &mut Project, i: usize, res_uid: u32, units: f64) -> Result<(), String> {
    let t = &p.tasks[i];
    if t.summary {
        return Err("assign resources to subtasks, not to summary tasks".into());
    }
    let uid = t.uid;
    let r = p.resource_by_uid(res_uid).ok_or("no such resource")?.clone();
    if let Some(ai) = p.assignments.iter().position(|a| a.task == uid && a.resource == res_uid) {
        return set_units(p, ai, units);
    }
    let auid = p.alloc_asg_uid();
    let t = &p.tasks[i];
    let mut a = Assignment { uid: auid, task: uid, resource: res_uid, units, work: 0, cost_amount: 0.0, cost: 0.0 };
    match r.kind {
        ResourceKind::Work => {
            let existing = work_asg_indices(p, uid);
            let total = t.work;
            if t.effort_driven && t.task_type != TaskType::FixedDuration && !existing.is_empty() && total > 0 {
                let newu = work_units(p, uid) + units;
                p.assignments.push(a);
                for ai in work_asg_indices(p, uid) {
                    let x = &mut p.assignments[ai];
                    x.work = (total as f64 * x.units / newu).round() as i64;
                }
                p.tasks[i].duration.minutes = (total as f64 / newu).round() as i64;
                return Ok(());
            }
            a.work = (t.duration.minutes as f64 * units).round() as i64;
        }
        ResourceKind::Material => {}
        ResourceKind::Cost => {
            a.units = 1.0;
            a.cost_amount = units;
        }
    }
    p.assignments.push(a);
    Ok(())
}

pub fn set_units(p: &mut Project, ai: usize, units: f64) -> Result<(), String> {
    if units < 0.0 {
        return Err("units can't be negative".into());
    }
    let tuid = p.assignments[ai].task;
    let i = p.task_index(tuid).ok_or("assignment has no task")?;
    let kind = p.resource_by_uid(p.assignments[ai].resource).map(|r| r.kind).unwrap_or_default();
    match kind {
        ResourceKind::Cost => {
            p.assignments[ai].cost_amount = units;
            return Ok(());
        }
        ResourceKind::Material => {
            p.assignments[ai].units = units;
            return Ok(());
        }
        ResourceKind::Work => {}
    }
    let tt = p.tasks[i].task_type;
    let dur = p.tasks[i].duration.minutes;
    if tt == TaskType::FixedDuration || p.assignments[ai].work == 0 || units == 0.0 {
        p.assignments[ai].units = units;
        p.assignments[ai].work = (dur as f64 * units).round() as i64;
    } else {
        p.assignments[ai].units = units;
        let nd = work_asg_indices(p, tuid)
            .into_iter()
            .map(|k| {
                let a = &p.assignments[k];
                if a.units > 0.0 {
                    (a.work as f64 / a.units).round() as i64
                } else {
                    0
                }
            })
            .max()
            .unwrap_or(dur);
        p.tasks[i].duration.minutes = nd;
    }
    Ok(())
}

fn split_top(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' => {
                depth += 1;
                cur.push(c)
            }
            ']' => {
                depth -= 1;
                cur.push(c)
            }
            ',' | ';' if depth == 0 => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// "Alice[50%], Bob, Concrete[20 tons], Travel[$300]" — creates resources that don't exist yet.
pub fn set_resource_names(p: &mut Project, i: usize, text: &str) -> Result<(), String> {
    if p.tasks[i].summary && !text.trim().is_empty() {
        return Err("assign resources to subtasks, not to summary tasks".into());
    }
    let uid = p.tasks[i].uid;
    let mut wanted: Vec<(u32, f64)> = vec![];
    for item in split_top(text) {
        let (name, inner) = match item.find('[') {
            Some(k) => (item[..k].trim().to_string(), Some(item[k + 1..].trim_end_matches(']').trim().to_string())),
            None => (item.clone(), None),
        };
        if name.is_empty() {
            continue;
        }
        let rid = match p.resource_by_name(&name) {
            Some(r) => r.uid,
            None => {
                let ru = p.alloc_res_uid();
                let mut r = Resource::new(ru, &name);
                if let Some(inner) = &inner {
                    if inner.contains(p.settings.currency.as_str()) {
                        r.kind = ResourceKind::Cost;
                    } else if !inner.contains('%') && inner.chars().any(|c| c.is_alphabetic()) {
                        r.kind = ResourceKind::Material;
                        r.material_label = inner.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ').to_string();
                    }
                }
                p.resources.push(r);
                ru
            }
        };
        let kind = p.resource_by_uid(rid).unwrap().kind;
        let units = match (&inner, kind) {
            (None, ResourceKind::Cost) => 0.0,
            (None, _) => 1.0,
            (Some(x), ResourceKind::Work) => {
                let n: f64 = x.trim_end_matches('%').trim().parse().map_err(|_| format!("'{x}' is not a percentage"))?;
                if x.contains('%') || n > 10.0 {
                    n / 100.0
                } else {
                    n
                }
            }
            (Some(x), _) => {
                let digits: String = x.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                digits.parse().map_err(|_| format!("'{x}' is not an amount"))?
            }
        };
        wanted.push((rid, units));
    }
    let keep: Vec<u32> = wanted.iter().map(|w| w.0).collect();
    p.assignments.retain(|a| a.task != uid || keep.contains(&a.resource));
    for (rid, units) in wanted {
        assign(p, i, rid, units)?;
    }
    Ok(())
}

/// "2,3FS+2d,4SS-1d" with task IDs.
pub fn parse_predecessors(p: &Project, i: usize, text: &str) -> Result<Vec<Link>, String> {
    let mut links = vec![];
    for tok in text.split([',', ';']) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let digits: String = tok.chars().take_while(|c| c.is_ascii_digit()).collect();
        let id: usize = digits.parse().map_err(|_| format!("'{tok}' should start with a task ID, like 3 or 3FS+2d"))?;
        if id < 1 || id > p.tasks.len() {
            return Err(format!("there is no task {id}"));
        }
        if id - 1 == i {
            return Err("a task can't be linked to itself".into());
        }
        let rest = tok[digits.len()..].trim();
        let (kind, lagtxt) = if rest.len() >= 2 && LinkKind::from_abbr(&rest[..2]).is_some() {
            (LinkKind::from_abbr(&rest[..2]).unwrap(), rest[2..].trim())
        } else {
            (LinkKind::FS, rest)
        };
        let lag = if lagtxt.is_empty() { Dur::default() } else { Dur::parse(lagtxt, p.settings.duration_unit, &p.settings)? };
        let pred_i = id - 1;
        if p.descendants(pred_i).contains(&i) || p.descendants(i).contains(&pred_i) {
            return Err("a summary task can't be linked to its own subtasks".into());
        }
        links.push(Link { pred: p.tasks[pred_i].uid, kind, lag });
    }
    Ok(links)
}

fn check_circular(p: &mut Project) -> Result<(), String> {
    schedule::schedule(p);
    if p.circular {
        Err("that link would make a loop (circular dependency), so it was not added".into())
    } else {
        Ok(())
    }
}

/// Set any task field, with the scheduling side effects the original has.
pub fn set_task_field(p: &mut Project, i: usize, field: &str, v: &Value) -> Result<(), String> {
    let fd = fields::task_field(field).ok_or_else(|| format!("unknown task field '{field}'"))?;
    if !fd.editable {
        return Err(format!("{} is calculated and can't be typed in", fd.title));
    }
    let field = fd.id;
    let s = p.settings.clone();
    match field {
        "duration" => {
            let d = parse_dur_value(v, &s)?;
            set_duration(p, i, d)?;
        }
        "work" => {
            let w = parse_work_value(v, &s)?;
            set_work(p, i, w)?;
        }
        "start" => {
            if p.tasks[i].summary {
                return Err("a summary task's start comes from its subtasks".into());
            }
            let d = parse_date(&value_as_text(v), s.default_start_min)?;
            let t = &mut p.tasks[i];
            if t.manual {
                t.manual_start = Some(d);
            } else {
                t.constraint = ConstraintKind::SNET;
                t.constraint_date = Some(d);
            }
        }
        "finish" => {
            if p.tasks[i].summary {
                return Err("a summary task's finish comes from its subtasks".into());
            }
            let d = parse_date(&value_as_text(v), s.default_finish_min)?;
            if p.tasks[i].manual {
                let cal = p.task_cal(&p.tasks[i]).clone();
                let st = p.tasks[i].start;
                if d < st {
                    return Err("finish can't be before start".into());
                }
                let mut dur = p.tasks[i].duration;
                dur.minutes = cal.work_between(st, d);
                set_duration(p, i, dur)?;
            } else {
                let t = &mut p.tasks[i];
                t.constraint = ConstraintKind::FNET;
                t.constraint_date = Some(d);
            }
        }
        "predecessors" => {
            let links = parse_predecessors(p, i, &value_as_text(v))?;
            p.tasks[i].predecessors = links;
            check_circular(p)?;
        }
        "resourceNames" => set_resource_names(p, i, &value_as_text(v))?,
        "percentComplete" => {
            let pc = parse_percent(v)?;
            if p.tasks[i].summary {
                for j in p.descendants(i) {
                    if !p.tasks[j].summary {
                        fields::set_task_simple(p, j, "percentComplete", &json!(pc))?;
                    }
                }
            } else {
                fields::set_task_simple(p, i, "percentComplete", &json!(pc))?;
            }
        }
        "milestone" => {
            fields::set_task_simple(p, i, field, v)?;
            if p.tasks[i].milestone && !p.tasks[i].summary {
                let mut d = p.tasks[i].duration;
                d.minutes = 0;
                set_duration(p, i, d)?;
            }
        }
        _ => fields::set_task_simple(p, i, field, v)?,
    }
    Ok(())
}

pub fn insert_task(s: &mut Session, name: &str, dur: Dur, at_index: Option<usize>, after_selection: bool) -> usize {
    let p = &mut s.project;
    let uid = p.alloc_uid();
    let mut t = Task::new(uid, name, dur);
    t.manual = p.settings.new_tasks_manual;
    t.task_type = p.settings.default_task_type;
    t.effort_driven = p.settings.new_tasks_effort_driven;
    let pos = match at_index {
        Some(k) => k.min(p.tasks.len()),
        None => {
            let sel = s.view.selected.iter().filter_map(|u| p.task_index(*u)).max();
            // agents and scripts append at the end unless they ask for a position
            let sel = if s.agent && !after_selection { None } else { sel };
            match sel {
                Some(k) if after_selection => p.descendants(k).end,
                Some(k) => k,
                None => p.tasks.len(),
            }
        }
    };
    t.outline_level = if pos < p.tasks.len() {
        p.tasks[pos].outline_level
    } else if pos > 0 {
        p.tasks[pos - 1].outline_level
    } else {
        1
    };
    if t.manual {
        t.manual_start = Some(p.start);
    }
    p.tasks.insert(pos, t);
    s.view.selected.clear();
    s.view.selected.insert(uid);
    pos
}

fn delete_tasks(p: &mut Project, idx: &[usize]) -> usize {
    let mut kill = vec![false; p.tasks.len()];
    for &i in idx {
        kill[i] = true;
        for j in p.descendants(i) {
            kill[j] = true;
        }
    }
    let uids: Vec<u32> = p.tasks.iter().enumerate().filter(|(i, _)| kill[*i]).map(|(_, t)| t.uid).collect();
    let mut k = 0;
    p.tasks.retain(|_| {
        let keep = !kill[k];
        k += 1;
        keep
    });
    for t in p.tasks.iter_mut() {
        t.predecessors.retain(|l| !uids.contains(&l.pred));
    }
    p.assignments.retain(|a| !uids.contains(&a.task));
    uids.len()
}

fn indent(p: &mut Project, idx: &[usize], by: i32) -> Result<(), String> {
    let mut done: Vec<usize> = vec![];
    for &i in idx {
        if done.iter().any(|&d| p.descendants(d).contains(&i)) {
            continue;
        }
        let range = p.descendants(i);
        if by > 0 {
            if i == 0 {
                return Err("the first task can't be indented".into());
            }
            if p.tasks[i].outline_level > p.tasks[i - 1].outline_level {
                return Err(format!("'{}' is already indented under the task above", p.tasks[i].name));
            }
            if p.tasks[i].outline_level >= 9 {
                return Err("outline levels go down to 9".into());
            }
        } else if p.tasks[i].outline_level <= 1 {
            return Err(format!("'{}' is already at the top level", p.tasks[i].name));
        }
        p.tasks[i].outline_level = (p.tasks[i].outline_level as i32 + by) as u8;
        for j in range {
            p.tasks[j].outline_level = (p.tasks[j].outline_level as i32 + by).max(1) as u8;
        }
        done.push(i);
    }
    // a task that just became a summary drops its own assignments
    for i in 0..p.tasks.len() {
        if p.is_summary(i) {
            let uid = p.tasks[i].uid;
            p.assignments.retain(|a| a.task != uid);
        }
    }
    Ok(())
}

fn move_block(p: &mut Project, i: usize, up: bool) -> Result<(), String> {
    let lvl = p.tasks[i].outline_level;
    let block_end = p.descendants(i).end;
    if up {
        let mut j = i;
        loop {
            if j == 0 {
                return Err("already at the top".into());
            }
            j -= 1;
            if p.tasks[j].outline_level < lvl {
                return Err("already first under its summary".into());
            }
            if p.tasks[j].outline_level == lvl {
                break;
            }
        }
        let moved: Vec<Task> = p.tasks.drain(i..block_end).collect();
        for (k, t) in moved.into_iter().enumerate() {
            p.tasks.insert(j + k, t);
        }
    } else {
        if block_end >= p.tasks.len() || p.tasks[block_end].outline_level < lvl {
            return Err("already last under its summary".into());
        }
        let next_end = p.descendants(block_end).end;
        let next: Vec<Task> = p.tasks.drain(block_end..next_end).collect();
        for (k, t) in next.into_iter().enumerate() {
            p.tasks.insert(i + k, t);
        }
    }
    Ok(())
}

fn task_info(p: &Project, i: usize) -> Value {
    let mut v = fields::task_json(p, i);
    let t = &p.tasks[i];
    v["assignments"] = Value::Array(
        p.assignments_of_task(t.uid)
            .map(|a| {
                let r = p.resource_by_uid(a.resource);
                json!({"resource": r.map(|r| r.name.clone()), "units": a.units, "workHours": a.work as f64 / 60.0, "cost": a.cost})
            })
            .collect(),
    );
    v["links"] = Value::Array(
        t.predecessors
            .iter()
            .filter_map(|l| {
                let pi = p.task_index(l.pred)?;
                Some(json!({"predecessorId": pi + 1, "predecessor": p.tasks[pi].name, "type": l.kind.abbr(), "lag": l.lag.format(&p.settings)}))
            })
            .collect(),
    );
    v
}

fn set_baseline(p: &mut Project, n: usize, only: Option<&[usize]>) {
    let stamp = now();
    for i in 0..p.tasks.len() {
        if let Some(o) = only {
            if !o.contains(&i) {
                continue;
            }
        }
        let t = &mut p.tasks[i];
        if t.baselines.len() < 11 {
            t.baselines.resize(11, None);
        }
        t.baselines[n] = Some(Baseline { start: Some(t.start), finish: Some(t.finish), duration: t.duration, work: t.work, cost: t.cost });
    }
    if p.baseline_saved.len() < 11 {
        p.baseline_saved.resize(11, None);
    }
    p.baseline_saved[n] = Some(stamp);
}

fn pick_calendar_index(p: &Project, v: &Value) -> Result<usize, String> {
    let name = pstr(v, "calendar").or_else(|| pstr(v, "name")).unwrap_or_else(|| p.calendar.clone());
    p.calendars.iter().position(|c| c.name.eq_ignore_ascii_case(&name)).ok_or_else(|| format!("no calendar named '{name}'"))
}

fn parse_color(v: &Value) -> Result<[u8; 3], String> {
    let s = req(v, "color")?;
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return Err("color must look like #3a7bd5".into());
    }
    let n = u32::from_str_radix(h, 16).map_err(|_| "color must look like #3a7bd5")?;
    Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

fn zoom_to_fit(s: &mut Session, a: NaiveDateTime, b: NaiveDateTime, width: f32) {
    let days = ((b - a).num_minutes() as f32 / 1440.0).max(1.0) + 2.0;
    s.view.px_per_day = (width / days).clamp(0.3, 400.0);
    s.view.timescale = if s.view.px_per_day > 120.0 {
        TimeUnit::Hours
    } else if s.view.px_per_day > 18.0 {
        TimeUnit::Days
    } else if s.view.px_per_day > 6.0 {
        TimeUnit::Weeks
    } else if s.view.px_per_day > 1.2 {
        TimeUnit::Months
    } else if s.view.px_per_day > 0.4 {
        TimeUnit::Quarters
    } else {
        TimeUnit::Years
    };
    s.view.scroll_to = Some(a - Duration::days(1));
}

fn flag_mut<'a>(v: &'a mut ViewState, id: &str) -> Option<&'a mut bool> {
    Some(match id {
        "timeline" => &mut v.show_timeline,
        "criticalPath" => &mut v.show_critical,
        "baseline" => &mut v.show_baseline,
        "slack" => &mut v.show_slack,
        "lateTasks" => &mut v.show_late,
        "outlineNumbers" => &mut v.show_outline_numbers,
        "projectSummary" => &mut v.show_project_summary,
        "gridlines" => &mut v.show_gridlines,
        "nonworkingTime" => &mut v.show_nonworking,
        "linkLines" => &mut v.show_links,
        "barText" => &mut v.show_bar_text,
        "progress" => &mut v.show_progress,
        "details" => &mut v.show_details,
        "todayLine" => &mut v.show_today,
        "darkMode" => &mut v.dark,
        "summaryTasks" => &mut v.show_summary_tasks,
        _ => return None,
    })
}

pub const VIEW_FLAGS: &[(&str, &str)] = &[
    ("timeline", "Timeline"),
    ("criticalPath", "Critical Tasks"),
    ("baseline", "Baseline Bars"),
    ("slack", "Slack Bars"),
    ("lateTasks", "Late Tasks"),
    ("outlineNumbers", "Outline Numbers"),
    ("projectSummary", "Project Summary Task"),
    ("gridlines", "Gridlines"),
    ("nonworkingTime", "Nonworking Time Shading"),
    ("linkLines", "Link Lines"),
    ("barText", "Bar Text"),
    ("progress", "Progress Bars"),
    ("details", "Details Pane"),
    ("todayLine", "Today Line"),
    ("darkMode", "Dark Mode"),
    ("summaryTasks", "Summary Tasks"),
];

pub fn view_flag(v: &ViewState, id: &str) -> bool {
    let mut c = v.clone();
    flag_mut(&mut c, id).map(|b| *b).unwrap_or(false)
}

pub const PRIORITY_LEVELS: &[(&str, &str, u16)] = &[
    ("lowest", "Lowest", 100),
    ("veryLow", "Very Low", 200),
    ("lower", "Lower", 300),
    ("low", "Low", 400),
    ("medium", "Medium", 500),
    ("high", "High", 600),
    ("higher", "Higher", 700),
    ("veryHigh", "Very High", 800),
    ("highest", "Highest", 900),
    ("doNotLevel", "Do Not Level", 1000),
];

pub const GETTING_STARTED: &[&str] = &[
    "Type task names in the Task Name column (or Task > Insert Task). Give each a Duration such as 3d.",
    "Link tasks: select two or more and press Link (or type IDs in Predecessors, like 2 or 3SS+1d).",
    "Indent tasks under a summary with Indent; the summary adds up its subtasks.",
    "Add people on the Resource Sheet, then type their names in Resource Names (Ana[50%]).",
    "Save a Baseline (Project > Set Baseline) and track progress with the % buttons and Mark on Track.",
];

pub const GRAPH_MODES: &[(&str, &str)] = &[
    ("peakUnits", "Peak Units"),
    ("work", "Work"),
    ("cumulativeWork", "Cumulative Work"),
    ("overallocation", "Overallocation"),
    ("percentAllocation", "Percent Allocation"),
    ("remainingAvailability", "Remaining Availability"),
    ("cost", "Cost"),
    ("cumulativeCost", "Cumulative Cost"),
];

pub const WEEKDAYS: [&str; 7] = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"];

// ---------------------------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------------------------

fn build() -> Registry {
    let mut b = B { cmds: vec![] };
    file_commands(&mut b);
    edit_commands(&mut b);
    task_commands(&mut b);
    resource_commands(&mut b);
    project_commands(&mut b);
    calendar_commands(&mut b);
    view_commands(&mut b);
    format_commands(&mut b);
    report_commands(&mut b);
    help_commands(&mut b);
    let mut index = HashMap::new();
    for (i, c) in b.cmds.iter().enumerate() {
        index.insert(c.id.clone(), i);
        index.entry(c.id.to_lowercase()).or_insert(i);
    }
    Registry { commands: b.cmds, index }
}

/// Mirror app settings that live in the view (theme, text size) after they change.
fn apply_prefs_to_session(s: &mut Session, p: &crate::prefs::Prefs) {
    match p.theme.as_str() {
        "dark" => s.view.dark = true,
        "light" => s.view.dark = false,
        _ => {} // "system": the window follows the operating system
    }
    s.view.font_size = p.text_size;
}

fn reset_session(s: &mut Session, p: Project) {
    let headless = s.headless;
    let dark = s.view.dark;
    *s = Session::new(p);
    s.headless = headless;
    s.view.dark = dark;
}

fn file_commands(b: &mut B) {
    b.add("file.new", "New Project", "Start a new, empty project.", &[("title", "string", false, "project title"), ("start", "date", false, "project start date")], false, |s, v| {
        let start = match pstr(v, "start") {
            Some(d) => parse_date(&d, 480)?.date(),
            None => now().date(),
        };
        let p = Project::new(&pstr(v, "title").unwrap_or_else(|| "Project1".into()), start);
        reset_session(s, p);
        if !s.headless {
            // File > Options > General: default view and the person's name as manager
            let prefs = crate::prefs::get();
            if let Some(k) = ViewKind::menu_order().into_iter().find(|k| k.id() == prefs.default_view) {
                s.view.view = k;
            }
            s.project.manager = prefs.user_name.clone();
            s.view.font_size = prefs.text_size;
        }
        ok()
    });
    b.key("Ctrl+N");
    b.add("file.open", "Open...", "Open a project file: .plan.json, .xml in the common XML interchange format, or .mpp from the desktop scheduling program (2010 and later releases; opened as a new plan, since .mpp can't be written).", &[("path", "path", false, "file to open")], false, |s, v| match pstr(v, "path") {
        Some(path) => {
            let pb = std::path::PathBuf::from(&path);
            let (p, view) = crate::io::load(&pb)?;
            reset_session(s, p);
            if let Some(view) = view {
                s.view = view;
            }
            if crate::io::is_mpp(&pb) {
                // .mpp is read-only here: Save asks where to keep a plan file
                s.path = None;
                return Ok(json!({"opened": path, "format": "mpp", "tasks": s.project.tasks.len(), "resources": s.project.resources.len(), "assignments": s.project.assignments.len(), "note": "Read from .mpp; save it as a .plan.json or .xml file to keep changes."}));
            }
            s.path = Some(pb);
            Ok(json!({"opened": s.path, "tasks": s.project.tasks.len()}))
        }
        None if !s.headless => {
            s.dialog(Dialog::Open);
            ok()
        }
        None => Err("file.open needs a 'path'".into()),
    });
    b.key("Ctrl+O");
    b.add("file.save", "Save", "Save the project to its file.", &[("path", "path", false, "where to save (default: the file it came from)")], false, |s, v| {
        let path = pstr(v, "path").map(std::path::PathBuf::from).or_else(|| s.path.clone());
        match path {
            Some(path) => {
                crate::io::write_any(s, &path)?;
                s.path = Some(path.clone());
                s.dirty = false;
                Ok(json!({"saved": path}))
            }
            None if !s.headless => {
                s.dialog(Dialog::SaveAs);
                ok()
            }
            None => Err("file.save needs a 'path' for a new project".into()),
        }
    });
    b.key("Ctrl+S");
    b.add("file.saveAs", "Save As...", "Save the project under a new name. The extension picks the format: .plan.json/.json, .xml, .csv, .svg, .md, .html.", &[("path", "path", false, "file to write")], false, |s, v| match pstr(v, "path") {
        Some(path) => {
            let pb = std::path::PathBuf::from(&path);
            crate::io::write_any(s, &pb)?;
            if crate::io::is_project_format(&pb) {
                s.path = Some(pb);
                s.dirty = false;
            }
            Ok(json!({"saved": path}))
        }
        None if !s.headless => {
            s.dialog(Dialog::SaveAs);
            ok()
        }
        None => Err("file.saveAs needs a 'path'".into()),
    });
    b.key("F12");
    for (fmt, title, ext) in [
        ("csv", "Export Task Table (CSV)", "csv"),
        ("xml", "Export XML Interchange File", "xml"),
        ("svg", "Export Gantt Chart Picture (SVG)", "svg"),
        ("json", "Export JSON", "json"),
        ("html", "Export HTML Report", "html"),
        ("markdown", "Export Markdown Summary", "md"),
        ("resourcesCsv", "Export Resource Table (CSV)", "csv"),
    ] {
        let fmt_s = fmt.to_string();
        b.add(&format!("file.export.{fmt}"), title, &format!("Write the project as .{ext}."), &[("path", "path", false, "file to write")], false, move |s, v| match pstr(v, "path") {
            Some(path) => {
                crate::io::export(s, &fmt_s, std::path::Path::new(&path))?;
                Ok(json!({"written": path}))
            }
            None if !s.headless => {
                s.dialog(Dialog::Export(fmt_s.clone()));
                ok()
            }
            None => Err("needs a 'path'".into()),
        });
    }
    b.add("file.close", "Close", "Close the project and start an empty one.", &[("force", "boolean", false, "discard unsaved changes without asking")], false, |s, v| {
        if s.dirty && !s.headless && !pbool(v, "force").unwrap_or(false) {
            s.dialog(Dialog::ConfirmDiscard("file.close".into()));
            return ok();
        }
        reset_session(s, Project::new("Project1", now().date()));
        ok()
    });
    b.add("file.info", "File Information", "The open file, whether it has unsaved changes and its statistics.", &[], false, |s, _| Ok(json!({"path": s.path, "dirty": s.dirty, "overview": reports::overview(&s.project)})));
    b.add("file.exportPicture", "Save Picture of Window (PNG)", "Save a PNG picture of the window (window only).", &[("path", "path", false, "PNG file")], false, |s, v| {
        if s.headless {
            return Err("pictures need the window; from the command line use file.export.svg".into());
        }
        s.request(UiRequest::Screenshot(pstr(v, "path").map(Into::into)));
        ok()
    });
    b.add("file.recent", "Recent Plans", "Plans recently opened or saved in the window (pinned first).", &[], false, |_, _| {
        let mut items = crate::recent::list();
        items.sort_by_key(|r| !r.pinned);
        Ok(serde_json::to_value(items).unwrap_or(Value::Null))
    });
    b.add("file.recent.pin", "Pin Recent Plan", "Pin or unpin a plan in the recent list.", &[("path", "path", true, "plan path as listed by file.recent"), ("pinned", "boolean", false, "default true")], false, |_, v| {
        let p = req(v, "path")?;
        if crate::recent::set_pinned(&p, pbool(v, "pinned").unwrap_or(true)) {
            ok()
        } else {
            Err(format!("'{p}' isn't in the recent list"))
        }
    });
    b.add("file.recent.remove", "Remove from Recent", "Take a plan off the recent list (the file stays).", &[("path", "path", true, "plan path as listed by file.recent")], false, |_, v| {
        let p = req(v, "path")?;
        if crate::recent::remove(&p) {
            ok()
        } else {
            Err(format!("'{p}' isn't in the recent list"))
        }
    });
    b.add("file.templates","List Templates", "The built-in project templates for file.newFromTemplate.", &[], false, |_, _| {
        Ok(Value::Array(crate::templates::TEMPLATES.iter().map(|t| json!({"id": t.id, "title": t.title, "description": t.description})).collect()))
    });
    b.add("file.newFromTemplate", "New from Template", "Start a new plan from a built-in template (see file.templates).", &[("template", "string", true, "template id or title"), ("start", "date", false, "start date (default next Monday)")], false, |s, v| {
        let id = req(v, "template")?;
        let t = crate::templates::find(&id).ok_or_else(|| format!("no template '{id}' (try file.templates)"))?;
        let start = match pstr(v, "start") {
            Some(d) => parse_date(&d, 480)?.date(),
            None => crate::templates::next_monday(now().date()),
        };
        let headless = s.headless;
        let agent = s.agent;
        let dark = s.view.dark;
        *s = crate::templates::build(t, start);
        s.headless = headless;
        s.agent = agent;
        s.view.dark = dark;
        Ok(json!({"template": t.title, "tasks": s.project.tasks.len(), "start": fmt_datetime(s.project.start)}))
    });
    b.add("file.newFromExisting", "New from Existing Project", "Open a plan as a new, unsaved copy (saving asks for a new name).", &[("path", "path", false, "plan to copy")], false, |s, v| {
        let path = match pstr(v, "path") {
            Some(p) => p,
            None if !s.headless => match rfd::FileDialog::new().add_filter("Project plans", &["json", "xml", "mpp"]).pick_file() {
                Some(p) => p.to_string_lossy().to_string(),
                None => return ok(),
            },
            None => return Err("needs a 'path'".into()),
        };
        let (p, view) = crate::io::load(std::path::Path::new(&path))?;
        reset_session(s, p);
        if let Some(view) = view {
            s.view = view;
        }
        s.project.title = format!("{} (copy)", s.project.title);
        s.path = None;
        s.dirty = true;
        Ok(json!({"copiedFrom": path}))
    });
    b.add(
        "file.printSetup",
        "Page Setup",
        "Print settings: orientation, paper, date range, whether to include the task table and only the visible rows.",
        &[("landscape", "boolean", false, "landscape (true) or portrait"), ("paper", "string", false, "letter, legal, tabloid, a4 or a3"), ("from", "date", false, "first date to print (empty = project start)"), ("to", "date", false, "last date to print"), ("table", "boolean", false, "print the task table"), ("visibleOnly", "boolean", false, "only the rows the view shows"), ("header", "string", false, "title printed at the top")],
        false,
        |s, v| {
            let ps = &mut s.view.print;
            if let Some(b2) = pbool(v, "landscape") {
                ps.landscape = b2;
            }
            if let Some(p) = pstr(v, "paper") {
                if !crate::print::PAPERS.iter().any(|(k, _)| *k == p) {
                    return Err("paper must be letter, legal, tabloid, a4 or a3".into());
                }
                ps.paper = p;
            }
            if let Some(f) = v.get("from") {
                ps.from = match f.as_str() {
                    Some(t) if !t.trim().is_empty() => Some(parse_date(t, 0)?.date()),
                    _ => None,
                };
            }
            if let Some(f) = v.get("to") {
                ps.to = match f.as_str() {
                    Some(t) if !t.trim().is_empty() => Some(parse_date(t, 0)?.date()),
                    _ => None,
                };
            }
            if let Some(b2) = pbool(v, "table") {
                ps.table = b2;
            }
            if let Some(b2) = pbool(v, "visibleOnly") {
                ps.visible_only = b2;
            }
            if let Some(h) = pstr(v, "header") {
                ps.header = h;
            }
            let pages = crate::print::layout(s, &s.view.print).len();
            Ok(json!({"settings": s.view.print, "pages": pages}))
        },
    );
    b.add("file.export.pdf", "Export PDF", "Write the printed pages as a PDF file.", &[("path", "path", false, "PDF file")], false, |s, v| {
        let path = match pstr(v, "path") {
            Some(p) => p,
            None if !s.headless => match rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{}.pdf", s.project.title)).save_file() {
                Some(p) => p.to_string_lossy().to_string(),
                None => return ok(),
            },
            None => return Err("needs a 'path'".into()),
        };
        let pages = crate::print::layout(s, &s.view.print);
        std::fs::write(&path, crate::print::to_pdf(&pages)).map_err(|e| format!("can't write {path}: {e}"))?;
        Ok(json!({"written": path, "pages": pages.len()}))
    });
    b.add("file.print", "Print", "Print the current view: makes a PDF of the pages and opens it in your PDF viewer to print (from scripts, writes the PDF and returns its path).", &[("path", "path", false, "where to put the PDF (default a temporary file)")], false, |s, v| {
        let path = pstr(v, "path").map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("{} - print.pdf", s.project.title.replace(['/', '\\', ':'], "-"))));
        let pages = crate::print::layout(s, &s.view.print);
        std::fs::write(&path, crate::print::to_pdf(&pages)).map_err(|e| format!("can't write {}: {e}", path.display()))?;
        if !s.headless && !s.agent {
            crate::print::open_with_system(&path)?;
        }
        Ok(json!({"pdf": path, "pages": pages.len()}))
    });
    for page in ["home", "new", "open", "info", "saveAs", "print", "share", "export"] {
        let pg = page.to_string();
        b.add(&format!("file.backstage.{page}"), &format!("File > {}", match page { "saveAs" => "Save As".to_string(), p => capitalize(p) }), "Open the File screen at this page (in the window).", &[], false, move |s, _| {
            s.request(UiRequest::Backstage(pg.clone()));
            ok()
        });
        if page == "print" {
            b.key("Ctrl+P");
        }
    }
    b.add("file.backstage.close", "Leave File Screen", "Go back from the File screen to the plan.", &[], false, |s, _| {
        s.request(UiRequest::Backstage(String::new()));
        ok()
    });
    b.add("file.exit", "Exit", "Close the window.", &[], false, |s, _| {
        s.request(UiRequest::Quit);
        ok()
    });
}

fn copy_tasks(s: &Session, idx: &[usize]) -> Vec<(Task, Vec<Assignment>)> {
    idx.iter()
        .map(|&i| {
            let t = s.project.tasks[i].clone();
            let a = s.project.assignments_of_task(t.uid).cloned().collect();
            (t, a)
        })
        .collect()
}

fn edit_commands(b: &mut B) {
    b.add("edit.undo", "Undo", "Undo the last change.", &[], false, |s, _| {
        let (label, prev) = s.undo.pop().ok_or("nothing to undo")?;
        let cur = std::mem::replace(&mut s.project, prev);
        s.redo.push((label.clone(), cur));
        s.dirty = true;
        Ok(json!({"undid": label}))
    });
    b.key("Ctrl+Z");
    b.add("edit.redo", "Redo", "Redo the last undone change.", &[], false, |s, _| {
        let (label, next) = s.redo.pop().ok_or("nothing to redo")?;
        let cur = std::mem::replace(&mut s.project, next);
        s.undo.push((label.clone(), cur));
        s.dirty = true;
        Ok(json!({"redid": label}))
    });
    b.key("Ctrl+Y");
    b.add("edit.copy", "Copy", "Copy the selected tasks.", &[IDS], false, |s, v| {
        let idx = need_targets(s, v)?;
        s.clipboard = copy_tasks(s, &idx);
        Ok(json!({"copied": idx.len()}))
    });
    b.key("Ctrl+C");
    b.add("edit.cut", "Cut", "Cut the selected tasks.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        s.clipboard = copy_tasks(s, &idx);
        let n = delete_tasks(&mut s.project, &idx);
        s.view.selected.clear();
        Ok(json!({"cut": n}))
    });
    b.key("Ctrl+X");
    b.add("edit.paste", "Paste", "Paste copied tasks above the selected task (or at the end).", &[], true, |s, _| {
        if s.clipboard.is_empty() {
            return Err("nothing copied".into());
        }
        let clip = s.clipboard.clone();
        let mut pos = s.current_task().unwrap_or(s.project.tasks.len());
        let mut new_uids = vec![];
        for (t, asg) in clip {
            let uid = s.project.alloc_uid();
            let mut nt = t.clone();
            nt.uid = uid;
            nt.predecessors.clear();
            s.project.tasks.insert(pos, nt);
            for a in asg {
                if s.project.resource_by_uid(a.resource).is_some() {
                    let au = s.project.alloc_asg_uid();
                    s.project.assignments.push(Assignment { uid: au, task: uid, ..a });
                }
            }
            new_uids.push(uid);
            pos += 1;
        }
        s.view.selected = new_uids.iter().copied().collect();
        Ok(json!({"pasted": new_uids.len()}))
    });
    b.key("Ctrl+V");
    b.add("edit.delete", "Delete Task", "Delete the selected tasks (and their subtasks).", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let n = delete_tasks(&mut s.project, &idx);
        s.view.selected.clear();
        Ok(json!({"deleted": n}))
    });
    b.key("Delete");
    b.add("edit.selectAll", "Select All", "Select every task.", &[], false, |s, _| {
        s.view.selected = s.project.tasks.iter().map(|t| t.uid).collect();
        Ok(json!({"selected": s.view.selected.len()}))
    });
    b.key("Ctrl+A");
    b.add("edit.selectNone", "Clear Selection", "Select nothing.", &[], false, |s, _| {
        s.view.selected.clear();
        ok()
    });
    b.add("edit.select", "Select Tasks", "Select tasks by ID.", &[("ids", "integer[]", true, "task IDs, e.g. [1,2] or \"3-5\"")], false, |s, v| {
        let idx = need_targets(s, v)?;
        s.view.selected = idx.iter().map(|&i| s.project.tasks[i].uid).collect();
        Ok(json!({"selected": idx.iter().map(|i| i + 1).collect::<Vec<_>>()}))
    });
    b.add("edit.find", "Find", "Find tasks whose name (or a given field) contains text; selects them.", &[("text", "string", false, "text to look for"), ("field", "string", false, "field to search (default name)")], false, |s, v| {
        let Some(text) = pstr(v, "text") else {
            if !s.headless {
                s.dialog(Dialog::FindReplace);
                return ok();
            }
            return Err("edit.find needs 'text'".into());
        };
        let field = pstr(v, "field").unwrap_or_else(|| "name".into());
        let text_l = text.to_lowercase();
        let hits: Vec<usize> = (0..s.project.tasks.len()).filter(|&i| fields::task_text(&s.project, i, &field).to_lowercase().contains(&text_l)).collect();
        s.view.selected = hits.iter().map(|&i| s.project.tasks[i].uid).collect();
        if let Some(&first) = hits.first() {
            let uid = s.project.tasks[first].uid;
            s.request(UiRequest::ScrollToTask(uid));
        }
        Ok(json!({"found": hits.iter().map(|i| i + 1).collect::<Vec<_>>()}))
    });
    b.key("Ctrl+F");
    b.add("edit.replace", "Replace", "Replace text in a task text field for every task (or the given ones).", &[("find", "string", true, "text to find"), ("replace", "string", true, "replacement"), ("field", "string", false, "text field (default name)"), IDS], true, |s, v| {
        let find = req(v, "find")?;
        let rep = req(v, "replace")?;
        let field = pstr(v, "field").unwrap_or_else(|| "name".into());
        let fd = fields::task_field(&field).ok_or("unknown field")?;
        if fd.kind != fields::FieldKind::Text || !fd.editable {
            return Err("replace works on editable text fields".into());
        }
        let idx = if v.get("ids").is_some() { need_targets(s, v)? } else { (0..s.project.tasks.len()).collect() };
        let mut n = 0;
        for i in idx {
            let cur = fields::task_text(&s.project, i, fd.id);
            if !find.is_empty() && cur.contains(&find) {
                set_task_field(&mut s.project, i, fd.id, &json!(cur.replace(&find, &rep)))?;
                n += 1;
            }
        }
        Ok(json!({"replaced": n}))
    });
    b.key("Ctrl+H");
    b.add("edit.goTo", "Go To", "Select a task by ID and scroll to it.", &[("id", "integer", true, "task ID")], false, |s, v| {
        let idx = need_targets(s, v)?;
        let uid = s.project.tasks[idx[0]].uid;
        s.view.selected = [uid].into_iter().collect();
        s.view.scroll_to = Some(s.project.tasks[idx[0]].start - Duration::days(1));
        s.request(UiRequest::ScrollToTask(uid));
        ok()
    });
    b.key("F5");
    b.add("edit.fillDown", "Fill Down", "Copy a field's value from the first selected task to the others.", &[("field", "string", true, "task field"), IDS], true, |s, v| {
        let field = req(v, "field")?;
        let idx = need_targets(s, v)?;
        if idx.len() < 2 {
            return Err("select at least two tasks".into());
        }
        let val = json!(fields::task_text(&s.project, idx[0], &field));
        for &i in &idx[1..] {
            set_task_field(&mut s.project, i, &field, &val)?;
        }
        Ok(json!({"filled": idx.len() - 1}))
    });
    b.key("Ctrl+D");
    for (what, title) in [("notes", "Clear Notes"), ("hyperlinks", "Clear Hyperlinks"), ("constraints", "Clear Constraints"), ("progress", "Clear Progress"), ("customFields", "Clear Custom Fields")] {
        let w = what.to_string();
        b.add(&format!("edit.clear.{what}"), title, &format!("{title} on the selected tasks."), &[IDS], true, move |s, v| {
            let idx = need_targets(s, v)?;
            for &i in &idx {
                let t = &mut s.project.tasks[i];
                match w.as_str() {
                    "notes" => t.notes.clear(),
                    "hyperlinks" => t.hyperlink.clear(),
                    "constraints" => {
                        t.constraint = ConstraintKind::ASAP;
                        t.constraint_date = None
                    }
                    "progress" => {
                        t.percent_complete = 0.0;
                        t.actual_start = None;
                        t.actual_finish = None
                    }
                    _ => {
                        t.text = Default::default();
                        t.number = [0.0; 5];
                        t.flag = [false; 5]
                    }
                }
            }
            Ok(json!({"cleared": idx.len()}))
        });
    }
}

fn task_commands(b: &mut B) {
    b.add(
        "task.insert",
        "Insert Task",
        "Insert a task above the selected one in the window; from scripts and agents it is added at the end unless at or after is given. Durations like 5d, 3h, 2w; a trailing ? marks an estimate. Returns the new task ID.",
        &[
            ("name", "string", false, "task name"),
            ("duration", "duration", false, "e.g. 5d (default 1d?)"),
            ("at", "integer", false, "row (ID) to insert at; default above the selection"),
            ("after", "boolean", false, "insert after the selection instead of above"),
            ("predecessors", "string", false, "e.g. \"2,3SS+1d\""),
            ("resources", "string", false, "e.g. \"Alice[50%],Bob\""),
            ("level", "integer", false, "outline level"),
        ],
        true,
        |s, v| {
            let dur = match v.get("duration") {
                Some(d) if !d.is_null() => parse_dur_value(d, &s.project.settings)?,
                _ => Dur { minutes: s.project.settings.minutes_per_day, unit: DurUnit::Days, elapsed: false, estimated: true },
            };
            let at = v.get("at").and_then(|x| x.as_i64()).map(|k| (k.max(1) - 1) as usize);
            let name = pstr(v, "name").unwrap_or_else(|| "New Task".into());
            let i = insert_task(s, &name, dur, at, pbool(v, "after").unwrap_or(false));
            if let Some(l) = v.get("level").and_then(|x| x.as_u64()) {
                s.project.tasks[i].outline_level = l.clamp(1, 9) as u8;
            }
            if let Some(pr) = pstr(v, "predecessors") {
                set_task_field(&mut s.project, i, "predecessors", &json!(pr))?;
            }
            if let Some(r) = pstr(v, "resources") {
                set_task_field(&mut s.project, i, "resourceNames", &json!(r))?;
            }
            Ok(json!({"id": i + 1, "uid": s.project.tasks[i].uid, "name": s.project.tasks[i].name}))
        },
    );
    b.key("Insert");
    b.add("task.insertMilestone", "Insert Milestone", "Insert a zero-duration milestone.", &[("name", "string", false, "milestone name"), ("at", "integer", false, "row to insert at"), ("predecessors", "string", false, "e.g. \"4\"")], true, |s, v| {
        let at = v.get("at").and_then(|x| x.as_i64()).map(|k| (k.max(1) - 1) as usize);
        let i = insert_task(s, &pstr(v, "name").unwrap_or_else(|| "New Milestone".into()), Dur::zero(), at, false);
        s.project.tasks[i].milestone = true;
        if let Some(pr) = pstr(v, "predecessors") {
            set_task_field(&mut s.project, i, "predecessors", &json!(pr))?;
        }
        Ok(json!({"id": i + 1}))
    });
    b.add("task.insertSummary", "Insert Summary Task", "Insert a summary task above the selected tasks and indent them under it.", &[("name", "string", false, "summary name"), IDS], true, |s, v| {
        let idx = targets(s, v)?;
        let name = pstr(v, "name").unwrap_or_else(|| "New Summary Task".into());
        let one_day = Dur::days(1.0, &s.project.settings);
        if idx.is_empty() {
            let i = insert_task(s, &name, one_day, None, false);
            let j = insert_task(s, "New Task", Dur { estimated: true, ..one_day }, Some(i + 1), false);
            s.project.tasks[j].outline_level = s.project.tasks[i].outline_level + 1;
            return Ok(json!({"id": i + 1}));
        }
        let first = idx[0];
        let lvl = s.project.tasks[first].outline_level;
        let i = insert_task(s, &name, one_day, Some(first), false);
        s.project.tasks[i].outline_level = lvl;
        let shifted: Vec<usize> = idx.iter().map(|k| k + 1).collect();
        indent(&mut s.project, &shifted, 1)?;
        Ok(json!({"id": i + 1}))
    });
    b.add(
        "task.insertRecurring",
        "Insert Recurring Task",
        "Insert a summary with a task repeated every day, week or month.",
        &[
            ("name", "string", true, "task name"),
            ("duration", "duration", false, "length of each occurrence (default 1h)"),
            ("every", "string", false, "day, week or month (default week)"),
            ("count", "integer", true, "number of occurrences"),
            ("start", "date", false, "first occurrence (default project start)"),
        ],
        true,
        |s, v| {
            let name = req(v, "name")?;
            let count = v.get("count").and_then(|x| x.as_u64()).ok_or("count must be a number")?.clamp(1, 520) as i32;
            let every = pstr(v, "every").unwrap_or_else(|| "week".into());
            let step = match every.as_str() {
                "day" | "daily" => Duration::days(1),
                "week" | "weekly" => Duration::weeks(1),
                "month" | "monthly" => Duration::days(30),
                o => return Err(format!("'{o}' should be day, week or month")),
            };
            let dur = match v.get("duration") {
                Some(d) if !d.is_null() => parse_dur_value(d, &s.project.settings)?,
                _ => Dur { minutes: 60, unit: DurUnit::Hours, ..Default::default() },
            };
            let start = match pstr(v, "start") {
                Some(d) => parse_date(&d, s.project.settings.default_start_min)?,
                None => s.project.start,
            };
            let end = s.project.tasks.len();
            let i = insert_task(s, &name, Dur::zero(), Some(end), false);
            s.project.tasks[i].outline_level = 1;
            for k in 0..count {
                let end = s.project.tasks.len();
                let j = insert_task(s, &format!("{} {}", name, k + 1), dur, Some(end), false);
                let t = &mut s.project.tasks[j];
                t.outline_level = 2;
                t.constraint = ConstraintKind::SNET;
                t.constraint_date = Some(start + step * k);
            }
            Ok(json!({"id": i + 1, "occurrences": count}))
        },
    );
    b.add("task.delete", "Delete Task", "Delete tasks (and their subtasks).", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let n = delete_tasks(&mut s.project, &idx);
        s.view.selected.clear();
        Ok(json!({"deleted": n}))
    });
    b.add("task.indent", "Indent Task", "Make the tasks subtasks of the task above.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        indent(&mut s.project, &idx, 1)?;
        ok()
    });
    b.key("Alt+Shift+Right");
    b.add("task.outdent", "Outdent Task", "Move the tasks up one outline level.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        indent(&mut s.project, &idx, -1)?;
        ok()
    });
    b.key("Alt+Shift+Left");
    b.add("task.moveUp", "Move Up", "Move the task (with its subtasks) above its previous sibling.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        move_block(&mut s.project, idx[0], true)?;
        ok()
    });
    b.key("Alt+Shift+Up");
    b.add("task.moveDown", "Move Down", "Move the task (with its subtasks) below its next sibling.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        move_block(&mut s.project, idx[0], false)?;
        ok()
    });
    b.key("Alt+Shift+Down");
    b.add(
        "task.link",
        "Link Tasks",
        "Link tasks: each given task depends on the one before it (Finish-to-Start unless 'type' says otherwise). With 'from' and 'to', links just those two.",
        &[IDS, ("from", "integer", false, "predecessor ID"), ("to", "integer", false, "successor ID"), ("type", "string", false, "FS, SS, FF or SF"), ("lag", "duration", false, "lag, e.g. 2d or -1d")],
        true,
        |s, v| link_cmd(s, v, None),
    );
    b.key("Ctrl+F2");
    for k in LinkKind::ALL {
        b.add(&format!("task.link.{}", k.abbr()), &format!("Link {}", k.title()), &format!("Link the selected tasks {}.", k.title()), &[IDS, ("lag", "duration", false, "lag")], true, move |s, v| link_cmd(s, v, Some(k)));
    }
    b.add("task.unlink", "Unlink Tasks", "Remove links between the selected tasks; with one task, remove all of its links.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let uids: Vec<u32> = idx.iter().map(|&i| s.project.tasks[i].uid).collect();
        let mut n = 0;
        if uids.len() == 1 {
            let u = uids[0];
            for t in s.project.tasks.iter_mut() {
                let before = t.predecessors.len();
                if t.uid == u {
                    t.predecessors.clear();
                } else {
                    t.predecessors.retain(|l| l.pred != u);
                }
                n += before - t.predecessors.len();
            }
        } else {
            for t in s.project.tasks.iter_mut() {
                if uids.contains(&t.uid) {
                    let before = t.predecessors.len();
                    t.predecessors.retain(|l| !uids.contains(&l.pred));
                    n += before - t.predecessors.len();
                }
            }
        }
        Ok(json!({"removedLinks": n}))
    });
    b.key("Ctrl+Shift+F2");
    b.add("task.set", "Set Task Field", "Set any editable task field on the given tasks.", &[IDS, ("field", "string", true, "field id (see task.fields)"), ("value", "any", true, "new value")], true, |s, v| {
        let idx = need_targets(s, v)?;
        let field = req(v, "field")?;
        let val = v.get("value").cloned().unwrap_or(Value::Null);
        for &i in &idx {
            set_task_field(&mut s.project, i, &field, &val)?;
        }
        Ok(json!({"updated": idx.len()}))
    });
    for fd in TASK_FIELDS.iter().filter(|f| f.editable) {
        let fid = fd.id.to_string();
        b.add(&format!("task.set.{}", fd.id), &format!("Set {}", fd.title), &format!("Set {} on the given tasks.", fd.title), &[IDS, ("value", "any", true, "new value")], true, move |s, v| {
            let idx = need_targets(s, v)?;
            let val = v.get("value").cloned().unwrap_or(Value::Null);
            for &i in &idx {
                set_task_field(&mut s.project, i, &fid, &val)?;
            }
            Ok(json!({"updated": idx.len()}))
        });
    }
    b.add("task.get", "Get Task", "Every field of one task, with its links and assignments.", &[("id", "integer", false, "task ID (default the selected task)")], false, |s, v| {
        let idx = need_targets(s, v)?;
        Ok(task_info(&s.project, idx[0]))
    });
    b.add("task.list", "List Tasks", "All tasks with their main fields, or only the given fields.", &[("fields", "string[]", false, "field ids to include")], false, |s, v| {
        let p = &s.project;
        let wanted: Option<Vec<String>> = v.get("fields").and_then(|f| f.as_array()).map(|a| a.iter().map(value_as_text).collect());
        let rows: Vec<Value> = (0..p.tasks.len())
            .map(|i| match &wanted {
                Some(fs) => {
                    let mut m = serde_json::Map::new();
                    m.insert("id".into(), json!(i + 1));
                    for f in fs {
                        m.insert(f.clone(), fields::task_value(p, i, f));
                    }
                    Value::Object(m)
                }
                None => json!({
                    "id": i + 1, "name": p.tasks[i].name, "level": p.tasks[i].outline_level,
                    "duration": p.tasks[i].duration.format(&p.settings), "start": fmt_datetime(p.tasks[i].start),
                    "finish": fmt_datetime(p.tasks[i].finish), "predecessors": p.predecessors_text(&p.tasks[i]),
                    "resources": p.resource_names(p.tasks[i].uid), "percentComplete": p.tasks[i].percent_complete,
                    "critical": p.tasks[i].critical, "summary": p.tasks[i].summary,
                }),
            })
            .collect();
        Ok(Value::Array(rows))
    });
    b.add("task.fields", "List Task Fields", "Every task field id, its title, kind and whether it can be set.", &[], false, |_, _| {
        Ok(Value::Array(TASK_FIELDS.iter().map(|f| json!({"id": f.id, "title": f.title, "kind": format!("{:?}", f.kind), "editable": f.editable})).collect()))
    });
    b.add("task.information", "Task Information...", "Open the Task Information dialog (returns the task's fields).", &[("id", "integer", false, "task ID")], false, |s, v| {
        let idx = need_targets(s, v)?;
        let uid = s.project.tasks[idx[0]].uid;
        s.dialog(Dialog::TaskInfo(uid));
        Ok(task_info(&s.project, idx[0]))
    });
    b.key("Shift+F2");
    b.add("task.notes", "Task Notes", "Set a task's notes (or open them).", &[("id", "integer", false, "task ID"), ("text", "string", false, "new notes")], true, |s, v| {
        let idx = need_targets(s, v)?;
        if let Some(text) = pstr(v, "text") {
            for &i in &idx {
                s.project.tasks[i].notes = text.clone();
            }
            return ok();
        }
        let uid = s.project.tasks[idx[0]].uid;
        s.dialog(Dialog::TaskInfo(uid));
        Ok(json!({"notes": s.project.tasks[idx[0]].notes}))
    });
    for pct in [0, 25, 50, 75, 100] {
        b.add(&format!("task.markComplete.{pct}"), &format!("{pct}% Complete"), &format!("Mark the tasks {pct}% complete."), &[IDS], true, move |s, v| {
            let idx = need_targets(s, v)?;
            for &i in &idx {
                set_task_field(&mut s.project, i, "percentComplete", &json!(pct))?;
            }
            Ok(json!({"updated": idx.len()}))
        });
    }
    b.add("task.markOnTrack", "Mark on Track", "Set % complete to what the status date (or today) says should be done.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let sd = s.project.status_date.unwrap_or_else(now);
        for &i in &idx {
            if s.project.tasks[i].summary {
                continue;
            }
            let t = &s.project.tasks[i];
            let cal = s.project.task_cal(t).clone();
            let pct = if sd >= t.finish {
                100.0
            } else if sd <= t.start {
                0.0
            } else {
                let tot = cal.work_between(t.start, t.finish).max(1) as f64;
                (cal.work_between(t.start, sd) as f64 / tot * 100.0).round()
            };
            fields::set_task_simple(&mut s.project, i, "percentComplete", &json!(pct))?;
        }
        Ok(json!({"updated": idx.len()}))
    });
    b.add("task.inactivate", "Inactivate", "Keep the tasks but take them out of the schedule.", &[IDS], true, |s, v| {
        for i in need_targets(s, v)? {
            s.project.tasks[i].active = false;
        }
        ok()
    });
    b.add("task.activate", "Activate", "Put inactive tasks back into the schedule.", &[IDS], true, |s, v| {
        for i in need_targets(s, v)? {
            s.project.tasks[i].active = true;
        }
        ok()
    });
    b.add("task.manualSchedule", "Manually Schedule", "The tasks keep the dates you give them.", &[IDS], true, |s, v| {
        for i in need_targets(s, v)? {
            fields::set_task_simple(&mut s.project, i, "mode", &json!("manual"))?;
        }
        ok()
    });
    b.key("Ctrl+Shift+M");
    b.add("task.autoSchedule", "Auto Schedule", "The scheduler places the tasks from links and constraints.", &[IDS], true, |s, v| {
        for i in need_targets(s, v)? {
            fields::set_task_simple(&mut s.project, i, "mode", &json!("auto"))?;
        }
        ok()
    });
    b.key("Ctrl+Shift+A");
    b.add("task.respectLinks", "Respect Links", "Move manually scheduled tasks to where their links allow.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let mut snap = s.project.clone();
        for &i in &idx {
            snap.tasks[i].manual = false;
            snap.tasks[i].constraint = ConstraintKind::ASAP;
        }
        schedule::schedule(&mut snap);
        for &i in &idx {
            if s.project.tasks[i].manual {
                s.project.tasks[i].manual_start = Some(snap.tasks[i].start);
            }
        }
        ok()
    });
    b.add("task.moveForward", "Move Task Later", "Move tasks later by some working days (adds a Start No Earlier Than).", &[IDS, ("days", "number", false, "working days (default 1)")], true, |s, v| move_tasks(s, v, 1.0));
    b.add("task.moveBack", "Move Task Earlier", "Move tasks earlier by some working days.", &[IDS, ("days", "number", false, "working days (default 1)")], true, |s, v| move_tasks(s, v, -1.0));
    for c in ConstraintKind::ALL {
        b.add(&format!("task.constraint.{}", c.abbr()), c.title(), &format!("Give the tasks a {} constraint.", c.title()), &[IDS, ("date", "date", c.needs_date(), "constraint date")], true, move |s, v| {
            let idx = need_targets(s, v)?;
            let date = match pstr(v, "date") {
                Some(d) => Some(parse_date(&d, s.project.settings.default_start_min)?),
                None => None,
            };
            for &i in &idx {
                if s.project.tasks[i].summary && matches!(c, ConstraintKind::MSO | ConstraintKind::MFO) {
                    return Err("summary tasks can't have Must Start/Finish On constraints".into());
                }
                let t = &mut s.project.tasks[i];
                t.constraint = c;
                t.constraint_date = if c.needs_date() { date } else { None };
            }
            ok()
        });
    }
    for tt in TaskType::ALL {
        b.add(&format!("task.type.{}", tt.id()), tt.title(), &format!("Make the tasks {}.", tt.title()), &[IDS], true, move |s, v| {
            for i in need_targets(s, v)? {
                s.project.tasks[i].task_type = tt;
            }
            ok()
        });
    }
    for (id, title, val) in PRIORITY_LEVELS {
        let val = *val;
        b.add(&format!("task.priority.{id}"), &format!("Priority: {title}"), &format!("Set the tasks' priority to {val}."), &[IDS], true, move |s, v| {
            for i in need_targets(s, v)? {
                s.project.tasks[i].priority = val;
            }
            ok()
        });
    }
    b.add("task.toggleMilestone", "Mark as Milestone", "Turn the milestone flag on or off.", &[IDS], true, |s, v| {
        for i in need_targets(s, v)? {
            let m = !s.project.tasks[i].milestone;
            set_task_field(&mut s.project, i, "milestone", &json!(m))?;
        }
        ok()
    });
    b.add("task.collapse", "Collapse", "Hide a summary task's subtasks.", &[IDS], false, |s, v| {
        for i in need_targets(s, v)? {
            s.project.tasks[i].collapsed = true;
        }
        ok()
    });
    b.add("task.expand", "Expand", "Show a summary task's subtasks.", &[IDS], false, |s, v| {
        for i in need_targets(s, v)? {
            s.project.tasks[i].collapsed = false;
        }
        ok()
    });
    b.add("task.collapseAll", "Collapse All", "Collapse every summary task.", &[], false, |s, _| {
        for t in s.project.tasks.iter_mut() {
            t.collapsed = t.summary;
        }
        ok()
    });
    b.add("task.expandAll", "Expand All", "Expand every summary task.", &[], false, |s, _| {
        for t in s.project.tasks.iter_mut() {
            t.collapsed = false;
        }
        s.view.outline_level = None;
        ok()
    });
    b.add("task.scrollTo", "Scroll to Task", "Scroll the chart to the selected task's bar.", &[IDS], false, |s, v| {
        let idx = need_targets(s, v)?;
        let t = &s.project.tasks[idx[0]];
        s.view.scroll_to = Some(t.start - Duration::days(1));
        let uid = t.uid;
        s.request(UiRequest::ScrollToTask(uid));
        ok()
    });
    b.key("Ctrl+Shift+F5");
    for (dir, title, step) in [("previous", "Previous Task", -1i64), ("next", "Next Task", 1)] {
        b.add(&format!("task.{dir}"), title, &format!("Select the {} task.", if step < 0 { "previous" } else { "next" }), &[], false, move |s, _| {
            let n = s.project.tasks.len() as i64;
            if n == 0 {
                return Err("there are no tasks".into());
            }
            let cur = s.current_task().map(|i| i as i64).unwrap_or(-1);
            let k = if cur < 0 { 0 } else { (cur + step).clamp(0, n - 1) } as usize;
            let uid = s.project.tasks[k].uid;
            s.view.selected = [uid].into_iter().collect();
            s.request(UiRequest::ScrollToTask(uid));
            Ok(json!({"id": k + 1, "name": s.project.tasks[k].name}))
        });
    }
    b.add(
        "task.insertSubproject",
        "Insert Subproject...",
        "Insert another plan's tasks (with their links, resources and assignments) under a new summary task at the end.",
        &[("path", "path", false, "plan file to insert (.plan.json or .xml)")],
        true,
        |s, v| {
            let Some(path) = pstr(v, "path") else {
                if !s.headless {
                    if let Some(p) = rfd::FileDialog::new().add_filter("Project plans", &["json", "xml", "mpp"]).pick_file() {
                        return Registry::get().run(s, "task.insertSubproject", &json!({"path": p.to_string_lossy()}));
                    }
                    return ok();
                }
                return Err("task.insertSubproject needs a 'path'".into());
            };
            let (other, _) = crate::io::load(std::path::Path::new(&path))?;
            insert_subproject(&mut s.project, &other);
            Ok(json!({"inserted": other.tasks.len(), "from": other.title}))
        },
    );
    b.add("task.duplicate", "Duplicate Task", "Copy the tasks and insert the copies below.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let mut pos = idx.iter().map(|&i| s.project.descendants(i).end).max().unwrap_or(0);
        for &i in &idx {
            let mut t = s.project.tasks[i].clone();
            let uid = s.project.alloc_uid();
            let old = t.uid;
            t.uid = uid;
            t.name = format!("{} (copy)", t.name);
            t.collapsed = false;
            s.project.tasks.insert(pos, t);
            let asg: Vec<Assignment> = s.project.assignments_of_task(old).cloned().collect();
            for a in asg {
                let au = s.project.alloc_asg_uid();
                s.project.assignments.push(Assignment { uid: au, task: uid, ..a });
            }
            pos += 1;
        }
        ok()
    });
    b.add("task.split", "Split Task", "Split a task in two parts at a fraction of its duration, linked Finish-to-Start.", &[("id", "integer", false, "task ID"), ("at", "number", false, "fraction 0-1 (default 0.5)")], true, |s, v| {
        let idx = need_targets(s, v)?;
        let i = idx[0];
        if s.project.tasks[i].summary || s.project.tasks[i].duration.minutes == 0 {
            return Err("only tasks with a duration can be split".into());
        }
        let frac = pf64(v, "at").unwrap_or(0.5).clamp(0.05, 0.95);
        let d = s.project.tasks[i].duration;
        let first = (d.minutes as f64 * frac).round() as i64;
        set_duration(&mut s.project, i, Dur { minutes: first, ..d })?;
        let mut second = s.project.tasks[i].clone();
        let uid = s.project.alloc_uid();
        let old_uid = s.project.tasks[i].uid;
        second.uid = uid;
        second.name = format!("{} (part 2)", second.name);
        second.predecessors = vec![Link { pred: old_uid, kind: LinkKind::FS, lag: Dur::default() }];
        second.percent_complete = 0.0;
        second.actual_start = None;
        second.actual_finish = None;
        for t in s.project.tasks.iter_mut() {
            for l in t.predecessors.iter_mut() {
                if l.pred == old_uid {
                    l.pred = uid;
                }
            }
        }
        s.project.tasks.insert(i + 1, second);
        let asg: Vec<Assignment> = s.project.assignments_of_task(old_uid).cloned().collect();
        for a in asg {
            let au = s.project.alloc_asg_uid();
            s.project.assignments.push(Assignment { uid: au, task: uid, ..a });
        }
        set_duration(&mut s.project, i + 1, Dur { minutes: d.minutes - first, ..d })?;
        Ok(json!({"parts": [i + 1, i + 2]}))
    });
}

/// Copy another plan into this one under a summary task, keeping its outline, links and assignments.
fn insert_subproject(p: &mut Project, other: &Project) {
    let sum_uid = p.alloc_uid();
    let mut sum = Task::new(sum_uid, &other.title, Dur::days(1.0, &p.settings));
    sum.outline_level = 1;
    p.tasks.push(sum);
    let mut map: HashMap<u32, u32> = HashMap::new();
    for t in &other.tasks {
        let nu = p.alloc_uid();
        map.insert(t.uid, nu);
    }
    let mut res_map: HashMap<u32, u32> = HashMap::new();
    for r in &other.resources {
        let ru = match p.resource_by_name(&r.name) {
            Some(x) => x.uid,
            None => {
                let nu = p.alloc_res_uid();
                p.resources.push(Resource { uid: nu, ..r.clone() });
                nu
            }
        };
        res_map.insert(r.uid, ru);
    }
    for t in &other.tasks {
        let mut nt = t.clone();
        nt.uid = map[&t.uid];
        nt.outline_level = (t.outline_level + 1).min(9);
        nt.predecessors = t.predecessors.iter().filter_map(|l| map.get(&l.pred).map(|&np| Link { pred: np, ..l.clone() })).collect();
        if nt.calendar.as_ref().map(|c| p.calendar_named(c).is_none()).unwrap_or(false) {
            nt.calendar = None;
        }
        p.tasks.push(nt);
    }
    for a in &other.assignments {
        if let (Some(&tu), Some(&ru)) = (map.get(&a.task), res_map.get(&a.resource)) {
            let au = p.alloc_asg_uid();
            p.assignments.push(Assignment { uid: au, task: tu, resource: ru, ..a.clone() });
        }
    }
}

fn link_cmd(s: &mut Session, v: &Value, forced: Option<LinkKind>) -> Result<Value, String> {
    let kind = match forced {
        Some(k) => k,
        None => match pstr(v, "type") {
            Some(t) => LinkKind::from_abbr(&t).ok_or("type must be FS, SS, FF or SF")?,
            None => LinkKind::FS,
        },
    };
    let lag = match v.get("lag") {
        Some(l) if !l.is_null() => parse_dur_value(l, &s.project.settings)?,
        _ => Dur::default(),
    };
    let pairs: Vec<(usize, usize)> = match (v.get("from").and_then(|x| x.as_i64()), v.get("to").and_then(|x| x.as_i64())) {
        (Some(a), Some(b)) => {
            let n = s.project.tasks.len() as i64;
            if a < 1 || b < 1 || a > n || b > n {
                return Err("from/to must be task IDs".into());
            }
            vec![(a as usize - 1, b as usize - 1)]
        }
        _ => {
            let idx = need_targets(s, v)?;
            if idx.len() < 2 {
                return Err("select two or more tasks to link".into());
            }
            idx.windows(2).map(|w| (w[0], w[1])).collect()
        }
    };
    for (a, bi) in &pairs {
        if a == bi {
            return Err("a task can't be linked to itself".into());
        }
        let p = &s.project;
        if p.descendants(*a).contains(bi) || p.descendants(*bi).contains(a) {
            return Err("a summary task can't be linked to its own subtasks".into());
        }
        let pu = p.tasks[*a].uid;
        let t = &mut s.project.tasks[*bi];
        t.predecessors.retain(|l| l.pred != pu);
        t.predecessors.push(Link { pred: pu, kind, lag });
    }
    check_circular(&mut s.project)?;
    Ok(json!({"linked": pairs.iter().map(|(a, b)| [a + 1, b + 1]).collect::<Vec<_>>()}))
}

fn move_tasks(s: &mut Session, v: &Value, dir: f64) -> Result<Value, String> {
    let idx = need_targets(s, v)?;
    let days = pf64(v, "days").unwrap_or(1.0) * dir;
    let mpd = s.project.settings.minutes_per_day;
    for &i in &idx {
        if s.project.tasks[i].summary {
            continue;
        }
        let cal = s.project.task_cal(&s.project.tasks[i]).clone();
        let t = &mut s.project.tasks[i];
        let ns = cal.snap_forward(cal.add_work(t.start, (days * mpd as f64).round() as i64));
        if t.manual {
            t.manual_start = Some(ns);
        } else {
            t.constraint = ConstraintKind::SNET;
            t.constraint_date = Some(ns);
        }
    }
    ok()
}

fn resource_commands(b: &mut B) {
    b.add(
        "resource.add",
        "Add Resource",
        "Add a resource to the project.",
        &[
            ("name", "string", true, "resource name"),
            ("type", "string", false, "Work, Material or Cost"),
            ("maxUnits", "number", false, "e.g. 100 (%)"),
            ("stdRate", "number", false, "per hour (work) or per unit (material)"),
            ("ovtRate", "number", false, "overtime rate per hour"),
            ("costPerUse", "number", false, "cost each time it is used"),
            ("group", "string", false, "group"),
            ("email", "string", false, "email"),
            ("materialLabel", "string", false, "unit name for materials"),
        ],
        true,
        |s, v| {
            let name = req(v, "name")?;
            if s.project.resource_by_name(&name).is_some() {
                return Err(format!("there is already a resource named '{name}'"));
            }
            let uid = s.project.alloc_res_uid();
            s.project.resources.push(Resource::new(uid, &name));
            let i = s.project.resources.len() - 1;
            for k in ["type", "maxUnits", "stdRate", "ovtRate", "costPerUse", "group", "email", "materialLabel"] {
                if let Some(x) = v.get(k) {
                    if !x.is_null() {
                        fields::set_resource_field(&mut s.project, i, k, x)?;
                    }
                }
            }
            s.view.selected_resources = [uid].into_iter().collect();
            Ok(json!({"id": i + 1, "uid": uid}))
        },
    );
    b.add("resource.delete", "Delete Resource", "Delete a resource and its assignments.", &[("resource", "string", false, "name or ID (default the selected one)")], true, |s, v| {
        let i = resource_target(s, v, "resource")?;
        let uid = s.project.resources[i].uid;
        s.project.resources.remove(i);
        s.project.assignments.retain(|a| a.resource != uid);
        s.view.selected_resources.remove(&uid);
        ok()
    });
    b.add("resource.set", "Set Resource Field", "Set a resource field.", &[("resource", "string", false, "name or ID"), ("field", "string", true, "field id"), ("value", "any", true, "value")], true, |s, v| {
        let i = resource_target(s, v, "resource")?;
        let f = req(v, "field")?;
        let fd = fields::resource_field(&f).ok_or_else(|| format!("unknown resource field '{f}'"))?;
        if !fd.editable {
            return Err(format!("{} is calculated", fd.title));
        }
        fields::set_resource_field(&mut s.project, i, fd.id, v.get("value").unwrap_or(&Value::Null))?;
        ok()
    });
    for fd in RESOURCE_FIELDS.iter().filter(|f| f.editable) {
        let fid = fd.id.to_string();
        b.add(&format!("resource.set.{}", fd.id), &format!("Set Resource {}", fd.title), &format!("Set a resource's {}.", fd.title), &[("resource", "string", false, "name or ID"), ("value", "any", true, "value")], true, move |s, v| {
            let i = resource_target(s, v, "resource")?;
            fields::set_resource_field(&mut s.project, i, &fid, v.get("value").unwrap_or(&Value::Null))?;
            ok()
        });
    }
    b.add("resource.list", "List Resources", "All resources with work, cost and overallocation.", &[], false, |s, _| Ok(Value::Array((0..s.project.resources.len()).map(|i| fields::resource_json(&s.project, i)).collect())));
    b.add("resource.fields", "List Resource Fields", "Every resource field id.", &[], false, |_, _| Ok(Value::Array(RESOURCE_FIELDS.iter().map(|f| json!({"id": f.id, "title": f.title, "editable": f.editable})).collect())));
    b.add("resource.assign", "Assign Resource", "Assign a resource to tasks (creates it if missing).", &[IDS, ("resource", "string", true, "resource name or ID"), ("units", "number", false, "percent for work resources (default 100), quantity for materials, amount for cost resources")], true, |s, v| {
        let idx = need_targets(s, v)?;
        let ri = match resource_target(s, v, "resource") {
            Ok(i) => i,
            Err(_) => {
                let name = req(v, "resource")?;
                let uid = s.project.alloc_res_uid();
                s.project.resources.push(Resource::new(uid, &name));
                s.project.resources.len() - 1
            }
        };
        let r = s.project.resources[ri].clone();
        let units = match (pf64(v, "units"), r.kind) {
            (Some(u), ResourceKind::Work) => {
                if u > 10.0 {
                    u / 100.0
                } else {
                    u
                }
            }
            (Some(u), _) => u,
            (None, ResourceKind::Cost) => 0.0,
            (None, _) => 1.0,
        };
        for &i in &idx {
            assign(&mut s.project, i, r.uid, units)?;
        }
        Ok(json!({"assigned": r.name, "tasks": idx.iter().map(|i| i + 1).collect::<Vec<_>>()}))
    });
    b.key("Alt+F10");
    b.add("resource.unassign", "Remove Assignment", "Take a resource off tasks.", &[IDS, ("resource", "string", true, "resource name or ID")], true, |s, v| {
        let idx = need_targets(s, v)?;
        let ri = resource_target(s, v, "resource")?;
        let ru = s.project.resources[ri].uid;
        let tu: Vec<u32> = idx.iter().map(|&i| s.project.tasks[i].uid).collect();
        let before = s.project.assignments.len();
        s.project.assignments.retain(|a| !(a.resource == ru && tu.contains(&a.task)));
        Ok(json!({"removed": before - s.project.assignments.len()}))
    });
    b.add("resource.replace", "Replace Resource", "Swap one resource for another on tasks.", &[IDS, ("from", "string", true, "resource now assigned"), ("to", "string", true, "resource to assign instead")], true, |s, v| {
        let idx = if v.get("ids").is_some() { need_targets(s, v)? } else { (0..s.project.tasks.len()).collect() };
        let from = resource_target(s, v, "from")?;
        let to = resource_target(s, v, "to")?;
        let (fu, tu) = (s.project.resources[from].uid, s.project.resources[to].uid);
        let mut n = 0;
        for i in idx {
            let uid = s.project.tasks[i].uid;
            if let Some(a) = s.project.assignments.iter_mut().find(|a| a.task == uid && a.resource == fu) {
                a.resource = tu;
                n += 1;
            }
        }
        Ok(json!({"replaced": n}))
    });
    b.add("resource.level", "Level All", "Delay tasks so no resource works over its maximum units.", &[], true, |s, _| {
        let n = schedule::level(&mut s.project);
        let still: Vec<String> = s.project.resources.iter().filter(|r| r.overallocated).map(|r| r.name.clone()).collect();
        Ok(json!({"delays": n, "stillOverallocated": still}))
    });
    b.key("Ctrl+Shift+F3");
    b.add("resource.clearLeveling", "Clear Leveling", "Remove the delays leveling added.", &[], true, |s, _| {
        schedule::clear_leveling(&mut s.project);
        ok()
    });
    for (kind, title) in [("work", "Work Resource"), ("material", "Material Resource"), ("cost", "Cost Resource")] {
        let k = kind.to_string();
        b.add(&format!("resource.new.{kind}"), &format!("Add {title}"), &format!("Add a new {} and open its information to fill in.", title.to_lowercase()), &[("name", "string", false, "its name (default: a numbered name)")], true, move |s, v| {
            let base = crate::commands::capitalize(&k);
            let name = pstr(v, "name").unwrap_or_else(|| {
                let mut n = 1;
                while s.project.resource_by_name(&format!("{base} Resource {n}")).is_some() {
                    n += 1;
                }
                format!("{base} Resource {n}")
            });
            if s.project.resource_by_name(&name).is_some() {
                return Err(format!("there is already a resource named '{name}'"));
            }
            let uid = s.project.alloc_res_uid();
            let mut r = Resource::new(uid, &name);
            r.kind = ResourceKind::parse(&base).unwrap_or_default();
            s.project.resources.push(r);
            s.view.selected_resources = [uid].into_iter().collect();
            if !s.headless && !s.agent {
                s.dialog(Dialog::ResourceInfo(uid));
            }
            Ok(json!({"id": s.project.resources.len(), "name": name}))
        });
    }
    b.add("resource.levelSelection", "Level Selection", "Level only the selected tasks: only they are delayed to clear overallocations.", &[IDS], true, |s, v| {
        let idx = need_targets(s, v)?;
        let uids: Vec<u32> = idx.iter().map(|&i| s.project.tasks[i].uid).collect();
        let n = schedule::level_where(&mut s.project, Some(&uids), None);
        Ok(json!({"delays": n}))
    });
    b.add("resource.levelResource", "Level Resource", "Level one resource: delay its tasks until it is no longer overallocated.", &[("resource", "string", false, "resource name or ID (default the selected one)")], true, |s, v| {
        let ri = resource_target(s, v, "resource")?;
        let uid = s.project.resources[ri].uid;
        let n = schedule::level_where(&mut s.project, None, Some(uid));
        Ok(json!({"resource": s.project.resources[ri].name, "delays": n, "stillOverallocated": s.project.resources[ri].overallocated}))
    });
    b.add("resource.levelingOptions", "Leveling Options...", "How leveling works, with buttons to level or clear.", &[], false, |s, _| {
        s.dialog(Dialog::LevelingOptions);
        Ok(json!({"order": "priority, then start date, then ID", "delays": "the lower-priority, later task moves to the finish of the task it overlaps", "fixedTasks": "manually scheduled, started and Must Start/Finish On tasks are not moved"}))
    });
    b.add("resource.notes", "Resource Notes", "Set a resource's notes, or open them.", &[("resource", "string", false, "name or ID"), ("text", "string", false, "new notes")], true, |s, v| {
        let ri = resource_target(s, v, "resource")?;
        if let Some(t) = pstr(v, "text") {
            s.project.resources[ri].notes = t;
            return ok();
        }
        let uid = s.project.resources[ri].uid;
        s.dialog(Dialog::ResourceInfo(uid));
        Ok(json!({"notes": s.project.resources[ri].notes}))
    });
    for (dir, title, step) in [("previous", "Previous Resource", -1i64), ("next", "Next Resource", 1)] {
        b.add(&format!("resource.{dir}"), title, &format!("Select the {} resource.", if step < 0 { "previous" } else { "next" }), &[], false, move |s, _| {
            let n = s.project.resources.len() as i64;
            if n == 0 {
                return Err("there are no resources".into());
            }
            let cur = s.view.selected_resources.iter().next().and_then(|u| s.project.resource_index(*u)).map(|i| i as i64).unwrap_or(-1);
            let k = if cur < 0 { 0 } else { (cur + step).clamp(0, n - 1) } as usize;
            s.view.selected_resources = [s.project.resources[k].uid].into_iter().collect();
            Ok(json!({"resource": s.project.resources[k].name, "id": k + 1}))
        });
    }
    b.add("resource.information", "Resource Information...", "Open the Resource Information dialog.", &[("resource", "string", false, "name or ID")], false, |s, v| {
        let i = resource_target(s, v, "resource")?;
        let uid = s.project.resources[i].uid;
        s.dialog(Dialog::ResourceInfo(uid));
        Ok(fields::resource_json(&s.project, i))
    });
    b.add("resource.assignDialog", "Assign Resources...", "Open the Assign Resources dialog.", &[], false, |s, _| {
        s.dialog(Dialog::AssignResources);
        ok()
    });
    b.add("resource.select", "Select Resource", "Select a resource.", &[("resource", "string", true, "name or ID")], false, |s, v| {
        let i = resource_target(s, v, "resource")?;
        s.view.selected_resources = [s.project.resources[i].uid].into_iter().collect();
        ok()
    });
    b.add("resource.nextOverallocation", "Next Overallocation", "Select the next task where a resource is overallocated.", &[], false, |s, _| {
        let over: Vec<u32> = s.project.resources.iter().filter(|r| r.overallocated).map(|r| r.uid).collect();
        let cur = s.current_task().map(|i| i + 1).unwrap_or(0);
        let p = &s.project;
        let n = p.tasks.len();
        let hit = (cur..n).chain(0..cur.min(n)).find(|&i| p.assignments_of_task(p.tasks[i].uid).any(|a| over.contains(&a.resource)));
        match hit {
            Some(i) => {
                let uid = p.tasks[i].uid;
                s.view.selected = [uid].into_iter().collect();
                s.request(UiRequest::ScrollToTask(uid));
                Ok(json!({"id": i + 1}))
            }
            None => Ok(json!({"id": null, "message": "no overallocations"})),
        }
    });
    b.add("assignment.list", "List Assignments", "Every assignment with units, work and cost.", &[], false, |s, _| {
        let p = &s.project;
        Ok(Value::Array(
            p.assignments
                .iter()
                .map(|a| {
                    json!({"task": p.task_index(a.task).map(|i| i + 1), "taskName": p.task_by_uid(a.task).map(|t| t.name.clone()),
                    "resource": p.resource_by_uid(a.resource).map(|r| r.name.clone()), "units": a.units, "workHours": a.work as f64 / 60.0, "cost": a.cost})
                })
                .collect(),
        ))
    });
    b.add("assignment.setUnits", "Set Assignment Units", "Change how much of a resource works on a task.", &[("id", "integer", true, "task ID"), ("resource", "string", true, "resource"), ("units", "number", true, "percent (work) or quantity")], true, |s, v| {
        let i = need_targets(s, v)?[0];
        let ri = resource_target(s, v, "resource")?;
        let (tu, ru) = (s.project.tasks[i].uid, s.project.resources[ri].uid);
        let ai = s.project.assignments.iter().position(|a| a.task == tu && a.resource == ru).ok_or("that resource isn't assigned to the task")?;
        let mut u = pf64(v, "units").ok_or("units must be a number")?;
        if s.project.resources[ri].kind == ResourceKind::Work && u > 10.0 {
            u /= 100.0;
        }
        set_units(&mut s.project, ai, u)?;
        ok()
    });
    b.add("assignment.setWork", "Set Assignment Work", "Change the work of one assignment (in hours or with a unit).", &[("id", "integer", true, "task ID"), ("resource", "string", true, "resource"), ("work", "duration", true, "e.g. 16h")], true, |s, v| {
        let i = need_targets(s, v)?[0];
        let ri = resource_target(s, v, "resource")?;
        let (tu, ru) = (s.project.tasks[i].uid, s.project.resources[ri].uid);
        let ai = s.project.assignments.iter().position(|a| a.task == tu && a.resource == ru).ok_or("that resource isn't assigned to the task")?;
        let w = parse_work_value(v.get("work").unwrap(), &s.project.settings)?;
        s.project.assignments[ai].work = w;
        let units = s.project.assignments[ai].units;
        if units > 0.0 && s.project.tasks[i].task_type != TaskType::FixedDuration {
            let nd = work_asg_indices(&s.project, tu)
                .into_iter()
                .map(|k| {
                    let a = &s.project.assignments[k];
                    if a.units > 0.0 {
                        (a.work as f64 / a.units).round() as i64
                    } else {
                        0
                    }
                })
                .max()
                .unwrap_or(0);
            s.project.tasks[i].duration.minutes = nd;
        } else if s.project.tasks[i].duration.minutes > 0 {
            let d = s.project.tasks[i].duration.minutes as f64;
            s.project.assignments[ai].units = w as f64 / d;
        }
        ok()
    });
}

fn opt_num(v: &Value) -> Result<f64, String> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(|| "expected a number".to_string())
}
fn opt_bool(v: &Value) -> Result<bool, String> {
    v.as_bool().or_else(|| v.as_str().map(|s| matches!(s, "true" | "yes" | "1"))).ok_or_else(|| "expected true/false".to_string())
}
fn opt_time(v: &Value) -> Result<u32, String> {
    let s = value_as_text(v);
    let c = Calendar::parse_shifts(&format!("{}-24:00", s.trim())).map_err(|_| format!("'{s}' is not a time like 08:00"))?;
    Ok(c[0].0)
}

fn add_option(b: &mut B, id: &str, title: &str, help: &str, kind: &'static str, f: fn(&mut Settings, &Value) -> Result<(), String>) {
    b.add(&format!("project.option.{id}"), title, help, &[("value", kind, true, "new value")], true, move |s, v| {
        f(&mut s.project.settings, v.get("value").unwrap())?;
        serde_json::to_value(&s.project.settings).map_err(|e| e.to_string())
    });
}

fn project_commands(b: &mut B) {
    b.add("project.information", "Project Information...", "Open the Project Information dialog (returns the project's details).", &[], false, |s, _| {
        s.dialog(Dialog::ProjectInfo);
        let p = &s.project;
        Ok(json!({"title": p.title, "manager": p.manager, "company": p.company, "start": fmt_datetime(p.start), "finish": fmt_datetime(p.finish), "statusDate": p.status_date.map(fmt_datetime), "calendar": p.calendar}))
    });
    b.add("project.summary", "Project Summary", "Overview numbers: dates, duration, work, cost, % complete, warnings.", &[], false, |s, _| Ok(reports::overview(&s.project)));
    b.add("project.get", "Get Whole Project", "The full project as JSON (tasks, resources, assignments, calendars).", &[], false, |s, _| serde_json::to_value(&s.project).map_err(|e| e.to_string()));
    b.add("project.criticalPath", "Critical Path", "The critical tasks in order.", &[], false, |s, _| {
        let p = &s.project;
        let mut v: Vec<usize> = (0..p.tasks.len()).filter(|&i| p.tasks[i].critical && !p.tasks[i].summary).collect();
        v.sort_by_key(|&i| (p.tasks[i].start, i));
        Ok(Value::Array(v.into_iter().map(|i| json!({"id": i + 1, "name": p.tasks[i].name, "start": fmt_datetime(p.tasks[i].start), "finish": fmt_datetime(p.tasks[i].finish)})).collect()))
    });
    for (field, title) in [("title", "Title"), ("subject", "Subject"), ("manager", "Manager"), ("company", "Company"), ("notes", "Notes")] {
        let f = field.to_string();
        b.add(&format!("project.set.{field}"), &format!("Set Project {title}"), &format!("Set the project's {}.", title.to_lowercase()), &[("value", "string", true, "new value")], true, move |s, v| {
            let val = req(v, "value")?;
            match f.as_str() {
                "title" => s.project.title = val,
                "subject" => s.project.subject = val,
                "manager" => s.project.manager = val,
                "company" => s.project.company = val,
                _ => s.project.notes = val,
            }
            ok()
        });
    }
    b.add("project.set.start", "Set Project Start", "Set the project start date; tasks move with it.", &[("value", "date", true, "e.g. 2026-11-16")], true, |s, v| {
        s.project.start = parse_date(&req(v, "value")?, s.project.settings.default_start_min)?;
        ok()
    });
    b.add("project.set.statusDate", "Set Status Date", "Set the date progress is measured against (empty clears it).", &[("value", "date", false, "e.g. 2026-11-20")], true, |s, v| {
        s.project.status_date = match pstr(v, "value") {
            Some(d) if !d.trim().is_empty() => Some(parse_date(&d, s.project.settings.default_finish_min)?),
            _ => None,
        };
        ok()
    });
    b.add("project.set.calendar", "Set Project Calendar", "Choose the calendar the project schedules on.", &[("value", "string", true, "calendar name")], true, |s, v| {
        let n = req(v, "value")?;
        let c = s.project.calendar_named(&n).ok_or_else(|| format!("no calendar named '{n}'"))?.name.clone();
        s.project.calendar = c;
        ok()
    });
    for c in Calendar::base_calendars() {
        let name = c.name.clone();
        let id = match name.as_str() {
            "Standard" => "standard",
            "24 Hours" => "twentyFourHours",
            _ => "nightShift",
        };
        b.add(&format!("project.calendar.{id}"), &format!("Use {name} Calendar"), &format!("Schedule the project on the {name} calendar."), &[], true, move |s, _| {
            if s.project.calendar_named(&name).is_none() {
                let c = Calendar::base_calendars().into_iter().find(|c| c.name == name).unwrap();
                s.project.calendars.push(c);
            }
            s.project.calendar = name.clone();
            ok()
        });
    }
    b.add("project.recalculate", "Calculate Project", "Reschedule everything now.", &[], true, |s, _| {
        schedule::schedule(&mut s.project);
        Ok(json!({"finish": fmt_datetime(s.project.finish), "warnings": s.project.warnings}))
    });
    b.key("F9");
    b.add("project.statistics", "Project Statistics...", "Start, finish, duration, work and cost against the baseline.", &[], false, |s, _| {
        s.dialog(Dialog::Statistics);
        Ok(reports::overview(&s.project))
    });
    b.add("project.move", "Move Project", "Move the whole project by calendar days (constraints and actuals move too).", &[("days", "number", true, "days, negative to move earlier")], true, |s, v| {
        let d = Duration::minutes((pf64(v, "days").ok_or("days must be a number")? * 1440.0).round() as i64);
        let p = &mut s.project;
        p.start += d;
        for t in p.tasks.iter_mut() {
            for x in [&mut t.constraint_date, &mut t.deadline, &mut t.actual_start, &mut t.actual_finish, &mut t.manual_start, &mut t.level_delay_to] {
                if let Some(v) = x {
                    *v += d;
                }
            }
        }
        if let Some(sd) = &mut p.status_date {
            *sd += d;
        }
        ok()
    });
    b.add("project.updateAsOf", "Update Project", "Set progress as if all work went to plan up to a date.", &[("date", "date", false, "default: status date or today")], true, |s, v| {
        let d = match pstr(v, "date") {
            Some(x) => parse_date(&x, s.project.settings.default_finish_min)?,
            None => s.project.status_date.unwrap_or_else(now),
        };
        let mut n = 0;
        for i in 0..s.project.tasks.len() {
            let t = &s.project.tasks[i];
            if t.summary || !t.active || d <= t.start {
                continue;
            }
            let cal = s.project.task_cal(t).clone();
            let pct = if d >= t.finish { 100.0 } else { (cal.work_between(t.start, d) as f64 / cal.work_between(t.start, t.finish).max(1) as f64 * 100.0).round() };
            if pct > s.project.tasks[i].percent_complete {
                fields::set_task_simple(&mut s.project, i, "percentComplete", &json!(pct))?;
                n += 1;
            }
        }
        s.project.status_date = Some(d);
        Ok(json!({"updated": n}))
    });
    for n in 0..=10usize {
        let name = if n == 0 { "Baseline".to_string() } else { format!("Baseline {n}") };
        b.add(&format!("project.baseline.set.{n}"), &format!("Set {name}"), &format!("Save the current dates, durations, work and costs as {name} (all tasks, or the given ones)."), &[IDS], true, move |s, v| {
            let only = if v.get("ids").is_some() || v.get("id").is_some() { Some(need_targets(s, v)?) } else { None };
            set_baseline(&mut s.project, n, only.as_deref());
            Ok(json!({"baseline": n}))
        });
        b.add(&format!("project.baseline.clear.{n}"), &format!("Clear {name}"), &format!("Remove {name}."), &[], true, move |s, _| {
            for t in s.project.tasks.iter_mut() {
                if let Some(x) = t.baselines.get_mut(n) {
                    *x = None;
                }
            }
            if let Some(x) = s.project.baseline_saved.get_mut(n) {
                *x = None;
            }
            ok()
        });
    }
    b.add("project.options", "Options...", "Open the scheduling options dialog (returns the settings).", &[], false, |s, _| {
        s.dialog(Dialog::Options);
        serde_json::to_value(&s.project.settings).map_err(|e| e.to_string())
    });
    add_option(b, "hoursPerDay", "Hours per Day", "Working hours in a day used to convert durations.", "number", |st, v| {
        st.minutes_per_day = (opt_num(v)? * 60.0).round().clamp(1.0, 1440.0) as i64;
        Ok(())
    });
    add_option(b, "hoursPerWeek", "Hours per Week", "Working hours in a week used to convert durations.", "number", |st, v| {
        st.minutes_per_week = (opt_num(v)? * 60.0).round().clamp(1.0, 10080.0) as i64;
        Ok(())
    });
    add_option(b, "daysPerMonth", "Days per Month", "Working days in a month used to convert durations.", "number", |st, v| {
        st.days_per_month = opt_num(v)?.round().clamp(1.0, 31.0) as i64;
        Ok(())
    });
    add_option(b, "defaultStartTime", "Default Start Time", "Time used when a date is typed without one, e.g. 08:00.", "time", |st, v| {
        st.default_start_min = opt_time(v)?;
        Ok(())
    });
    add_option(b, "defaultFinishTime", "Default Finish Time", "Time used for typed finish dates, e.g. 17:00.", "time", |st, v| {
        st.default_finish_min = opt_time(v)?;
        Ok(())
    });
    add_option(b, "criticalSlack", "Critical Slack (days)", "Tasks with this much slack or less are critical.", "number", |st, v| {
        st.critical_slack = (opt_num(v)? * st.minutes_per_day as f64).round() as i64;
        Ok(())
    });
    add_option(b, "honorConstraints", "Tasks Honor Constraints", "Constraints win over links when they disagree.", "boolean", |st, v| {
        st.honor_constraints = opt_bool(v)?;
        Ok(())
    });
    add_option(b, "newTasksManual", "New Tasks Manually Scheduled", "New tasks keep the dates you give them.", "boolean", |st, v| {
        st.new_tasks_manual = opt_bool(v)?;
        Ok(())
    });
    add_option(b, "newTasksEffortDriven", "New Tasks Effort Driven", "Adding people to new tasks shortens them.", "boolean", |st, v| {
        st.new_tasks_effort_driven = opt_bool(v)?;
        Ok(())
    });
    add_option(b, "defaultTaskType", "Default Task Type", "fixedUnits, fixedDuration or fixedWork.", "string", |st, v| {
        st.default_task_type = TaskType::parse(&value_as_text(v)).ok_or("unknown task type")?;
        Ok(())
    });
    add_option(b, "currency", "Currency Symbol", "Shown before costs.", "string", |st, v| {
        st.currency = value_as_text(v);
        Ok(())
    });
    add_option(b, "autoCalculate", "Calculate Automatically", "Reschedule after every change.", "boolean", |st, v| {
        st.auto_calc = opt_bool(v)?;
        Ok(())
    });
    for u in DurUnit::ALL {
        b.add(&format!("project.option.durationUnit.{}", u.id()), &format!("Show Durations in {}", capitalize(u.id())), "Unit for new and displayed durations.", &[], true, move |s, _| {
            s.project.settings.duration_unit = u;
            for t in s.project.tasks.iter_mut() {
                t.duration.unit = u;
            }
            ok()
        });
        b.add(&format!("project.option.workUnit.{}", u.id()), &format!("Show Work in {}", capitalize(u.id())), "Unit for displayed work.", &[], true, move |s, _| {
            s.project.settings.work_unit = u;
            ok()
        });
    }
}

fn calendar_commands(b: &mut B) {
    b.add("calendar.list", "List Calendars", "Every calendar with its work week and exceptions.", &[], false, |s, _| {
        Ok(Value::Array(
            s.project
                .calendars
                .iter()
                .map(|c| {
                    json!({"name": c.name, "projectCalendar": c.name == s.project.calendar,
                    "week": WEEKDAYS.iter().enumerate().map(|(i, d)| json!({"day": d, "hours": Calendar::shifts_text(&c.week[i])})).collect::<Vec<_>>(),
                    "exceptions": c.exceptions.iter().map(|e| json!({"name": e.name, "start": e.start.to_string(), "end": e.end.to_string(), "hours": Calendar::shifts_text(&e.shifts)})).collect::<Vec<_>>()})
                })
                .collect(),
        ))
    });
    b.add("calendar.changeWorkingTime", "Change Working Time...", "Open the Change Working Time dialog.", &[("calendar", "string", false, "calendar name")], false, |s, v| {
        let i = pick_calendar_index(&s.project, v)?;
        let name = s.project.calendars[i].name.clone();
        s.dialog(Dialog::ChangeWorkingTime(name));
        ok()
    });
    b.add("calendar.new", "New Calendar", "Create a calendar, copying another (default Standard).", &[("name", "string", true, "new calendar name"), ("copyFrom", "string", false, "calendar to copy")], true, |s, v| {
        let name = req(v, "name")?;
        if s.project.calendar_named(&name).is_some() {
            return Err(format!("a calendar named '{name}' already exists"));
        }
        let from = pstr(v, "copyFrom").unwrap_or_else(|| "Standard".into());
        let mut c = s.project.calendar_named(&from).cloned().unwrap_or_else(Calendar::standard);
        c.name = name;
        s.project.calendars.push(c);
        ok()
    });
    b.add("calendar.delete", "Delete Calendar", "Delete a calendar that isn't the project calendar.", &[("name", "string", true, "calendar")], true, |s, v| {
        let i = pick_calendar_index(&s.project, v)?;
        let name = s.project.calendars[i].name.clone();
        if name == s.project.calendar {
            return Err("that is the project calendar; choose another one first".into());
        }
        s.project.calendars.remove(i);
        for t in s.project.tasks.iter_mut() {
            if t.calendar.as_deref() == Some(name.as_str()) {
                t.calendar = None;
            }
        }
        ok()
    });
    b.add("calendar.rename", "Rename Calendar", "Rename a calendar.", &[("name", "string", true, "calendar"), ("to", "string", true, "new name")], true, |s, v| {
        let i = pick_calendar_index(&s.project, v)?;
        let to = req(v, "to")?;
        let old = s.project.calendars[i].name.clone();
        s.project.calendars[i].name = to.clone();
        if s.project.calendar == old {
            s.project.calendar = to.clone();
        }
        for t in s.project.tasks.iter_mut() {
            if t.calendar.as_deref() == Some(old.as_str()) {
                t.calendar = Some(to.clone());
            }
        }
        ok()
    });
    b.add(
        "calendar.setWorkWeek",
        "Set Working Times",
        "Set the hours of weekdays: day is sunday..saturday, weekdays, weekend or all; hours like \"08:00-12:00,13:00-17:00\" or \"nonworking\".",
        &[("calendar", "string", false, "calendar (default project calendar)"), ("day", "string", true, "which days"), ("hours", "string", true, "shifts or nonworking")],
        true,
        |s, v| {
            let i = pick_calendar_index(&s.project, &json!({"calendar": v.get("calendar")}))?;
            let shifts = Calendar::parse_shifts(&req(v, "hours")?)?;
            let day = req(v, "day")?.to_lowercase();
            let days: Vec<usize> = match day.as_str() {
                "all" => (0..7).collect(),
                "weekdays" => (1..6).collect(),
                "weekend" => vec![0, 6],
                d => vec![WEEKDAYS.iter().position(|w| d.len() >= 2 && w.starts_with(&d[..d.len().min(3)])).ok_or_else(|| format!("'{d}' is not a weekday"))?],
            };
            for d in days {
                s.project.calendars[i].week[d] = shifts.clone();
            }
            ok()
        },
    );
    b.add(
        "calendar.addException",
        "Add Calendar Exception",
        "Make days nonworking (holidays) or give them special hours.",
        &[("calendar", "string", false, "calendar"), ("name", "string", false, "exception name"), ("start", "date", true, "first day"), ("end", "date", false, "last day (default start)"), ("hours", "string", false, "special hours; default nonworking")],
        true,
        |s, v| {
            let i = pick_calendar_index(&s.project, &json!({"calendar": v.get("calendar")}))?;
            let start = parse_date(&req(v, "start")?, 0)?.date();
            let end = match pstr(v, "end") {
                Some(e) => parse_date(&e, 0)?.date(),
                None => start,
            };
            if end < start {
                return Err("end is before start".into());
            }
            let shifts = Calendar::parse_shifts(&pstr(v, "hours").unwrap_or_default())?;
            s.project.calendars[i].exceptions.push(CalException { name: pstr(v, "name").unwrap_or_else(|| "Exception".into()), start, end, shifts });
            ok()
        },
    );
    b.add("calendar.removeException", "Remove Calendar Exception", "Remove an exception by name or start date.", &[("calendar", "string", false, "calendar"), ("name", "string", false, "exception name"), ("start", "date", false, "its start date")], true, |s, v| {
        let i = pick_calendar_index(&s.project, &json!({"calendar": v.get("calendar")}))?;
        let name = pstr(v, "name");
        let start = match pstr(v, "start") {
            Some(d) => Some(parse_date(&d, 0)?.date()),
            None => None,
        };
        let before = s.project.calendars[i].exceptions.len();
        s.project.calendars[i].exceptions.retain(|e| !(name.as_ref().map(|n| e.name.eq_ignore_ascii_case(n)).unwrap_or(false) || start.map(|d| e.start == d).unwrap_or(false)));
        let n = before - s.project.calendars[i].exceptions.len();
        if n == 0 {
            return Err("no matching exception".into());
        }
        Ok(json!({"removed": n}))
    });
    b.add("calendar.workingTime", "Working Time on a Date", "The working hours of a date in a calendar.", &[("calendar", "string", false, "calendar"), ("date", "date", true, "date")], false, |s, v| {
        let i = pick_calendar_index(&s.project, &json!({"calendar": v.get("calendar")}))?;
        let d = parse_date(&req(v, "date")?, 0)?.date();
        let c = &s.project.calendars[i];
        Ok(json!({"date": d.to_string(), "hours": Calendar::shifts_text(c.shifts_on(d)), "minutes": c.working_minutes_on(d)}))
    });
}

fn view_commands(b: &mut B) {
    for vk in ViewKind::ALL {
        b.add(&format!("view.show.{}", vk.id()), vk.title(), &format!("Switch to the {} view.", vk.title()), &[], false, move |s, _| {
            s.view.view = vk;
            if vk == ViewKind::TrackingGantt {
                s.view.show_baseline = true;
            }
            ok()
        });
    }
    b.add("view.reset", "Reset to Default", "Put the current view's table, filter, group, sort and zoom back to their defaults.", &[], false, |s, _| {
        let keep = (s.view.view, s.view.selected.clone(), s.view.selected_resources.clone(), s.view.saved_views.clone(), s.view.dark, s.view.show_details);
        s.view = ViewState::default();
        (s.view.view, s.view.selected, s.view.selected_resources, s.view.saved_views, s.view.dark, s.view.show_details) = keep;
        Ok(json!({"view": s.view.view.id()}))
    });
    b.add("view.save", "Save View...", "Save the current view (its table, filter, group, sort and zoom) under a name.", &[("name", "string", false, "name for the view")], false, |s, v| {
        let Some(name) = pstr(v, "name").filter(|n| !n.trim().is_empty()) else {
            if !s.headless {
                s.dialog(Dialog::SaveView);
                return ok();
            }
            return Err("view.save needs a 'name'".into());
        };
        let sv = SavedView {
            name: name.trim().to_string(),
            view: s.view.view,
            columns: s.view.columns.clone(),
            filter: s.view.filter,
            highlight: s.view.highlight,
            group: s.view.group,
            sort: s.view.sort.clone(),
            timescale: s.view.timescale,
            px_per_day: s.view.px_per_day,
            outline_level: s.view.outline_level,
        };
        s.view.saved_views.retain(|x| !x.name.eq_ignore_ascii_case(&sv.name));
        s.view.saved_views.push(sv);
        s.dirty = true;
        Ok(json!({"saved": name, "views": s.view.saved_views.iter().map(|x| &x.name).collect::<Vec<_>>()}))
    });
    b.add("view.apply", "Apply View", "Switch to a built-in view (by id or title) or a view saved with Save View.", &[("name", "string", true, "view id, title or saved name")], false, |s, v| {
        let name = req(v, "name")?;
        if let Some(sv) = s.view.saved_views.iter().find(|x| x.name.eq_ignore_ascii_case(&name)).cloned() {
            s.view.view = sv.view;
            s.view.columns = sv.columns;
            s.view.filter = sv.filter;
            s.view.highlight = sv.highlight;
            s.view.group = sv.group;
            s.view.sort = sv.sort;
            s.view.timescale = sv.timescale;
            s.view.px_per_day = sv.px_per_day;
            s.view.outline_level = sv.outline_level;
            return Ok(json!({"applied": sv.name}));
        }
        let vk = ViewKind::ALL.into_iter().find(|k| k.id().eq_ignore_ascii_case(&name) || k.title().eq_ignore_ascii_case(&name)).ok_or_else(|| format!("no view named '{name}'"))?;
        s.view.view = vk;
        Ok(json!({"applied": vk.title()}))
    });
    b.add("view.deleteSaved", "Delete Saved View", "Remove a view saved with Save View.", &[("name", "string", true, "saved view name")], false, |s, v| {
        let name = req(v, "name")?;
        let before = s.view.saved_views.len();
        s.view.saved_views.retain(|x| !x.name.eq_ignore_ascii_case(&name));
        if before == s.view.saved_views.len() {
            return Err(format!("no saved view named '{name}'"));
        }
        s.dirty = true;
        ok()
    });
    b.add("view.list", "More Views...", "Every built-in view and every saved view (opens the More Views dialog in the window).", &[], false, |s, _| {
        if !s.headless {
            s.dialog(Dialog::MoreViews);
        }
        Ok(json!({
            "current": s.view.view.id(),
            "builtIn": ViewKind::menu_order().into_iter().map(|k| json!({"id": k.id(), "title": k.title()})).collect::<Vec<_>>(),
            "saved": s.view.saved_views.iter().map(|x| json!({"name": x.name, "view": x.view.id()})).collect::<Vec<_>>(),
        }))
    });
    for (id, title) in GRAPH_MODES {
        let mode = id.to_string();
        b.add(&format!("view.graph.{id}"), &format!("Graph: {title}"), &format!("Show {title} in the Resource Graph."), &[], false, move |s, _| {
            s.view.graph_mode = mode.clone();
            if !s.view.view.is_resource_view() || s.view.view != ViewKind::ResourceGraph {
                s.view.view = ViewKind::ResourceGraph;
            }
            Ok(json!({"graph": mode}))
        });
    }
    b.add("view.current", "Current View", "Which view, filter, group, sort and zoom are in use.", &[], false, |s, _| {
        Ok(json!({"view": s.view.view.id(), "filter": s.view.filter.id(), "highlight": s.view.highlight.map(|h| h.id()), "group": s.view.group.id(),
        "sort": s.view.sort, "timescale": s.view.timescale.id(), "pxPerDay": s.view.px_per_day, "columns": s.view.columns}))
    });
    for tu in TimeUnit::ALL {
        b.add(&format!("view.timescale.{}", tu.id()), &format!("Timescale: {}", capitalize(tu.id())), &format!("Show the chart in {}.", tu.id()), &[], false, move |s, _| {
            s.view.timescale = tu;
            s.view.px_per_day = tu.px_per_day();
            ok()
        });
    }
    b.add("view.zoom.in", "Zoom In", "Show less time, in more detail.", &[], false, |s, _| {
        s.view.px_per_day = (s.view.px_per_day * 1.5).min(400.0);
        let pp = s.view.px_per_day;
        s.view.timescale = TimeUnit::ALL.into_iter().rev().find(|t| t.px_per_day() >= pp * 0.7).unwrap_or(TimeUnit::Hours);
        ok()
    });
    b.key("Ctrl+=");
    b.add("view.zoom.out", "Zoom Out", "Show more time.", &[], false, |s, _| {
        s.view.px_per_day = (s.view.px_per_day / 1.5).max(0.3);
        let pp = s.view.px_per_day;
        s.view.timescale = TimeUnit::ALL.into_iter().rev().find(|t| t.px_per_day() >= pp * 0.7).unwrap_or(TimeUnit::Hours);
        ok()
    });
    b.key("Ctrl+-");
    b.add("view.zoom.set", "Set Zoom", "Set how many pixels one day takes in the chart.", &[("pxPerDay", "number", true, "0.3 to 400")], false, |s, v| {
        let pp = pf64(v, "pxPerDay").ok_or("pxPerDay must be a number")?.clamp(0.3, 400.0) as f32;
        s.view.px_per_day = pp;
        s.view.timescale = TimeUnit::ALL.into_iter().rev().find(|t| t.px_per_day() >= pp * 0.7).unwrap_or(TimeUnit::Hours);
        Ok(json!({"pxPerDay": pp, "timescale": s.view.timescale.id()}))
    });
    b.add("view.zoom.entireProject", "Entire Project", "Fit the whole project in the chart.", &[("width", "number", false, "chart width in pixels (default 900)")], false, |s, v| {
        let w = pf64(v, "width").unwrap_or(900.0) as f32;
        let (a, b2) = (s.project.cal().snap_forward(s.project.start), s.project.finish);
        zoom_to_fit(s, a, b2, w);
        ok()
    });
    b.add("view.zoom.selectedTasks", "Selected Tasks", "Fit the selected tasks in the chart.", &[IDS, ("width", "number", false, "chart width")], false, |s, v| {
        let idx = need_targets(s, v)?;
        let a = idx.iter().map(|&i| s.project.tasks[i].start).min().unwrap();
        let b2 = idx.iter().map(|&i| s.project.tasks[i].finish).max().unwrap();
        zoom_to_fit(s, a, b2, pf64(v, "width").unwrap_or(900.0) as f32);
        ok()
    });
    b.add("view.goToToday", "Scroll to Today", "Scroll the chart to today.", &[], false, |s, _| {
        s.view.scroll_to = Some(now() - Duration::days(2));
        ok()
    });
    b.add("view.goToProjectStart", "Scroll to Project Start", "Scroll the chart to the project start.", &[], false, |s, _| {
        s.view.scroll_to = Some(s.project.start - Duration::days(2));
        ok()
    });
    for (id, title) in VIEW_FLAGS {
        let fid = id.to_string();
        b.add(&format!("view.toggle.{id}"), &format!("Show/Hide {title}"), &format!("Turn {title} on or off (or set 'on')."), &[("on", "boolean", false, "true/false; default flips it")], false, move |s, v| {
            let on = pbool(v, "on");
            let f = flag_mut(&mut s.view, &fid).unwrap();
            *f = on.unwrap_or(!*f);
            let now = *f;
            if fid == "darkMode" {
                // dark mode is an app-wide theme (File > Options > General)
                crate::prefs::update(|p| p.theme = if now { "dark".into() } else { "light".into() });
            }
            Ok(json!({"on": now}))
        });
    }
    b.add("app.options", "Options...", "Open the Options window (General, Display, Schedule, Save, Language, Advanced, Customize Ribbon, Quick Access Toolbar, Agents). Returns every app setting.", &[("tab", "string", false, "general, display, schedule, save, language, advanced, ribbon, quickAccess or agents")], false, |s, v| {
        s.dialog(Dialog::AppOptions(pstr(v, "tab").unwrap_or_else(|| "general".into())));
        Ok(crate::prefs::describe())
    });
    b.add("app.options.get", "Get App Settings", "Every app-wide setting (theme, text size, start-up, saving, date style, ribbon tabs, quick access toolbar) and the values each accepts.", &[], false, |_, _| Ok(crate::prefs::describe()));
    b.add("app.options.set", "Set App Setting", "Change one app-wide setting by key (see app.options.get), e.g. {\"key\":\"theme\",\"value\":\"dark\"}.", &[("key", "string", true, "setting key, e.g. theme"), ("value", "any", true, "new value")], false, |s, v| {
        let key = req(v, "key")?;
        let val = v.get("value").cloned().ok_or("needs a 'value'")?;
        let p = crate::prefs::set(&key, &val)?;
        apply_prefs_to_session(s, &p);
        Ok(json!({"key": key, "settings": p}))
    });
    b.add("app.options.reset", "Reset App Settings", "Put every app-wide setting back to its default.", &[], false, |s, _| {
        let p = crate::prefs::update(|p| *p = crate::prefs::Prefs::default());
        apply_prefs_to_session(s, &p);
        Ok(json!({"settings": p}))
    });
    for (id, title) in crate::prefs::THEMES {
        let tid = id.to_string();
        b.add(&format!("app.theme.{id}"), &format!("Theme: {title}"), &format!("Use the {} theme for the whole app.", title.to_lowercase()), &[], false, move |s, _| {
            let p = crate::prefs::set("theme", &json!(tid))?;
            apply_prefs_to_session(s, &p);
            Ok(json!({"theme": p.theme}))
        });
    }
    for l in 1..=9u8 {
        b.add(&format!("view.outline.level{l}"), &format!("Outline Level {l}"), &format!("Show tasks down to outline level {l}."), &[], false, move |s, _| {
            s.view.outline_level = Some(l);
            for t in s.project.tasks.iter_mut() {
                t.collapsed = false;
            }
            ok()
        });
    }
    b.add("view.outline.all", "All Subtasks", "Show every outline level.", &[], false, |s, _| {
        s.view.outline_level = None;
        for t in s.project.tasks.iter_mut() {
            t.collapsed = false;
        }
        ok()
    });
    for f in Filter::ALL {
        b.add(&format!("view.filter.{}", f.id()), &format!("Filter: {}", f.title()), &format!("Show only {} (with their summary tasks).", f.title()), &[], false, move |s, _| {
            s.view.filter = f;
            Ok(json!({"shown": s.task_rows().len()}))
        });
        if f != Filter::All {
            b.add(&format!("view.highlight.{}", f.id()), &format!("Highlight: {}", f.title()), &format!("Highlight {} without hiding the rest.", f.title()), &[], false, move |s, _| {
                s.view.highlight = Some(f);
                ok()
            });
        }
    }
    b.add("view.highlight.none", "No Highlight", "Remove the highlight filter.", &[], false, |s, _| {
        s.view.highlight = None;
        ok()
    });
    b.add("view.filter.search", "Search Tasks", "Show only tasks whose name contains text (empty shows all).", &[("text", "string", false, "text")], false, |s, v| {
        s.view.search = pstr(v, "text").unwrap_or_default();
        Ok(json!({"shown": s.task_rows().len()}))
    });
    for g in Group::ALL {
        b.add(&format!("view.group.{}", g.id()), &format!("Group by: {}", g.title()), &format!("Group tasks by {}.", g.title()), &[], false, move |s, _| {
            s.view.group = g;
            ok()
        });
    }
    for f in crate::session::SORT_FIELDS {
        let title = fields::task_field(f).map(|d| d.title).unwrap_or(f);
        for (dir, asc) in [("asc", true), ("desc", false)] {
            let fs = f.to_string();
            b.add(&format!("view.sort.{f}.{dir}"), &format!("Sort by {title} ({})", if asc { "ascending" } else { "descending" }), &format!("Sort the rows by {title}."), &[], false, move |s, _| {
                s.view.sort = Some((fs.clone(), asc));
                ok()
            });
        }
    }
    b.add("view.sort.none", "Remove Sort", "Show tasks in ID order.", &[], false, |s, _| {
        s.view.sort = None;
        ok()
    });
    for fd in TASK_FIELDS {
        let fid = fd.id.to_string();
        b.add(&format!("view.column.show.{}", fd.id), &format!("Show Column: {}", fd.title), &format!("Add the {} column to the task table.", fd.title), &[("at", "integer", false, "position")], false, move |s, v| {
            if !s.view.columns.contains(&fid) {
                let at = v.get("at").and_then(|x| x.as_u64()).map(|x| x as usize).unwrap_or(s.view.columns.len()).min(s.view.columns.len());
                s.view.columns.insert(at, fid.clone());
            }
            Ok(json!({"columns": s.view.columns}))
        });
        let fid = fd.id.to_string();
        b.add(&format!("view.column.hide.{}", fd.id), &format!("Hide Column: {}", fd.title), &format!("Remove the {} column from the task table.", fd.title), &[], false, move |s, _| {
            s.view.columns.retain(|c| *c != fid);
            Ok(json!({"columns": s.view.columns}))
        });
    }
    b.add("view.column.reset", "Reset Columns", "Back to the standard Entry table.", &[], false, |s, _| {
        s.view.columns = default_columns();
        ok()
    });
    for (table, cols) in [
        ("entry", vec!["id", "mode", "name", "duration", "start", "finish", "predecessors", "resourceNames"]),
        ("cost", vec!["id", "name", "fixedCost", "cost", "baselineCost", "percentComplete"]),
        ("schedule", vec!["id", "name", "start", "finish", "lateStart", "lateFinish", "freeSlack", "totalSlack"]),
        ("tracking", vec!["id", "name", "actualStart", "actualFinish", "percentComplete", "actualDuration", "remainingDuration", "cost", "work"]),
        ("variance", vec!["id", "name", "start", "finish", "baselineStart", "baselineFinish", "startVariance", "finishVariance"]),
        ("work", vec!["id", "name", "work", "baselineWork", "percentComplete", "resourceNames"]),
        ("constraints", vec!["id", "name", "duration", "constraintType", "constraintDate", "deadline"]),
        ("summary", vec!["id", "name", "duration", "start", "finish", "percentComplete", "cost", "work"]),
    ] {
        let cols: Vec<String> = cols.into_iter().map(String::from).collect();
        let tname = table.to_string();
        b.add(&format!("view.table.{table}"), &format!("Table: {}", capitalize(table)), &format!("Show the {table} table's columns."), &[], false, move |s, _| {
            s.view.columns = cols.clone();
            Ok(json!({"table": tname}))
        });
    }
    b.add("view.columns.pick", "Insert Column...", "Open the column picker.", &[], false, |s, _| {
        s.dialog(Dialog::ColumnPicker);
        ok()
    });
    for fd in RESOURCE_FIELDS {
        let fid = fd.id.to_string();
        b.add(&format!("view.resourceColumn.show.{}", fd.id), &format!("Show Resource Column: {}", fd.title), &format!("Add {} to the resource table.", fd.title), &[], false, move |s, _| {
            if !s.view.resource_columns.contains(&fid) {
                s.view.resource_columns.push(fid.clone());
            }
            ok()
        });
        let fid = fd.id.to_string();
        b.add(&format!("view.resourceColumn.hide.{}", fd.id), &format!("Hide Resource Column: {}", fd.title), &format!("Remove {} from the resource table.", fd.title), &[], false, move |s, _| {
            s.view.resource_columns.retain(|c| *c != fid);
            ok()
        });
    }
    for (id, size) in [("small", 11.0), ("normal", 13.0), ("large", 15.0), ("larger", 17.0)] {
        b.add(&format!("view.textSize.{id}"), &format!("Text Size: {}", capitalize(id)), "Change the text size.", &[], false, move |s, _| {
            s.view.font_size = size;
            ok()
        });
    }
    for (id, h) in [("compact", 18.0), ("normal", 22.0), ("roomy", 28.0)] {
        b.add(&format!("view.rowHeight.{id}"), &format!("Row Height: {}", capitalize(id)), "Change the row height.", &[], false, move |s, _| {
            s.view.row_height = h;
            ok()
        });
    }
    b.add("view.tableWidth", "Table Width", "Set the width of the table next to the chart.", &[("px", "number", true, "pixels")], false, |s, v| {
        s.view.table_width = pf64(v, "px").unwrap_or(600.0).clamp(80.0, 3000.0) as f32;
        ok()
    });
    b.add("view.rows", "Visible Rows", "The rows the current view shows (after filters, groups and sorting).", &[], false, |s, _| {
        let p = &s.project;
        Ok(Value::Array(
            s.task_rows()
                .into_iter()
                .map(|r| match r {
                    Row::Task(i) => json!({"id": i + 1, "name": p.tasks[i].name}),
                    Row::Group(g) => json!({"group": g}),
                    Row::ProjectSummary => json!({"projectSummary": p.title}),
                })
                .collect(),
        ))
    });
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The column a column command acts on: "field" if given, else the last clicked column.
fn column_target(s: &Session, v: &Value) -> Result<String, String> {
    let f = pstr(v, "field").unwrap_or_else(|| if s.view.active_column.is_empty() { "name".into() } else { s.view.active_column.clone() });
    let fd = fields::task_field(&f).ok_or_else(|| format!("unknown column '{f}'"))?;
    Ok(fd.id.to_string())
}

/// Change the text style of the target tasks.
fn restyle(s: &mut Session, v: &Value, f: impl Fn(&mut TextStyle)) -> Result<Value, String> {
    let ids = need_targets(s, v)?;
    for &i in &ids {
        let t = &mut s.project.tasks[i];
        let mut st = t.style.clone().unwrap_or_default();
        f(&mut st);
        t.style = if st.is_empty() { None } else { Some(st) };
    }
    Ok(json!({"updated": ids.len()}))
}

fn opt_color(v: &Value, key: &str) -> Result<Option<[u8; 3]>, String> {
    match pstr(v, key) {
        Some(c) if c.trim().is_empty() || c.eq_ignore_ascii_case("automatic") || c.eq_ignore_ascii_case("none") => Ok(None),
        Some(c) => Ok(Some(crate::model::parse_color(&c)?)),
        None => Err(format!("needs '{key}' (#rrggbb, a colour name, or 'automatic')")),
    }
}

pub const TEXT_STYLE_ITEMS: &[(&str, &str)] = &[
    ("all", "All"),
    ("noncritical", "Noncritical Tasks"),
    ("critical", "Critical Tasks"),
    ("milestone", "Milestone Tasks"),
    ("summary", "Summary Tasks"),
    ("inactive", "Inactive Tasks"),
    ("manual", "Manually Scheduled Tasks"),
];

/// Task tab > Font, Format Painter, Format > Text Styles, Add to Timeline, Move, Mode, Inspect.
fn task_format_commands(b: &mut B) {
    for (id, title, which) in [("bold", "Bold", 0u8), ("italic", "Italic", 1), ("underline", "Underline", 2)] {
        b.add(&format!("format.font.{id}"), title, &format!("Turn {} on or off for the selected tasks (or set 'on').", title.to_lowercase()), &[IDS, ("on", "boolean", false, "true/false; default flips it")], true, move |s, v| {
            let ids = need_targets(s, v)?;
            let cur = ids.first().and_then(|&i| s.project.tasks[i].style.as_ref()).and_then(|st| match which {
                0 => st.bold,
                1 => st.italic,
                _ => st.underline,
            });
            let on = pbool(v, "on").unwrap_or(!cur.unwrap_or(false));
            restyle(s, v, |st| match which {
                0 => st.bold = Some(on),
                1 => st.italic = Some(on),
                _ => st.underline = Some(on),
            })
        });
        b.key(match which {
            0 => "Ctrl+B",
            1 => "Ctrl+I",
            _ => "Ctrl+U",
        });
    }
    b.add("format.font.color", "Font Color", "Text colour of the selected tasks ('automatic' resets it).", &[IDS, ("color", "string", true, "#rrggbb, red, blue... or automatic")], true, |s, v| {
        let c = opt_color(v, "color")?;
        restyle(s, v, |st| st.color = c)
    });
    b.add("format.font.background", "Background Color", "Cell background of the selected tasks ('automatic' resets it).", &[IDS, ("color", "string", true, "#rrggbb, yellow... or automatic")], true, |s, v| {
        let c = opt_color(v, "color")?;
        restyle(s, v, |st| st.background = c)
    });
    b.add("format.font.size", "Font Size", "Text size of the selected tasks in points (0 resets it).", &[IDS, ("size", "number", true, "8 to 28 points")], true, |s, v| {
        let n = pf64(v, "size").ok_or("needs a 'size'")?;
        let size = if n <= 0.0 { None } else { Some((n as f32).clamp(8.0, 28.0)) };
        restyle(s, v, |st| st.size = size)
    });
    b.add("format.font.family", "Font", "Typeface of the selected tasks: sans (default) or mono.", &[IDS, ("font", "string", true, "sans or mono")], true, |s, v| {
        let f = req(v, "font")?.to_lowercase();
        let font = match f.as_str() {
            "sans" | "default" | "proportional" => None,
            "mono" | "monospace" | "fixed" => Some("mono".to_string()),
            _ => return Err("font must be sans or mono".into()),
        };
        restyle(s, v, |st| st.font = font.clone())
    });
    b.add("format.font.clear", "Clear Formats", "Remove font and colour formatting from the selected tasks.", &[IDS], true, |s, v| restyle(s, v, |st| *st = TextStyle::default()));
    b.add("format.painter.copy", "Format Painter", "Pick up the selected task's text formatting, to apply with format.painter.apply.", &[IDS], false, |s, v| {
        let ids = need_targets(s, v)?;
        let st = s.project.tasks[ids[0]].style.clone().unwrap_or_default();
        s.style_clipboard = Some(st.clone());
        Ok(json!({"picked": st}))
    });
    b.add("format.painter.apply", "Paint Format", "Apply the formatting picked up by Format Painter to tasks.", &[IDS], true, |s, v| {
        let st = s.style_clipboard.clone().ok_or("pick a format first (format.painter.copy)")?;
        restyle(s, v, |x| *x = st.clone())
    });
    b.add("format.textStyles", "Text Styles...", "Font and colours for a whole kind of row: all rows, critical, milestone, summary, inactive or manually scheduled tasks.", &[], false, |s, _| {
        s.dialog(Dialog::TextStyles);
        Ok(json!({"styles": s.view.text_styles}))
    });
    b.add(
        "format.textStyle.set",
        "Set Text Style",
        "Change the text style of a kind of row (see format.textStyles).",
        &[
            ("item", "string", true, "all, noncritical, critical, milestone, summary, inactive or manual"),
            ("bold", "boolean", false, ""),
            ("italic", "boolean", false, ""),
            ("underline", "boolean", false, ""),
            ("color", "string", false, "#rrggbb or automatic"),
            ("background", "string", false, "#rrggbb or automatic"),
            ("size", "number", false, "points (0 = default)"),
            ("font", "string", false, "sans or mono"),
        ],
        false,
        |s, v| {
            let item = req(v, "item")?;
            if !TEXT_STYLE_ITEMS.iter().any(|(k, _)| *k == item) {
                return Err(format!("item must be one of: {}", TEXT_STYLE_ITEMS.iter().map(|x| x.0).collect::<Vec<_>>().join(", ")));
            }
            let mut st = s.view.text_styles.get(&item).cloned().unwrap_or_default();
            if let Some(x) = pbool(v, "bold") {
                st.bold = Some(x);
            }
            if let Some(x) = pbool(v, "italic") {
                st.italic = Some(x);
            }
            if let Some(x) = pbool(v, "underline") {
                st.underline = Some(x);
            }
            if v.get("color").is_some() {
                st.color = opt_color(v, "color")?;
            }
            if v.get("background").is_some() {
                st.background = opt_color(v, "background")?;
            }
            if let Some(n) = pf64(v, "size") {
                st.size = if n <= 0.0 { None } else { Some((n as f32).clamp(8.0, 28.0)) };
            }
            if let Some(f) = pstr(v, "font") {
                st.font = if f.eq_ignore_ascii_case("mono") { Some("mono".into()) } else { None };
            }
            if st.is_empty() {
                s.view.text_styles.remove(&item);
            } else {
                s.view.text_styles.insert(item.clone(), st);
            }
            Ok(json!({"item": item, "styles": s.view.text_styles}))
        },
    );
    b.add("format.textStyle.reset", "Reset Text Styles", "Remove every text style (rows go back to the default look).", &[], false, |s, _| {
        s.view.text_styles.clear();
        ok()
    });
    b.add("task.addToTimeline", "Add to Timeline", "Show the selected tasks on the Timeline (or set 'on': false to remove them).", &[IDS, ("on", "boolean", false, "default true")], true, |s, v| {
        let ids = need_targets(s, v)?;
        let on = pbool(v, "on").unwrap_or(true);
        for &i in &ids {
            s.project.tasks[i].on_timeline = on;
        }
        if on {
            s.view.show_timeline = true;
        }
        Ok(json!({"updated": ids.len(), "on": on}))
    });
    b.add("task.removeFromTimeline", "Remove from Timeline", "Take the selected tasks off the Timeline.", &[IDS], true, |s, v| {
        let ids = need_targets(s, v)?;
        for &i in &ids {
            s.project.tasks[i].on_timeline = false;
        }
        Ok(json!({"updated": ids.len()}))
    });
    for (id, title, days) in [("1d", "1 Day", 1.0), ("1w", "1 Week", 5.0), ("2w", "2 Weeks", 10.0), ("4w", "4 Weeks", 20.0)] {
        b.add(&format!("task.move.{id}"), &format!("Move {title} Later"), &format!("Move the selected tasks {} later (working time).", title.to_lowercase()), &[IDS], true, move |s, v| {
            let mut p = v.clone();
            if !p.is_object() {
                p = json!({});
            }
            p["days"] = json!(days);
            move_tasks(s, &p, 1.0)
        });
    }
    b.add("task.move.custom", "Move Custom...", "Move the selected tasks by a number of working days (negative = earlier).", &[IDS, ("days", "number", true, "working days, e.g. 3 or -2")], true, |s, v| move_tasks(s, v, 1.0));
    b.add("task.newTasksManual", "New Tasks: Manually Scheduled", "Tasks you add from now on are manually scheduled.", &[], true, |s, _| {
        s.project.settings.new_tasks_manual = true;
        ok()
    });
    b.add("task.newTasksAuto", "New Tasks: Auto Scheduled", "Tasks you add from now on are auto scheduled.", &[], true, |s, _| {
        s.project.settings.new_tasks_manual = false;
        ok()
    });
    b.add("task.inspect", "Inspect", "Show what drives the selected task's dates: predecessors, constraints, calendars, progress and leveling (Task Inspector).", &[IDS], false, |s, v| {
        let ids = need_targets(s, v)?;
        let i = ids[0];
        s.dialog(Dialog::Inspector);
        Ok(crate::schedule::inspect(&s.project, i))
    });
    b.add("task.update", "Update Tasks...", "Enter progress for the selected tasks: % complete, actual and remaining duration, actual start and finish.", &[IDS], false, |s, v| {
        let ids = need_targets(s, v)?;
        s.view.selected = ids.iter().map(|&i| s.project.tasks[i].uid).collect();
        s.dialog(Dialog::UpdateTasks);
        ok()
    });
    b.add("project.baseline.dialog", "Set Baseline...", "Set or clear a baseline (0-10) for the whole plan or the selected tasks.", &[], false, |s, _| {
        s.dialog(Dialog::SetBaseline);
        ok()
    });
}

fn format_extra_commands(b: &mut B) {
    for (al, title) in [("left", "Align Left"), ("center", "Center"), ("right", "Align Right")] {
        let a = al.to_string();
        b.add(&format!("view.column.align.{al}"), title, &format!("{title} the text of a column (default the last clicked column)."), &[("field", "string", false, "column field id")], false, move |s, v| {
            let f = column_target(s, v)?;
            s.view.col_align.insert(f.clone(), a.clone());
            Ok(json!({"column": f, "align": a}))
        });
    }
    b.add("view.column.select", "Select Column", "Make a column the one that alignment and column settings act on.", &[("field", "string", true, "column field id")], false, |s, v| {
        let f = column_target(s, v)?;
        s.view.active_column = f.clone();
        Ok(json!({"column": f}))
    });
    b.add("view.column.title", "Field Settings...", "Give a column its own title (empty puts the standard title back).", &[("field", "string", false, "column field id"), ("title", "string", false, "new title")], false, |s, v| {
        let f = column_target(s, v)?;
        match pstr(v, "title") {
            Some(t) if !t.trim().is_empty() => {
                s.view.col_titles.insert(f.clone(), t.trim().to_string());
            }
            Some(_) => {
                s.view.col_titles.remove(&f);
            }
            None if !s.headless => {
                s.dialog(Dialog::FieldSettings(f.clone()));
                return ok();
            }
            None => {
                s.view.col_titles.remove(&f);
            }
        }
        Ok(json!({"column": f, "title": s.view.col_titles.get(&f)}))
    });
    b.add("view.column.bestFit", "Best Fit", "Size a column to fit its title and contents.", &[("field", "string", false, "column field id")], false, |s, v| {
        let f = column_target(s, v)?;
        let title = s.view.col_titles.get(&f).cloned().unwrap_or_else(|| fields::task_field(&f).map(|d| d.title.to_string()).unwrap_or_default());
        let longest = (0..s.project.tasks.len()).map(|i| fields::task_text(&s.project, i, &f).chars().count() + if f == "name" { 3 * s.project.tasks[i].outline_level as usize } else { 0 }).max().unwrap_or(0).max(title.chars().count());
        let w = (longest as f32 * 7.4 + 18.0).clamp(30.0, 600.0);
        s.view.col_widths.insert(f.clone(), w);
        Ok(json!({"column": f, "width": w}))
    });
    b.add("view.column.resetWidth", "Standard Width", "Put a column back to its standard width.", &[("field", "string", false, "column field id")], false, |s, v| {
        let f = column_target(s, v)?;
        s.view.col_widths.remove(&f);
        ok()
    });
    b.add("view.column.hideActive", "Hide Column", "Hide the last clicked column (or the given one).", &[("field", "string", false, "column field id")], false, |s, v| {
        let f = column_target(s, v)?;
        s.view.columns.retain(|c| *c != f);
        Ok(json!({"columns": s.view.columns}))
    });
    for (id, title, help) in [
        ("predecessors", "Predecessors", "Highlight every task the selected task depends on."),
        ("drivingPredecessors", "Driving Predecessors", "Highlight the chain of tasks that set the selected task's start."),
        ("successors", "Successors", "Highlight every task that depends on the selected task."),
        ("drivenSuccessors", "Driven Successors", "Highlight the tasks whose start the selected task sets."),
    ] {
        let m = id.to_string();
        b.add(&format!("view.taskPath.{id}"), title, help, &[("on", "boolean", false, "true/false; default flips it")], false, move |s, v| {
            let on = pbool(v, "on").unwrap_or(!s.view.task_path.contains(&m));
            s.view.task_path.retain(|x| *x != m);
            if on {
                s.view.task_path.push(m.clone());
            }
            Ok(json!({"taskPath": s.view.task_path}))
        });
    }
    b.add("view.taskPath.clear", "Remove Highlighting", "Turn off all Task Path highlighting.", &[], false, |s, _| {
        s.view.task_path.clear();
        ok()
    });
    for n in 0..=10usize {
        let name = if n == 0 { "Baseline".to_string() } else { format!("Baseline {n}") };
        b.add(&format!("view.baseline.show.{n}"), &name, &format!("Show {name} bars under the task bars."), &[], false, move |s, _| {
            s.view.baseline_shown = n;
            s.view.show_baseline = true;
            Ok(json!({"baseline": n}))
        });
        b.add(&format!("view.slippage.{n}"), &name, &format!("Draw slippage lines from {name}'s start to the current start."), &[], false, move |s, _| {
            s.view.slippage = Some(n);
            Ok(json!({"slippage": n}))
        });
    }
    b.add("view.slippage.none", "No Slippage", "Stop drawing slippage lines.", &[], false, |s, _| {
        s.view.slippage = None;
        ok()
    });
    b.add("view.layout", "Layout...", "Link line style, bar height and the dates beside milestones.", &[], false, |s, _| {
        s.dialog(Dialog::Layout);
        Ok(json!({"links": if s.view.link_style.is_empty() { "elbow" } else { &s.view.link_style }, "barSize": if s.view.bar_size.is_empty() { "medium" } else { &s.view.bar_size }, "dates": if s.view.bar_date_format.is_empty() { "short" } else { &s.view.bar_date_format }}))
    });
    for (id, title) in [("elbow", "Elbow Links"), ("straight", "Straight Links"), ("none", "No Links")] {
        let v2 = id.to_string();
        b.add(&format!("view.layout.links.{id}"), title, "How link lines are drawn.", &[], false, move |s, _| {
            s.view.link_style = v2.clone();
            s.view.show_links = v2 != "none";
            ok()
        });
    }
    for (id, title) in [("small", "Small Bars"), ("medium", "Medium Bars"), ("large", "Large Bars")] {
        let v2 = id.to_string();
        b.add(&format!("view.layout.barSize.{id}"), title, "Bar height.", &[], false, move |s, _| {
            s.view.bar_size = v2.clone();
            ok()
        });
    }
    for (id, title) in [("short", "Short Dates (11/12)"), ("long", "Long Dates (Thu 11/12/26)"), ("none", "No Dates")] {
        let v2 = id.to_string();
        b.add(&format!("view.layout.dateFormat.{id}"), title, "Dates shown beside milestones.", &[], false, move |s, _| {
            s.view.bar_date_format = v2.clone();
            ok()
        });
    }
    b.add("format.barStyles", "Bar Styles...", "Choose the colour of each kind of bar.", &[], false, |s, _| {
        s.dialog(Dialog::BarStyles);
        serde_json::to_value(&s.view.bar_colors).map_err(|e| e.to_string())
    });
    for (kind, title) in [("text", "Text Box"), ("box", "Rectangle"), ("line", "Line"), ("arrow", "Arrow")] {
        let k = kind.to_string();
        b.add(
            &format!("format.drawing.{kind}"),
            &format!("Draw {title}"),
            &format!("Add a {} to the Gantt chart, on the selected task's row (or a given row) at a date.", title.to_lowercase()),
            &[("text", "string", false, "words to show"), ("date", "date", false, "where it starts (default the task's start)"), ("id", "integer", false, "task row to sit on"), ("days", "number", false, "width in days (default 3)"), ("color", "color", false, "#rrggbb")],
            false,
            move |s, v| {
                let idx = targets(s, v)?;
                let task = idx.first().map(|&i| s.project.tasks[i].uid);
                let date = match pstr(v, "date") {
                    Some(d) => parse_date(&d, s.project.settings.default_start_min)?,
                    None => task.and_then(|u| s.project.task_by_uid(u)).map(|t| t.start).unwrap_or(s.project.start),
                };
                let color = if v.get("color").is_some() { parse_color(v)? } else if k == "text" { [60, 60, 60] } else { [220, 120, 40] };
                let id = s.view.drawings.iter().map(|d| d.id).max().unwrap_or(0) + 1;
                let text = pstr(v, "text").unwrap_or_else(|| if k == "text" { "Note".into() } else { String::new() });
                s.view.drawings.push(Drawing {
                    id,
                    kind: k.clone(),
                    date,
                    task,
                    row: 0.0,
                    width_days: pf64(v, "days").unwrap_or(3.0) as f32,
                    height_rows: if k == "line" || k == "arrow" { 0.0 } else { 1.0 },
                    text,
                    color,
                });
                s.dirty = true;
                Ok(json!({"drawing": id}))
            },
        );
    }
    b.add("format.drawing.move", "Move Drawing", "Move a drawing by some days and rows.", &[("drawing", "integer", true, "drawing id"), ("days", "number", false, "days later (negative = earlier)"), ("rows", "number", false, "rows down")], false, |s, v| {
        let id = v.get("drawing").and_then(|x| x.as_u64()).ok_or("drawing must be an id")? as u32;
        let d = s.view.drawings.iter_mut().find(|d| d.id == id).ok_or("no such drawing")?;
        d.date += Duration::minutes((pf64(v, "days").unwrap_or(0.0) * 1440.0).round() as i64);
        d.row += pf64(v, "rows").unwrap_or(0.0) as f32;
        s.dirty = true;
        ok()
    });
    b.add("format.drawing.setText", "Drawing Text", "Change a drawing's words.", &[("drawing", "integer", true, "drawing id"), ("value", "string", true, "text")], false, |s, v| {
        let id = v.get("drawing").and_then(|x| x.as_u64()).ok_or("drawing must be an id")? as u32;
        let d = s.view.drawings.iter_mut().find(|d| d.id == id).ok_or("no such drawing")?;
        d.text = req(v, "value")?;
        s.dirty = true;
        ok()
    });
    b.add("format.drawing.delete", "Delete Drawing", "Remove one drawing.", &[("drawing", "integer", true, "drawing id")], false, |s, v| {
        let id = v.get("drawing").and_then(|x| x.as_u64()).ok_or("drawing must be an id")? as u32;
        s.view.drawings.retain(|d| d.id != id);
        s.dirty = true;
        ok()
    });
    b.add("format.drawing.clear", "Remove All Drawings", "Remove every drawing from the chart.", &[], false, |s, _| {
        s.view.drawings.clear();
        s.dirty = true;
        ok()
    });
}

fn format_commands(b: &mut B) {
    format_extra_commands(b);
    task_format_commands(b);
    for part in ["task", "summary", "milestone", "critical", "progress", "baseline", "manual"] {
        let ps = part.to_string();
        b.add(&format!("format.barColor.{part}"), &format!("{} Bar Color", capitalize(part)), &format!("Color of {part} bars, e.g. #3a7bd5."), &[("color", "color", true, "#rrggbb")], false, move |s, v| {
            let c = parse_color(v)?;
            let bc = &mut s.view.bar_colors;
            match ps.as_str() {
                "task" => bc.task = c,
                "summary" => bc.summary = c,
                "milestone" => bc.milestone = c,
                "critical" => bc.critical = c,
                "progress" => bc.progress = c,
                "baseline" => bc.baseline = c,
                _ => bc.manual = c,
            }
            ok()
        });
    }
    b.add("format.barColor.selected", "Color Selected Bars", "Give the selected tasks' bars their own color (empty resets).", &[IDS, ("color", "color", false, "#rrggbb")], true, |s, v| {
        let c = if pstr(v, "color").map(|c| c.is_empty()).unwrap_or(true) { None } else { Some(parse_color(v)?) };
        for i in need_targets(s, v)? {
            s.project.tasks[i].bar_color = c;
        }
        ok()
    });
    for n in 0..crate::session::GANTT_STYLE_COUNT {
        let (name, _) = gantt_style(n);
        b.add(&format!("format.ganttStyle.{n}"), &format!("Gantt Style: {name}"), &format!("Use the {name} bar colors."), &[], false, move |s, _| {
            s.view.gantt_style = n;
            s.view.bar_colors = gantt_style(n).1;
            ok()
        });
    }
    b.add("format.resetBarColors", "Reset Bar Colors", "Back to the standard bar colors.", &[], false, |s, _| {
        s.view.bar_colors = BarColors::default();
        s.view.gantt_style = 0;
        ok()
    });
}

fn report_commands(b: &mut B) {
    b.add("report.compareProjects", "Compare Projects...", "Compare this plan with another version of it: tasks added, removed, or with different dates or durations.", &[("path", "path", false, "the other plan (.plan.json or .xml)")], false, |s, v| {
        let path = match pstr(v, "path") {
            Some(p) => p,
            None if !s.headless => match rfd::FileDialog::new().set_title("Compare with which plan?").add_filter("Project plans", &["json", "xml", "mpp"]).pick_file() {
                Some(p) => p.to_string_lossy().to_string(),
                None => return ok(),
            },
            None => return Err("report.compareProjects needs a 'path'".into()),
        };
        let (other, _) = crate::io::load(std::path::Path::new(&path))?;
        let v = reports::compare(&s.project, &other);
        if !s.headless {
            s.dialog(Dialog::Report(v["title"].as_str().unwrap_or("").to_string(), v.clone()));
        }
        Ok(v)
    });
    for (id, title) in reports::REPORTS {
        let rid = id.to_string();
        b.add(&format!("report.{id}"), title, &format!("Build the {title} report (shown in a window; returned as a table)."), &[], false, move |s, _| {
            let v = reports::build(&s.project, &rid, now())?;
            if !s.headless {
                s.dialog(Dialog::Report(v["title"].as_str().unwrap_or("").to_string(), v.clone()));
            }
            Ok(v)
        });
    }
}

fn help_commands(b: &mut B) {
    b.add("help.about", "About", "About this app.", &[], false, |s, _| {
        s.dialog(Dialog::About);
        Ok(json!({"name": crate::APP_NAME, "version": env!("CARGO_PKG_VERSION"), "commands": Registry::get().len()}))
    });
    b.add("help.commands", "List Commands", "Every command id and title (optionally filtered).", &[("filter", "string", false, "text or category")], false, |_, v| Ok(Registry::get().list_json(pstr(v, "filter").as_deref())));
    b.add("help.describe", "Describe Command", "A command's help and parameters.", &[("command", "string", true, "command id")], false, |_, v| {
        let id = req(v, "command")?;
        Registry::get().describe(&id).ok_or_else(|| format!("no command '{id}'"))
    });
    b.add("help.agentAccess", "Scripts and Agents...", "How to drive the app from the command line, JSON or MCP.", &[], false, |s, _| {
        s.dialog(Dialog::AgentAccess);
        Ok(json!({"cli": "plancraft-cli run <file> --cmd <id> --params <json> --out <file>", "mcp": "plancraft-cli mcp [--bridge]", "control": format!("tcp://127.0.0.1:{}", crate::DEFAULT_PORT)}))
    });
    b.add("help.show", "Help", "How to use PlanCraft: views, scheduling, shortcuts.", &[], false, |s, _| {
        s.dialog(Dialog::Help);
        let shortcuts: Vec<Value> = Registry::get().commands.iter().filter_map(|c| c.shortcut.as_ref().map(|k| json!({"keys": k, "command": c.id, "title": c.title}))).collect();
        Ok(json!({"shortcuts": shortcuts}))
    });
    b.key("F1");
    b.add("help.gettingStarted", "Show Training", "A short walk-through: build a first plan in five steps.", &[], false, |s, _| {
        s.dialog(Dialog::GettingStarted);
        Ok(json!({"steps": GETTING_STARTED}))
    });
    b.add("help.feedback", "Feedback", "How to report a problem, with details you can copy.", &[], false, |s, _| {
        s.dialog(Dialog::Feedback);
        Ok(json!({"version": env!("CARGO_PKG_VERSION"), "commands": Registry::get().len(), "tasks": s.project.tasks.len()}))
    });
    b.add("project.customFields", "Custom Fields...", "The custom fields (Text1-5, Number1-5, Flag1-5): show them as columns.", &[], false, |s, _| {
        s.dialog(Dialog::CustomFields);
        Ok(Value::Array(TASK_FIELDS.iter().filter(|f| f.id.starts_with("text") || f.id.starts_with("number") || f.id.starts_with("flag")).map(|f| json!({"id": f.id, "title": f.title, "shown": s.view.columns.iter().any(|c| c == f.id)})).collect()))
    });
    b.add("app.runScript", "Run Macro...", "Run a saved list of commands from a JSON file ([{\"cmd\":..., \"params\":{...}}] or {\"steps\":[...]}).", &[("path", "path", false, "script file")], false, |s, v| {
        let path = match pstr(v, "path") {
            Some(p) => p,
            None if !s.headless => match rfd::FileDialog::new().add_filter("Command script", &["json"]).pick_file() {
                Some(p) => p.to_string_lossy().to_string(),
                None => return ok(),
            },
            None => return Err("app.runScript needs a 'path'".into()),
        };
        let text = std::fs::read_to_string(&path).map_err(|e| format!("can't read {path}: {e}"))?;
        let parsed: Value = serde_json::from_str(&text).map_err(|e| format!("{path} is not JSON: {e}"))?;
        let steps = match parsed {
            Value::Array(a) => Value::Array(a),
            Value::Object(o) if o.contains_key("steps") => o["steps"].clone(),
            _ => return Err("the script must be a list of {\"cmd\",\"params\"} steps".into()),
        };
        Registry::get().run(s, "app.batch", &json!({"steps": steps}))
    });
    b.add("app.saveLogAsScript", "Record Macro (Save Log as Script)...", "Save the commands run so far in this session as a script that app.runScript can replay.", &[("path", "path", false, "file to write")], false, |s, v| {
        let path = match pstr(v, "path") {
            Some(p) => p,
            None if !s.headless => match rfd::FileDialog::new().add_filter("Command script", &["json"]).set_file_name("macro.json").save_file() {
                Some(p) => p.to_string_lossy().to_string(),
                None => return ok(),
            },
            None => return Err("needs a 'path'".into()),
        };
        let steps: Vec<Value> = s
            .log
            .iter()
            .filter_map(|line| {
                let (cmd, params) = line.split_once(' ').unwrap_or((line.as_str(), "{}"));
                let mutating = Registry::get().find(cmd).map(|c| c.mutates).unwrap_or(false);
                mutating.then(|| json!({"cmd": cmd, "params": serde_json::from_str::<Value>(params).unwrap_or(json!({}))}))
            })
            .collect();
        let n = steps.len();
        std::fs::write(&path, serde_json::to_string_pretty(&steps).unwrap()).map_err(|e| e.to_string())?;
        Ok(json!({"saved": path, "steps": n}))
    });
    for tab in ["task", "resource", "report", "project", "view", "help", "format"] {
        let t = tab.to_string();
        b.add(&format!("app.ribbon.{tab}"), &format!("Show the {} Tab", capitalize(tab)), "Switch the ribbon to this tab.", &[], false, move |s, _| {
            s.request(UiRequest::RibbonTab(t.clone()));
            ok()
        });
    }
    b.add("app.commandPalette", "Command Palette...", "Search and run any command.", &[], false, |s, _| {
        s.dialog(Dialog::CommandPalette);
        ok()
    });
    b.key("Ctrl+Shift+P");
    b.add("app.log", "Command Log", "The commands run in this session.", &[], false, |s, _| Ok(json!(s.log)));
    b.add("app.batch", "Run Several Commands", "Run a list of {\"cmd\",\"params\"} steps in order; stops at the first error.", &[("steps", "object[]", true, "[{\"cmd\":..., \"params\":{...}}]")], false, |s, v| {
        let steps = v.get("steps").and_then(|x| x.as_array()).ok_or("steps must be a list")?.clone();
        let mut out = vec![];
        for (k, st) in steps.iter().enumerate() {
            let cmd = st.get("cmd").or_else(|| st.get("command")).and_then(|x| x.as_str()).ok_or_else(|| format!("step {} has no cmd", k + 1))?;
            let params = st.get("params").cloned().unwrap_or(json!({}));
            let r = Registry::get().run(s, cmd, &params).map_err(|e| format!("step {} ({cmd}): {e}", k + 1))?;
            out.push(r);
        }
        Ok(Value::Array(out))
    });
}
