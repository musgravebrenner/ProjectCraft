//! Developer tool: compare what the .mpp reader reads with the scheduling program's own XML
//! export of the same file (FILE.ref.xml), field by field.
//! cargo run --example mppcheck -- FILE.mpp...
use plancraft::io::import_xml;

fn main() {
    let mut total = 0;
    let mut bad = 0;
    for f in std::env::args().skip(1) {
        let (xml, _) = plancraft::mpp::to_xml(std::path::Path::new(&f)).unwrap();
        let got = import_xml(&xml).unwrap();
        let want = import_xml(&std::fs::read_to_string(f.replace(".mpp", ".ref.xml")).unwrap()).unwrap();
        let mut diff = |what: String, a: String, b: String| {
            total += 1;
            if a != b {
                bad += 1;
                println!("{f}: {what}: read {a:?}, program says {b:?}");
            }
        };
        diff("title".into(), got.title.clone(), want.title.clone());
        diff("manager".into(), got.manager.clone(), want.manager.clone());
        diff("task count".into(), got.tasks.len().to_string(), want.tasks.len().to_string());
        for w in &want.tasks {
            let Some(g) = got.tasks.iter().find(|t| t.uid == w.uid) else {
                diff(format!("task {}", w.uid), "missing".into(), w.name.clone());
                continue;
            };
            let n = &w.name;
            diff(format!("{n} name"), g.name.clone(), w.name.clone());
            diff(format!("{n} level"), g.outline_level.to_string(), w.outline_level.to_string());
            diff(format!("{n} duration"), format!("{} {:?}", g.duration.minutes, g.duration.unit), format!("{} {:?}", w.duration.minutes, w.duration.unit));
            diff(format!("{n} manual"), g.manual.to_string(), w.manual.to_string());
            diff(format!("{n} active"), g.active.to_string(), w.active.to_string());
            diff(format!("{n} milestone"), g.milestone.to_string(), w.milestone.to_string());
            diff(format!("{n} priority"), g.priority.to_string(), w.priority.to_string());
            diff(format!("{n} percent"), g.percent_complete.to_string(), w.percent_complete.to_string());
            diff(format!("{n} constraint"), format!("{:?} {:?}", g.constraint, g.constraint_date), format!("{:?} {:?}", w.constraint, w.constraint_date));
            diff(format!("{n} deadline"), format!("{:?}", g.deadline), format!("{:?}", w.deadline));
            diff(format!("{n} actual start"), format!("{:?}", g.actual_start), format!("{:?}", w.actual_start));
            diff(format!("{n} actual finish"), format!("{:?}", g.actual_finish), format!("{:?}", w.actual_finish));
            diff(format!("{n} fixed cost"), format!("{:.2}", g.fixed_cost), format!("{:.2}", w.fixed_cost));
            diff(format!("{n} notes"), g.notes.clone(), w.notes.replace("\r\n", "\n").trim().to_string());
            diff(format!("{n} manual start"), format!("{:?}", g.manual_start), format!("{:?}", w.manual_start));
            let links = |t: &plancraft::model::Task| {
                let mut v: Vec<String> = t.predecessors.iter().map(|l| format!("{}{:?}{}{:?}", l.pred, l.kind, l.lag.minutes, l.lag.unit)).collect();
                v.sort();
                v.join(",")
            };
            diff(format!("{n} links"), links(g), links(w));
        }
        diff("resource count".into(), got.resources.len().to_string(), want.resources.len().to_string());
        for w in &want.resources {
            let Some(g) = got.resources.iter().find(|r| r.uid == w.uid) else {
                diff(format!("resource {}", w.uid), "missing".into(), w.name.clone());
                continue;
            };
            let n = &w.name;
            diff(format!("{n} name"), g.name.clone(), w.name.clone());
            diff(format!("{n} kind"), format!("{:?}", g.kind), format!("{:?}", w.kind));
            diff(format!("{n} initials"), g.initials.clone(), w.initials.clone());
            diff(format!("{n} group"), g.group.clone(), w.group.clone());
            diff(format!("{n} max units"), format!("{:.2}", g.max_units), format!("{:.2}", w.max_units));
            diff(format!("{n} rate"), format!("{:.2}", g.std_rate), format!("{:.2}", w.std_rate));
            diff(format!("{n} label"), g.material_label.clone(), w.material_label.clone());
        }
        diff("assignment count".into(), got.assignments.len().to_string(), want.assignments.len().to_string());
        for w in &want.assignments {
            let Some(g) = got.assignments.iter().find(|a| a.task == w.task && a.resource == w.resource) else {
                diff(format!("assignment {}/{}", w.task, w.resource), "missing".into(), "present".into());
                continue;
            };
            let n = format!("assignment t{} r{}", w.task, w.resource);
            // units mean nothing for cost resources (the program itself isn't consistent)
            let cost_res = want.resources.iter().any(|r| r.uid == w.resource && r.kind == plancraft::model::ResourceKind::Cost);
            if !cost_res {
                diff(format!("{n} units"), format!("{:.3}", g.units), format!("{:.3}", w.units));
            }
            diff(format!("{n} work"), g.work.to_string(), w.work.to_string());
            diff(format!("{n} cost amount"), format!("{:.2}", g.cost_amount), format!("{:.2}", w.cost_amount));
        }
    }
    println!("{} of {total} values match the program's own export", total - bad);
}
