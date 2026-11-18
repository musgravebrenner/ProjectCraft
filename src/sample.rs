//! A small sample plan, built through the command registry (so it also exercises it).

use crate::commands::Registry;
use crate::model::Project;
use crate::session::Session;
use chrono::NaiveDate;
use serde_json::json;

pub fn sample_session() -> Session {
    let p = Project::new("Office Move", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
    let mut s = Session::new(p);
    let r = Registry::get();
    let steps = json!([
        {"cmd": "project.set.manager", "params": {"value": "Dana Park"}},
        {"cmd": "project.set.company", "params": {"value": "Northwind Studio"}},
        {"cmd": "resource.add", "params": {"name": "Dana Park", "stdRate": 85, "group": "Management"}},
        {"cmd": "resource.add", "params": {"name": "Lee Ortiz", "stdRate": 60, "group": "Operations"}},
        {"cmd": "resource.add", "params": {"name": "Morgan Reyes", "stdRate": 95, "group": "Management"}},
        {"cmd": "resource.add", "params": {"name": "Sam Chen", "stdRate": 70, "group": "Design"}},
        {"cmd": "resource.add", "params": {"name": "IT Crew", "stdRate": 55, "maxUnits": 200, "group": "IT"}},
        {"cmd": "resource.add", "params": {"name": "Movers", "stdRate": 45, "maxUnits": 300, "group": "Vendors"}},
        {"cmd": "resource.add", "params": {"name": "Staff", "stdRate": 35, "maxUnits": 500, "group": "Operations"}},
        {"cmd": "resource.add", "params": {"name": "Truck Rental", "type": "Cost"}},
        {"cmd": "resource.add", "params": {"name": "Boxes", "type": "Material", "stdRate": 2.5, "materialLabel": "boxes"}},
        {"cmd": "task.insert", "params": {"name": "Office Move", "duration": "1d", "level": 1}},
        {"cmd": "task.insert", "params": {"name": "Planning", "duration": "1d", "level": 2, "after": true}},
        {"cmd": "task.insert", "params": {"name": "Define requirements", "duration": "3d", "level": 3, "after": true, "resources": "Dana Park"}},
        {"cmd": "task.insert", "params": {"name": "Shortlist and select site", "duration": "5d", "level": 3, "after": true, "predecessors": "3", "resources": "Dana Park[50%],Lee Ortiz"}},
        {"cmd": "task.insert", "params": {"name": "Sign lease", "duration": "1d", "level": 3, "after": true, "predecessors": "4", "resources": "Morgan Reyes"}},
        {"cmd": "task.insertMilestone", "params": {"name": "Planning complete", "at": 6, "predecessors": "5"}},
        {"cmd": "task.insert", "params": {"name": "Preparation", "duration": "1d", "level": 2, "at": 7}},
        {"cmd": "task.insert", "params": {"name": "Design floor plan", "duration": "4d", "level": 3, "at": 8, "predecessors": "6", "resources": "Sam Chen"}},
        {"cmd": "task.insert", "params": {"name": "Order furniture", "duration": "2d", "level": 3, "at": 9, "predecessors": "8", "resources": "Lee Ortiz"}},
        {"cmd": "task.insert", "params": {"name": "Install network and phones", "duration": "6d", "level": 3, "at": 10, "predecessors": "5", "resources": "IT Crew[200%]"}},
        {"cmd": "task.insert", "params": {"name": "Furniture delivery", "duration": "3d", "level": 3, "at": 11, "predecessors": "9FS+5d", "resources": "Movers"}},
        {"cmd": "task.insert", "params": {"name": "Moving", "duration": "1d", "level": 2, "at": 12}},
        {"cmd": "task.insert", "params": {"name": "Pack offices", "duration": "3d", "level": 3, "at": 13, "predecessors": "11SS", "resources": "Staff[300%],Boxes[120 boxes]"}},
        {"cmd": "task.insert", "params": {"name": "Move day", "duration": "1d", "level": 3, "at": 14, "predecessors": "13,10", "resources": "Movers[300%],Truck Rental[$1200]"}},
        {"cmd": "task.insert", "params": {"name": "Unpack and set up desks", "duration": "2d", "level": 3, "at": 15, "predecessors": "14", "resources": "Staff[400%]"}},
        {"cmd": "task.insertMilestone", "params": {"name": "New office open", "at": 16, "predecessors": "15"}},
        {"cmd": "task.set.deadline", "params": {"id": 16, "value": "2026-12-18"}},
        {"cmd": "task.set.notes", "params": {"id": 4, "value": "Visit at least three sites; check transit and parking."}},
        {"cmd": "project.baseline.set.0", "params": {}},
        {"cmd": "task.markComplete.100", "params": {"ids": [3]}},
        {"cmd": "task.set.percentComplete", "params": {"ids": [4], "value": 60}},
        {"cmd": "project.set.statusDate", "params": {"value": "2026-11-12 17:00"}},
        {"cmd": "edit.select", "params": {"ids": [4]}}
    ]);
    for st in steps.as_array().unwrap() {
        let cmd = st["cmd"].as_str().unwrap();
        if let Err(e) = r.run(&mut s, cmd, &st["params"]) {
            eprintln!("sample step {cmd} failed: {e}");
        }
    }
    s.undo.clear();
    s.redo.clear();
    s.dirty = false;
    s.log.clear();
    s
}
