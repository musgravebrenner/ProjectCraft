//! The scheduling engine: forward and backward passes, slack, critical path, rollups, costs,
//! overallocation and leveling. Rules are written up in MECHANICS.md.

use crate::calendar::Calendar;
use crate::model::*;
use chrono::{Duration, NaiveDateTime};
use std::collections::{BTreeSet, HashMap};

struct Graph {
    leaves: Vec<usize>,
    /// Effective incoming links of each leaf: (pred index, link).
    incoming: HashMap<usize, Vec<(usize, Link)>>,
    /// Effective outgoing links of each leaf: (successor index, link).
    outgoing: HashMap<usize, Vec<(usize, Link)>>,
    order: Vec<usize>,
    circular: bool,
}

fn leaves_of(p: &Project, i: usize, summary: &[bool]) -> Vec<usize> {
    if !summary[i] {
        return vec![i];
    }
    p.descendants(i).filter(|&j| !summary[j]).collect()
}

fn ancestors(p: &Project, i: usize) -> Vec<usize> {
    let mut out = vec![];
    let mut cur = i;
    while let Some(par) = p.parent(cur) {
        out.push(par);
        cur = par;
    }
    out
}

fn build_graph(p: &Project, summary: &[bool]) -> Graph {
    let n = p.tasks.len();
    let uid_ix: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    let leaves: Vec<usize> = (0..n).filter(|&i| !summary[i]).collect();
    let mut incoming: HashMap<usize, Vec<(usize, Link)>> = HashMap::new();
    let mut outgoing: HashMap<usize, Vec<(usize, Link)>> = HashMap::new();
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &s in &leaves {
        let anc = ancestors(p, s);
        let mut owners = vec![s];
        owners.extend(anc.iter().copied());
        for &x in &owners {
            for l in &p.tasks[x].predecessors {
                let Some(&pi) = uid_ix.get(&l.pred) else { continue };
                if pi == x || anc.contains(&pi) || pi == s {
                    continue;
                }
                if !p.tasks[pi].active {
                    continue;
                }
                // a predecessor inside the owner (child linked to its own summary) is ignored
                if p.descendants(x).contains(&pi) {
                    continue;
                }
                incoming.entry(s).or_default().push((pi, l.clone()));
                for q in leaves_of(p, pi, summary) {
                    if q != s {
                        edges.insert((q, s));
                    }
                }
            }
        }
    }
    // outgoing: for each leaf p, every (X, link) where link.pred is p or an ancestor of p
    for (xi, t) in p.tasks.iter().enumerate() {
        for l in &t.predecessors {
            let Some(&pi) = uid_ix.get(&l.pred) else { continue };
            if pi == xi || !p.tasks[pi].active {
                continue;
            }
            for q in leaves_of(p, pi, summary) {
                if p.descendants(xi).contains(&q) || leaves_of(p, xi, summary).contains(&q) {
                    continue;
                }
                outgoing.entry(q).or_default().push((xi, l.clone()));
            }
        }
    }
    // Kahn's algorithm, ties broken by ID order.
    let mut indeg: HashMap<usize, usize> = leaves.iter().map(|&l| (l, 0)).collect();
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(a, b) in &edges {
        adj.entry(a).or_default().push(b);
        *indeg.get_mut(&b).unwrap() += 1;
    }
    let mut ready: BTreeSet<usize> = leaves.iter().copied().filter(|l| indeg[l] == 0).collect();
    let mut order = vec![];
    while let Some(&x) = ready.iter().next() {
        ready.remove(&x);
        order.push(x);
        if let Some(v) = adj.get(&x) {
            for &y in v {
                let d = indeg.get_mut(&y).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.insert(y);
                }
            }
        }
    }
    let circular = order.len() < leaves.len();
    if circular {
        for &l in &leaves {
            if !order.contains(&l) {
                order.push(l);
            }
        }
    }
    Graph { leaves, incoming, outgoing, order, circular }
}

fn fwd(cal: &Calendar, s: NaiveDateTime, d: &Dur) -> NaiveDateTime {
    if d.elapsed {
        s + Duration::minutes(d.minutes)
    } else if d.minutes == 0 {
        s
    } else {
        cal.add_work(s, d.minutes)
    }
}
fn back(cal: &Calendar, f: NaiveDateTime, d: &Dur) -> NaiveDateTime {
    if d.elapsed {
        f - Duration::minutes(d.minutes)
    } else if d.minutes == 0 {
        f
    } else {
        cal.sub_work(f, d.minutes)
    }
}
fn shift(cal: &Calendar, t: NaiveDateTime, lag: &Dur) -> NaiveDateTime {
    if lag.elapsed {
        t + Duration::minutes(lag.minutes)
    } else if lag.minutes >= 0 {
        cal.add_work(t, lag.minutes)
    } else {
        cal.sub_work(t, -lag.minutes)
    }
}
fn shift_back(cal: &Calendar, t: NaiveDateTime, lag: &Dur) -> NaiveDateTime {
    if lag.elapsed {
        t - Duration::minutes(lag.minutes)
    } else if lag.minutes >= 0 {
        cal.sub_work(t, lag.minutes)
    } else {
        cal.add_work(t, -lag.minutes)
    }
}

/// Recalculate every computed field of the project.
pub fn schedule(p: &mut Project) {
    let n = p.tasks.len();
    p.warnings.clear();
    let summary: Vec<bool> = (0..n).map(|i| p.is_summary(i)).collect();
    for i in 0..n {
        p.tasks[i].summary = summary[i];
        if p.tasks[i].baselines.len() < 11 {
            p.tasks[i].baselines.resize(11, None);
        }
    }
    if p.baseline_saved.len() < 11 {
        p.baseline_saved.resize(11, None);
    }
    let pcal = p.cal().clone();
    let proj_start = pcal.snap_forward(p.start);
    if n == 0 {
        p.finish = proj_start;
        p.circular = false;
        compute_costs(p);
        return;
    }
    let g = build_graph(p, &summary);
    p.circular = g.circular;
    if g.circular {
        p.warnings.push("Circular link found: some tasks could not be ordered".into());
    }
    let has_alap = g.leaves.iter().any(|&i| p.tasks[i].constraint == ConstraintKind::ALAP && !p.tasks[i].manual);
    let passes = if has_alap { 3 } else { 1 };
    for pass in 0..passes {
        forward(p, &g, &summary, proj_start, pass > 0);
        rollup_dates(p, &summary);
        backward(p, &g, &summary, &pcal);
        if has_alap {
            for &i in &g.leaves {
                let t = &mut p.tasks[i];
                if t.constraint == ConstraintKind::ALAP && !t.manual && t.percent_complete <= 0.0 {
                    t.start = t.late_start;
                    t.finish = t.late_finish;
                }
            }
            rollup_dates(p, &summary);
        }
    }
    // slack and critical
    let crit = p.settings.critical_slack;
    for &i in &g.leaves {
        let cal = p.task_cal(&p.tasks[i]).clone();
        let t = &p.tasks[i];
        let ss = cal.work_between(t.early_start, t.late_start);
        let fs = cal.work_between(t.early_finish, t.late_finish);
        let total = ss.min(fs);
        // free slack: room before the first successor needs this task
        let mut free = total;
        if let Some(outs) = g.outgoing.get(&i) {
            for (si, l) in outs {
                let s = &p.tasks[*si];
                let room = match l.kind {
                    LinkKind::FS => cal.work_between(shift(&cal, t.early_finish, &l.lag), s.early_start),
                    LinkKind::SS => cal.work_between(shift(&cal, t.early_start, &l.lag), s.early_start),
                    LinkKind::FF => cal.work_between(shift(&cal, t.early_finish, &l.lag), s.early_finish),
                    LinkKind::SF => cal.work_between(shift(&cal, t.early_start, &l.lag), s.early_finish),
                };
                free = free.min(room.max(0));
            }
        }
        let t = &mut p.tasks[i];
        t.total_slack = total;
        t.free_slack = free.min(total).max(if total < 0 { total } else { 0 });
        t.critical = total <= crit && t.percent_complete < 100.0 && t.active;
        t.missed_deadline = t.deadline.map(|d| t.finish > d).unwrap_or(false);
    }
    rollup_summaries(p, &summary);
    p.finish = g.leaves.iter().filter(|&&i| p.tasks[i].active).map(|&i| p.tasks[i].finish).max().unwrap_or(proj_start);
    // late flag (behind schedule versus status date)
    if let Some(sd) = p.status_date {
        for t in p.tasks.iter_mut() {
            let expected = if sd >= t.finish {
                100.0
            } else if sd <= t.start {
                0.0
            } else {
                let tot = (t.finish - t.start).num_minutes().max(1) as f64;
                (sd - t.start).num_minutes() as f64 / tot * 100.0
            };
            t.late = t.percent_complete + 0.5 < expected && t.active;
        }
    } else {
        for t in p.tasks.iter_mut() {
            t.late = false;
        }
    }
    for t in p.tasks.iter() {
        if t.missed_deadline {
            p.warnings.push(format!("'{}' finishes after its deadline", t.name));
        }
        if t.total_slack < 0 && !t.summary {
            p.warnings.push(format!("'{}' has negative slack (a constraint or deadline cannot be met)", t.name));
        }
    }
    compute_costs(p);
}

fn pred_dates(p: &Project, i: usize, summary: &[bool]) -> (NaiveDateTime, NaiveDateTime) {
    if summary[i] {
        let mut s: Option<NaiveDateTime> = None;
        let mut f: Option<NaiveDateTime> = None;
        for j in p.descendants(i) {
            if !summary[j] && p.tasks[j].active {
                s = Some(s.map_or(p.tasks[j].start, |x| x.min(p.tasks[j].start)));
                f = Some(f.map_or(p.tasks[j].finish, |x| x.max(p.tasks[j].finish)));
            }
        }
        (s.unwrap_or(p.start), f.unwrap_or(p.start))
    } else {
        (p.tasks[i].start, p.tasks[i].finish)
    }
}

fn late_dates(p: &Project, i: usize, summary: &[bool]) -> (NaiveDateTime, NaiveDateTime) {
    if summary[i] {
        let mut s: Option<NaiveDateTime> = None;
        let mut f: Option<NaiveDateTime> = None;
        for j in p.descendants(i) {
            if !summary[j] && p.tasks[j].active {
                s = Some(s.map_or(p.tasks[j].late_start, |x| x.min(p.tasks[j].late_start)));
                f = Some(f.map_or(p.tasks[j].late_finish, |x| x.max(p.tasks[j].late_finish)));
            }
        }
        (s.unwrap_or(p.finish), f.unwrap_or(p.finish))
    } else {
        (p.tasks[i].late_start, p.tasks[i].late_finish)
    }
}

fn forward(p: &mut Project, g: &Graph, summary: &[bool], proj_start: NaiveDateTime, keep_alap: bool) {
    for &i in &g.order {
        let cal = p.task_cal(&p.tasks[i]).clone();
        let t = &p.tasks[i];
        let dur = t.duration;
        let start;
        if t.manual {
            start = t.manual_start.unwrap_or(proj_start);
        } else {
            let mut es = cal.snap_forward(proj_start);
            if let Some(links) = g.incoming.get(&i) {
                for (pi, l) in links {
                    let (ps, pf) = pred_dates(p, *pi, summary);
                    let cand = match l.kind {
                        LinkKind::FS => {
                            if dur.minutes == 0 && l.lag.minutes == 0 {
                                pf
                            } else {
                                cal.snap_forward(shift(&cal, pf, &l.lag))
                            }
                        }
                        LinkKind::SS => cal.snap_forward(shift(&cal, ps, &l.lag)),
                        LinkKind::FF => back(&cal, shift(&cal, pf, &l.lag), &dur),
                        LinkKind::SF => back(&cal, shift(&cal, ps, &l.lag), &dur),
                    };
                    if cand > es {
                        es = cand;
                    }
                }
            }
            if let Some(cd) = t.constraint_date {
                match t.constraint {
                    ConstraintKind::SNET => es = es.max(cal.snap_forward(cd)),
                    ConstraintKind::FNET => es = es.max(back(&cal, cd, &dur)),
                    ConstraintKind::MSO => es = cal.snap_forward(cd),
                    ConstraintKind::MFO => es = back(&cal, cd, &dur),
                    ConstraintKind::SNLT => {
                        if p.settings.honor_constraints && es > cd {
                            es = cal.snap_forward(cd).min(cd.max(cal.snap_forward(cd)));
                        }
                    }
                    ConstraintKind::FNLT => {
                        if p.settings.honor_constraints && fwd(&cal, es, &dur) > cd {
                            es = back(&cal, cd, &dur);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(ld) = t.level_delay_to {
                es = es.max(cal.snap_forward(ld));
            }
            if let Some(a) = t.actual_start {
                if t.percent_complete > 0.0 {
                    es = a;
                }
            }
            start = es;
        }
        let mut finish = fwd(&cal, start, &dur);
        if let Some(af) = t.actual_finish {
            if t.percent_complete >= 100.0 {
                finish = af.max(start);
            }
        }
        let is_alap = t.constraint == ConstraintKind::ALAP && !t.manual && t.percent_complete <= 0.0;
        let t = &mut p.tasks[i];
        t.early_start = start;
        t.early_finish = finish;
        if !(keep_alap && is_alap) || true {
            t.start = start;
            t.finish = finish;
        }
    }
}

fn rollup_dates(p: &mut Project, summary: &[bool]) {
    for i in (0..p.tasks.len()).rev() {
        if summary[i] {
            let (s, f) = pred_dates(p, i, summary);
            let t = &mut p.tasks[i];
            t.start = s;
            t.finish = f;
        }
    }
}

fn backward(p: &mut Project, g: &Graph, summary: &[bool], pcal: &Calendar) {
    let pf = g.leaves.iter().filter(|&&i| p.tasks[i].active).map(|&i| p.tasks[i].early_finish).max().unwrap_or(p.start);
    p.finish = pf;
    for &i in g.order.iter().rev() {
        let cal = p.task_cal(&p.tasks[i]).clone();
        let t = &p.tasks[i];
        let dur = t.duration;
        let mut lf = pcal.snap_backward(pf).max(t.early_finish);
        if let Some(outs) = g.outgoing.get(&i) {
            for (si, l) in outs {
                let (sls, slf) = late_dates(p, *si, summary);
                let cand = match l.kind {
                    LinkKind::FS => cal.snap_backward(shift_back(&cal, sls, &l.lag)),
                    LinkKind::SS => fwd(&cal, shift_back(&cal, sls, &l.lag), &dur),
                    LinkKind::FF => shift_back(&cal, slf, &l.lag),
                    LinkKind::SF => fwd(&cal, shift_back(&cal, slf, &l.lag), &dur),
                };
                if cand < lf {
                    lf = cand;
                }
            }
        }
        if let Some(cd) = t.constraint_date {
            match t.constraint {
                ConstraintKind::FNLT => lf = lf.min(cd),
                ConstraintKind::SNLT => lf = lf.min(fwd(&cal, cd, &dur)),
                ConstraintKind::MSO | ConstraintKind::MFO => lf = t.early_finish,
                _ => {}
            }
        }
        if let Some(dl) = t.deadline {
            lf = lf.min(dl);
        }
        if t.manual {
            lf = lf.min(t.early_finish.max(lf));
        }
        let ls = back(&cal, lf, &dur);
        let t = &mut p.tasks[i];
        t.late_finish = lf;
        t.late_start = ls;
    }
}

fn rollup_summaries(p: &mut Project, summary: &[bool]) {
    let pcal = p.cal().clone();
    for i in (0..p.tasks.len()).rev() {
        if !summary[i] {
            continue;
        }
        let r = p.descendants(i);
        let mut es: Option<NaiveDateTime> = None;
        let mut ef: Option<NaiveDateTime> = None;
        let mut ls: Option<NaiveDateTime> = None;
        let mut lf: Option<NaiveDateTime> = None;
        let mut slack = i64::MAX;
        let mut crit = false;
        let mut dur_sum = 0f64;
        let mut done_sum = 0f64;
        let mut late = false;
        let mut deadline_miss = false;
        for j in r.clone() {
            if summary[j] || !p.tasks[j].active {
                continue;
            }
            let c = &p.tasks[j];
            es = Some(es.map_or(c.early_start, |x| x.min(c.early_start)));
            ef = Some(ef.map_or(c.early_finish, |x| x.max(c.early_finish)));
            ls = Some(ls.map_or(c.late_start, |x| x.min(c.late_start)));
            lf = Some(lf.map_or(c.late_finish, |x| x.max(c.late_finish)));
            slack = slack.min(c.total_slack);
            crit |= c.critical;
            let w = c.duration.minutes.max(1) as f64;
            dur_sum += w;
            done_sum += w * c.percent_complete / 100.0;
            late |= c.late;
            deadline_miss |= c.missed_deadline;
        }
        let (s, f) = pred_dates(p, i, summary);
        let t = &mut p.tasks[i];
        t.start = s;
        t.finish = f;
        t.early_start = es.unwrap_or(s);
        t.early_finish = ef.unwrap_or(f);
        t.late_start = ls.unwrap_or(s);
        t.late_finish = lf.unwrap_or(f);
        t.total_slack = if slack == i64::MAX { 0 } else { slack };
        t.free_slack = 0;
        t.critical = crit;
        t.late = late;
        t.missed_deadline = t.missed_deadline || deadline_miss;
        let work_min = pcal.work_between(s, f);
        t.duration = Dur { minutes: work_min, unit: t.duration.unit, elapsed: false, estimated: false };
        t.percent_complete = if dur_sum > 0.0 { (done_sum / dur_sum * 100.0).round() } else { 0.0 };
    }
}

/// Work and cost of assignments, tasks (with summary rollups) and resources; overallocation.
pub fn compute_costs(p: &mut Project) {
    let res: HashMap<u32, Resource> = p.resources.iter().map(|r| (r.uid, r.clone())).collect();
    for a in p.assignments.iter_mut() {
        a.cost = match res.get(&a.resource) {
            Some(r) => match r.kind {
                ResourceKind::Work => a.work as f64 / 60.0 * r.std_rate + r.cost_per_use,
                ResourceKind::Material => a.units * r.std_rate + r.cost_per_use,
                ResourceKind::Cost => a.cost_amount,
            },
            None => 0.0,
        };
    }
    let n = p.tasks.len();
    for i in 0..n {
        if p.tasks[i].summary {
            continue;
        }
        let uid = p.tasks[i].uid;
        let mut work = 0;
        let mut cost = p.tasks[i].fixed_cost;
        for a in p.assignments.iter().filter(|a| a.task == uid) {
            if res.get(&a.resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false) {
                work += a.work;
            }
            cost += a.cost;
        }
        p.tasks[i].work = work;
        p.tasks[i].cost = cost;
    }
    for i in (0..n).rev() {
        if !p.tasks[i].summary {
            continue;
        }
        let mut work = 0;
        let mut cost = p.tasks[i].fixed_cost;
        for j in p.descendants(i) {
            if !p.tasks[j].summary && p.tasks[j].active {
                work += p.tasks[j].work;
                cost += p.tasks[j].cost;
            }
        }
        p.tasks[i].work = work;
        p.tasks[i].cost = cost;
    }
    // resources
    let tasks: HashMap<u32, (NaiveDateTime, NaiveDateTime, bool)> =
        p.tasks.iter().map(|t| (t.uid, (t.start, t.finish, t.active && !t.summary))).collect();
    for r in p.resources.iter_mut() {
        let mine: Vec<&Assignment> = p.assignments.iter().filter(|a| a.resource == r.uid).collect();
        r.work = mine.iter().map(|a| a.work).sum();
        r.cost = mine.iter().map(|a| a.cost).sum();
        if r.kind != ResourceKind::Work {
            r.peak = 0.0;
            r.overallocated = false;
            continue;
        }
        let mut events: Vec<(NaiveDateTime, f64)> = vec![];
        for a in &mine {
            if let Some(&(s, f, act)) = tasks.get(&a.task) {
                if act && f > s {
                    events.push((s, a.units));
                    events.push((f, -a.units));
                }
            }
        }
        events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));
        let mut cur = 0.0f64;
        let mut peak = 0.0f64;
        for (_, d) in events {
            cur += d;
            peak = peak.max(cur);
        }
        r.peak = peak;
        r.overallocated = peak > r.max_units + 1e-6;
    }
}

/// Find the first overallocation: (task index to delay, moment to delay it to).
fn find_conflict(p: &Project, only_tasks: Option<&[u32]>, only_res: Option<u32>) -> Option<(usize, NaiveDateTime)> {
    for r in &p.resources {
        if r.kind != ResourceKind::Work || only_res.map(|u| u != r.uid).unwrap_or(false) {
            continue;
        }
        let mut items: Vec<(usize, f64)> = p
            .assignments
            .iter()
            .filter(|a| a.resource == r.uid)
            .filter_map(|a| {
                let i = p.task_index(a.task)?;
                let t = &p.tasks[i];
                if t.summary || !t.active || t.finish <= t.start || t.percent_complete >= 100.0 {
                    return None;
                }
                Some((i, a.units))
            })
            .collect();
        items.sort_by(|a, b| {
            let ta = &p.tasks[a.0];
            let tb = &p.tasks[b.0];
            ta.start.cmp(&tb.start).then(tb.priority.cmp(&ta.priority)).then(a.0.cmp(&b.0))
        });
        for (k, &(i, u)) in items.iter().enumerate() {
            let t = &p.tasks[i];
            // load from earlier-ordered tasks that overlap this task's start
            let overlapping: Vec<usize> = items[..k]
                .iter()
                .filter(|(j, _)| {
                    let o = &p.tasks[*j];
                    o.start < t.finish && o.finish > t.start
                })
                .map(|(j, _)| *j)
                .collect();
            let load: f64 = items[..k].iter().filter(|(j, _)| overlapping.contains(j)).map(|(_, u2)| *u2).sum();
            if load + u > r.max_units + 1e-6 {
                // pick the one to delay: lower priority (or later in order) is delayed
                let blocker_finish = overlapping.iter().map(|&j| p.tasks[j].finish).min().unwrap();
                let fixed = matches!(t.constraint, ConstraintKind::MSO | ConstraintKind::MFO) || t.manual || t.percent_complete > 0.0 || only_tasks.map(|o| !o.contains(&t.uid)).unwrap_or(false);
                if fixed {
                    continue;
                }
                return Some((i, blocker_finish));
            }
        }
    }
    None
}

/// Delay tasks until no work resource is over its maximum units. Returns how many delays were added.
pub fn level(p: &mut Project) -> usize {
    level_where(p, None, None)
}

/// Level only some tasks (only they may be delayed) and/or only one resource.
pub fn level_where(p: &mut Project, only_tasks: Option<&[u32]>, only_res: Option<u32>) -> usize {
    let mut count = 0;
    schedule(p);
    for _ in 0..500 {
        match find_conflict(p, only_tasks, only_res) {
            Some((i, to)) => {
                let cur = p.tasks[i].level_delay_to;
                if cur.map(|c| c >= to).unwrap_or(false) {
                    break;
                }
                p.tasks[i].level_delay_to = Some(to);
                count += 1;
                schedule(p);
            }
            None => break,
        }
    }
    count
}

pub fn clear_leveling(p: &mut Project) {
    for t in p.tasks.iter_mut() {
        t.level_delay_to = None;
    }
    schedule(p);
}

/// Work of an assignment that falls in [a, b) with a flat contour.
pub fn assignment_work_in(p: &Project, asg: &Assignment, a: NaiveDateTime, b: NaiveDateTime) -> i64 {
    let Some(t) = p.task_by_uid(asg.task) else { return 0 };
    if t.finish <= t.start {
        return 0;
    }
    let cal = p.task_cal(t);
    let total = cal.work_between(t.start, t.finish).max(1);
    let lo = a.max(t.start);
    let hi = b.min(t.finish);
    if hi <= lo {
        return 0;
    }
    let part = cal.work_between(lo, hi);
    ((asg.work as f64) * part as f64 / total as f64).round() as i64
}

/// What drives a task's dates, for the Task Inspector: mode, progress, constraint, the
/// predecessor that sets its start, calendars, leveling delay and warnings.
pub fn inspect(p: &Project, i: usize) -> serde_json::Value {
    use serde_json::json;
    let t = &p.tasks[i];
    let mut factors: Vec<serde_json::Value> = vec![];
    if t.manual {
        factors.push(json!({"kind": "manual", "text": "Manually scheduled: its dates are the ones you typed, not calculated from links."}));
    }
    if let Some(a) = t.actual_start {
        factors.push(json!({"kind": "actualStart", "text": format!("Actual start: {}", fmt_date(a))}));
    }
    // predecessors ranked by the date each would allow the task to start
    let mut preds: Vec<(f64, serde_json::Value)> = vec![];
    for l in &t.predecessors {
        let Some(pi) = p.task_index(l.pred) else { continue };
        let pt = &p.tasks[pi];
        let lag = Duration::minutes(l.lag.minutes);
        let implied = match l.kind {
            LinkKind::FS => pt.finish + lag,
            LinkKind::SS => pt.start + lag,
            LinkKind::FF => pt.finish + lag - (t.finish - t.start),
            LinkKind::SF => pt.start + lag - (t.finish - t.start),
        };
        preds.push((
            implied.and_utc().timestamp() as f64,
            json!({"id": pi + 1, "name": pt.name, "type": l.kind.abbr(), "lag": l.lag.format(&p.settings), "allowsStartFrom": fmt_date(implied)}),
        ));
    }
    preds.sort_by(|a, b| b.0.total_cmp(&a.0));
    let driving = if t.manual || t.actual_start.is_some() { None } else { preds.first().map(|x| x.1.clone()) };
    if let Some(d) = &driving {
        factors.push(json!({"kind": "predecessor", "text": format!("Predecessor {} '{}' ({} {}) sets the start.", d["id"], d["name"].as_str().unwrap_or(""), d["type"].as_str().unwrap_or(""), d["lag"].as_str().unwrap_or(""))}));
    }
    if t.constraint.needs_date() {
        factors.push(json!({"kind": "constraint", "text": format!("Constraint: {} {}", t.constraint.title(), t.constraint_date.map(fmt_date).unwrap_or_default())}));
    }
    if let Some(ld) = t.level_delay_to {
        factors.push(json!({"kind": "leveling", "text": format!("Leveling delayed it to {}", fmt_date(ld))}));
    }
    let mut calendars = vec![format!("Project calendar: {}", p.calendar)];
    if let Some(c) = &t.calendar {
        calendars.push(format!("Task calendar: {c}"));
    }
    for a in p.assignments_of_task(t.uid) {
        if let Some(r) = p.resource_by_uid(a.resource) {
            if let Some(c) = &r.calendar {
                calendars.push(format!("{}: {c}", r.name));
            }
        }
    }
    if driving.is_none() && !t.manual && t.actual_start.is_none() && !t.constraint.needs_date() {
        factors.push(json!({"kind": "projectStart", "text": format!("Starts with the project ({})", fmt_date(p.start))}));
    }
    let mut warnings = vec![];
    if t.missed_deadline {
        warnings.push(format!("Finishes after its deadline ({})", t.deadline.map(fmt_date).unwrap_or_default()));
    }
    if t.total_slack < 0 {
        warnings.push("Negative slack: a constraint or deadline can't be met.".to_string());
    }
    for a in p.assignments_of_task(t.uid) {
        if let Some(r) = p.resource_by_uid(a.resource) {
            if r.overallocated {
                warnings.push(format!("{} is overallocated", r.name));
            }
        }
    }
    json!({
        "id": i + 1, "name": t.name, "start": fmt_date(t.start), "finish": fmt_date(t.finish),
        "mode": if t.manual { "Manually Scheduled" } else { "Auto Scheduled" },
        "factors": factors, "predecessors": preds.into_iter().map(|x| x.1).collect::<Vec<_>>(),
        "calendars": calendars, "warnings": warnings,
    })
}
