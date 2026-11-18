//! Built-in project templates for File > New. Each is built through the command registry.

use crate::commands::Registry;
use crate::model::Project;
use crate::session::Session;
use chrono::{Datelike, Duration, NaiveDate};
use serde_json::json;

pub struct Template {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// (name, outline level, duration, predecessors, resources)
    pub tasks: &'static [(&'static str, u8, &'static str, &'static str, &'static str)],
}

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "softwareRelease",
        title: "Software Development Plan",
        description: "Requirements to release, with design, build, test and launch phases.",
        tasks: &[
            ("Software release", 1, "1d", "", ""),
            ("Requirements", 2, "1d", "", ""),
            ("Gather requirements", 3, "5d", "", "Product Owner"),
            ("Write specification", 3, "4d", "3", "Product Owner,Analyst"),
            ("Specification approved", 3, "0d", "4", ""),
            ("Design", 2, "1d", "", ""),
            ("Architecture", 3, "5d", "5", "Lead Developer"),
            ("User interface design", 3, "6d", "5", "Designer"),
            ("Build", 2, "1d", "", ""),
            ("Backend development", 3, "15d", "7", "Developer A,Lead Developer[50%]"),
            ("Frontend development", 3, "12d", "8", "Developer B"),
            ("Integration", 3, "4d", "10,11", "Developer A,Developer B"),
            ("Test", 2, "1d", "", ""),
            ("System testing", 3, "8d", "12", "Tester"),
            ("Fix defects", 3, "5d", "14SS+3d", "Developer A,Developer B"),
            ("User acceptance", 3, "4d", "15", "Product Owner"),
            ("Launch", 2, "1d", "", ""),
            ("Deploy to production", 3, "1d", "16", "Lead Developer"),
            ("Release", 3, "0d", "18", ""),
        ],
    },
    Template {
        id: "construction",
        title: "Commercial Construction",
        description: "Site work, structure, enclosure, interiors and handover.",
        tasks: &[
            ("Building project", 1, "1d", "", ""),
            ("Pre-construction", 2, "1d", "", ""),
            ("Permits", 3, "15d", "", "Project Manager"),
            ("Site survey", 3, "3d", "", "Surveyor"),
            ("Site work", 2, "1d", "", ""),
            ("Clear and grade site", 3, "5d", "3,4", "Site Crew"),
            ("Foundations", 3, "10d", "6", "Concrete Crew"),
            ("Structure", 2, "1d", "", ""),
            ("Steel frame", 3, "15d", "7", "Steel Crew"),
            ("Floors and roof deck", 3, "10d", "9SS+5d", "Concrete Crew"),
            ("Enclosure", 2, "1d", "", ""),
            ("Exterior walls", 3, "12d", "10", "Masonry Crew"),
            ("Roofing", 3, "6d", "10", "Roofers"),
            ("Interiors", 2, "1d", "", ""),
            ("Mechanical and electrical", 3, "20d", "12SS+5d", "MEP Crew"),
            ("Drywall and finishes", 3, "15d", "15SS+10d", "Finish Crew"),
            ("Handover", 2, "1d", "", ""),
            ("Inspections", 3, "3d", "16,13", "Project Manager"),
            ("Building handed over", 3, "0d", "18", ""),
        ],
    },
    Template {
        id: "eventPlanning",
        title: "Event Planning",
        description: "Venue, speakers, marketing and the event itself.",
        tasks: &[
            ("Conference", 1, "1d", "", ""),
            ("Set budget and goals", 2, "3d", "", "Organizer"),
            ("Book venue", 2, "5d", "2", "Organizer"),
            ("Invite speakers", 2, "10d", "2", "Program Lead"),
            ("Open registration", 2, "1d", "3", "Marketing"),
            ("Promote the event", 2, "20d", "5", "Marketing"),
            ("Arrange catering", 2, "4d", "3", "Organizer"),
            ("Print materials", 2, "3d", "4", "Designer"),
            ("Event day", 2, "1d", "6,7,8", "Organizer,Program Lead,Marketing"),
            ("Send thank-you notes", 2, "2d", "9", "Organizer"),
        ],
    },
    Template {
        id: "marketingCampaign",
        title: "Marketing Campaign",
        description: "Research, creative, channels, launch and review.",
        tasks: &[
            ("Campaign", 1, "1d", "", ""),
            ("Market research", 2, "5d", "", "Analyst"),
            ("Campaign brief", 2, "2d", "2", "Marketing Lead"),
            ("Creative concepts", 2, "6d", "3", "Designer,Copywriter"),
            ("Concepts approved", 2, "0d", "4", ""),
            ("Produce ads", 2, "8d", "5", "Designer"),
            ("Set up channels", 2, "4d", "5", "Marketing Lead"),
            ("Launch", 2, "0d", "6,7", ""),
            ("Run campaign", 2, "20d", "8", "Marketing Lead[50%]"),
            ("Results review", 2, "2d", "9", "Analyst,Marketing Lead"),
        ],
    },
    Template {
        id: "officeMove",
        title: "Office Move",
        description: "The sample plan: planning, preparation and moving day.",
        tasks: &[],
    },
];

pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id.eq_ignore_ascii_case(id) || t.title.eq_ignore_ascii_case(id))
}

/// Next Monday on or after a date.
pub fn next_monday(d: NaiveDate) -> NaiveDate {
    let mut d = d;
    while d.weekday().num_days_from_monday() != 0 {
        d += Duration::days(1);
    }
    d
}

/// A fresh session holding the template's plan, starting on `start`.
pub fn build(t: &Template, start: NaiveDate) -> Session {
    if t.id == "officeMove" {
        return crate::sample::sample_session();
    }
    let mut s = Session::new(Project::new(t.title, start));
    let r = Registry::get();
    let was = s.agent;
    s.agent = true;
    for (k, (name, level, dur, preds, res)) in t.tasks.iter().enumerate() {
        let mut p = json!({"name": name, "duration": dur, "at": k + 1, "level": level});
        let _ = r.run(&mut s, "task.insert", &p);
        if !preds.is_empty() {
            p = json!({"id": k + 1, "value": preds});
            let _ = r.run(&mut s, "task.set.predecessors", &p);
        }
        if !res.is_empty() {
            let _ = r.run(&mut s, "task.set.resourceNames", &json!({"id": k + 1, "value": res}));
        }
    }
    s.agent = was;
    s.undo.clear();
    s.redo.clear();
    s.dirty = false;
    s.log.clear();
    s.view.selected.clear();
    s
}
