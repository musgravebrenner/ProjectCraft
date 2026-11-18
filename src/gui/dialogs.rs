//! Dialogs. Each one collects input and, on OK, runs commands from the registry.

use super::{theme, App};
use crate::calendar::Calendar;
use crate::commands::WEEKDAYS;
use crate::fields::{self, TASK_FIELDS};
use crate::model::*;
use crate::session::Dialog;
use egui::{Color32, Context, RichText, Ui};
use serde_json::{json, Value};

#[derive(Clone, PartialEq)]
pub struct TaskInfoDlg {
    pub uid: u32,
    pub tab: u8,
    pub name: String,
    pub duration: String,
    pub start: String,
    pub finish: String,
    pub pct: String,
    pub priority: String,
    pub manual: bool,
    pub milestone: bool,
    pub active: bool,
    pub task_type: TaskType,
    pub effort: bool,
    pub constraint: ConstraintKind,
    pub cdate: String,
    pub deadline: String,
    pub calendar: String,
    pub notes: String,
    pub hyperlink: String,
    pub estimated: bool,
    pub on_timeline: bool,
    pub hide_bar: bool,
    pub text: [String; 5],
    pub number: [String; 5],
    pub flag: [bool; 5],
    pub preds: Vec<(String, LinkKind, String)>,
    pub res: Vec<(String, String)>,
}

#[derive(Clone, PartialEq)]
pub struct ResInfoDlg {
    pub uid: u32,
    pub name: String,
    pub initials: String,
    pub kind: ResourceKind,
    pub group: String,
    pub max: String,
    pub std: String,
    pub ovt: String,
    pub cpu: String,
    pub email: String,
    pub calendar: String,
    pub notes: String,
    pub label: String,
}

#[derive(Clone, PartialEq)]
pub struct ProjInfoDlg {
    pub title: String,
    pub subject: String,
    pub manager: String,
    pub company: String,
    pub start: String,
    pub status: String,
    pub calendar: String,
    pub notes: String,
}

#[derive(Clone, PartialEq)]
pub struct WorkTimeDlg {
    pub calendar: String,
    pub week: [String; 7],
    pub orig_week: [String; 7],
    pub exc_name: String,
    pub exc_start: String,
    pub exc_end: String,
    pub exc_hours: String,
    pub new_name: String,
}

#[derive(Clone, PartialEq)]
pub struct OptionsDlg {
    pub hpd: String,
    pub hpw: String,
    pub dpm: String,
    pub start: String,
    pub finish: String,
    pub crit: String,
    pub honor: bool,
    pub manual: bool,
    pub effort: bool,
    pub ttype: TaskType,
    pub currency: String,
    pub auto: bool,
    pub dur_unit: DurUnit,
    pub work_unit: DurUnit,
}

#[derive(Clone, Default)]
pub struct FindDlg {
    pub text: String,
    pub field: String,
    pub replace: String,
}

#[derive(Default)]
pub struct Dialogs {
    pub palette: bool,
    pub task_info: Option<(TaskInfoDlg, TaskInfoDlg)>,
    pub res_info: Option<(ResInfoDlg, ResInfoDlg)>,
    pub project_info: Option<(ProjInfoDlg, ProjInfoDlg)>,
    pub working_time: Option<WorkTimeDlg>,
    pub options: Option<(OptionsDlg, OptionsDlg)>,
    pub find: Option<FindDlg>,
    pub reports: Vec<(String, Value)>,
    pub about: bool,
    pub agent: bool,
    pub stats: bool,
    pub assign: Option<String>,
    pub columns: bool,
    pub save_view: Option<String>,
    pub more_views: bool,
    pub info: Option<Dialog>,
    pub text_styles: Option<super::more_dialogs::TextStylesDlg>,
    pub inspector: bool,
    pub update_tasks: Option<(super::more_dialogs::UpdateDlg, super::more_dialogs::UpdateDlg)>,
    pub set_baseline: Option<super::more_dialogs::BaselineDlg>,
    pub bar_styles: Option<crate::session::BarColors>,
    pub field_settings: Option<(String, String)>,
    pub confirm: Option<(String, Value)>,
    pub message: Option<(String, String)>,
}

impl Dialogs {
    pub fn open_confirm(&mut self, id: &str, params: Value) {
        self.confirm = Some((id.to_string(), params));
    }
}

fn date_text(d: Option<chrono::NaiveDateTime>) -> String {
    d.map(fmt_datetime).unwrap_or_default()
}

pub fn open(app: &mut App, d: Dialog) {
    let p = &app.s.project;
    match d {
        Dialog::TaskInfo(uid) => {
            if let Some(i) = p.task_index(uid) {
                let t = &p.tasks[i];
                let dlg = TaskInfoDlg {
                    uid,
                    tab: 0,
                    name: t.name.clone(),
                    duration: t.duration.format(&p.settings),
                    start: fmt_datetime(t.start),
                    finish: fmt_datetime(t.finish),
                    pct: fmt_num(t.percent_complete),
                    priority: t.priority.to_string(),
                    manual: t.manual,
                    milestone: t.milestone,
                    active: t.active,
                    task_type: t.task_type,
                    effort: t.effort_driven,
                    constraint: t.constraint,
                    cdate: date_text(t.constraint_date),
                    deadline: date_text(t.deadline),
                    calendar: t.calendar.clone().unwrap_or_default(),
                    notes: t.notes.clone(),
                    estimated: t.duration.estimated,
                    on_timeline: t.on_timeline,
                    hide_bar: t.hide_bar,
                    text: t.text.clone(),
                    number: t.number.map(fmt_num),
                    flag: t.flag,
                    hyperlink: t.hyperlink.clone(),
                    preds: t
                        .predecessors
                        .iter()
                        .filter_map(|l| p.task_index(l.pred).map(|pi| ((pi + 1).to_string(), l.kind, if l.lag.minutes == 0 { String::new() } else { l.lag.format(&p.settings) })))
                        .collect(),
                    res: p
                        .assignments_of_task(uid)
                        .filter_map(|a| {
                            let r = p.resource_by_uid(a.resource)?;
                            let units = match r.kind {
                                ResourceKind::Work => format!("{}%", fmt_num(a.units * 100.0)),
                                ResourceKind::Material => fmt_num(a.units),
                                ResourceKind::Cost => format!("{}{}", p.settings.currency, fmt_num(a.cost_amount)),
                            };
                            Some((r.name.clone(), units))
                        })
                        .collect(),
                };
                app.dialogs.task_info = Some((dlg.clone(), dlg));
            }
        }
        Dialog::ResourceInfo(uid) => {
            if let Some(r) = p.resource_by_uid(uid) {
                let dlg = ResInfoDlg {
                    uid,
                    name: r.name.clone(),
                    initials: r.initials.clone(),
                    kind: r.kind,
                    group: r.group.clone(),
                    max: format!("{}%", fmt_num(r.max_units * 100.0)),
                    std: fmt_num(r.std_rate),
                    ovt: fmt_num(r.ovt_rate),
                    cpu: fmt_num(r.cost_per_use),
                    email: r.email.clone(),
                    calendar: r.calendar.clone().unwrap_or_default(),
                    notes: r.notes.clone(),
                    label: r.material_label.clone(),
                };
                app.dialogs.res_info = Some((dlg.clone(), dlg));
            }
        }
        Dialog::ProjectInfo => {
            let dlg = ProjInfoDlg {
                title: p.title.clone(),
                subject: p.subject.clone(),
                manager: p.manager.clone(),
                company: p.company.clone(),
                start: fmt_datetime(p.start),
                status: date_text(p.status_date),
                calendar: p.calendar.clone(),
                notes: p.notes.clone(),
            };
            app.dialogs.project_info = Some((dlg.clone(), dlg));
        }
        Dialog::ChangeWorkingTime(name) => app.dialogs.working_time = Some(work_time_state(app, &name)),
        // the scheduling options are the Schedule tab of File > Options
        Dialog::Options => super::options::open(app, "schedule"),
        Dialog::AppOptions(tab) => super::options::open(app, &tab),
        Dialog::TextStyles | Dialog::Inspector | Dialog::UpdateTasks | Dialog::SetBaseline => super::more_dialogs::open(app, &d),
        Dialog::CommandPalette => {
            app.dialogs.palette = true;
            app.palette.focus = false;
        }
        Dialog::FindReplace => app.dialogs.find = Some(FindDlg { field: "name".into(), ..Default::default() }),
        Dialog::Report(title, v) => {
            app.dialogs.reports.retain(|(t, _)| *t != title);
            app.dialogs.reports.push((title, v));
        }
        Dialog::About => app.dialogs.about = true,
        Dialog::AgentAccess => app.dialogs.agent = true,
        Dialog::Statistics => app.dialogs.stats = true,
        Dialog::AssignResources => app.dialogs.assign = Some("100%".into()),
        Dialog::ColumnPicker => app.dialogs.columns = true,
        Dialog::SaveView => app.dialogs.save_view = Some(format!("{} (custom)", app.s.view.view.title())),
        Dialog::MoreViews => app.dialogs.more_views = true,
        d @ (Dialog::LevelingOptions | Dialog::CustomFields | Dialog::Help | Dialog::GettingStarted | Dialog::Feedback | Dialog::Layout) => app.dialogs.info = Some(d),
        Dialog::BarStyles => app.dialogs.bar_styles = Some(app.s.view.bar_colors.clone()),
        Dialog::FieldSettings(f) => {
            let t = app.s.view.col_titles.get(&f).cloned().unwrap_or_else(|| fields::task_field(&f).map(|d| d.title.to_string()).unwrap_or_default());
            app.dialogs.field_settings = Some((f, t));
        }
        Dialog::ConfirmDiscard(cmd) => app.dialogs.confirm = Some((cmd, json!({}))),
        Dialog::Message(t, m) => app.dialogs.message = Some((t, m)),
        Dialog::Open => {
            if let Some(path) = rfd::FileDialog::new().add_filter("Project plans", &["json", "xml", "mpp"]).pick_file() {
                app.run("file.open", json!({"path": path.to_string_lossy()}));
            }
        }
        Dialog::SaveAs => {
            let name = format!("{}.plan.json", app.s.project.title.replace(['/', '\\', ':'], "-"));
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PlanCraft plan", &["json"])
                .add_filter("XML interchange", &["xml"])
                .add_filter("HTML report", &["html"])
                .add_filter("Gantt picture (SVG)", &["svg"])
                .add_filter("Task table (CSV)", &["csv"])
                .set_file_name(&name)
                .save_file()
            {
                app.run("file.saveAs", json!({"path": path.to_string_lossy()}));
            }
        }
        Dialog::Export(fmt) => {
            let ext = match fmt.as_str() {
                "markdown" => "md",
                "resourcesCsv" => "csv",
                f => f,
            };
            let ext = ext.to_string();
            if let Some(path) = rfd::FileDialog::new().add_filter(&ext.to_uppercase(), &[ext.as_str()]).set_file_name(format!("{}.{ext}", app.s.project.title)).save_file() {
                app.run(&format!("file.export.{fmt}"), json!({"path": path.to_string_lossy()}));
            }
        }
    }
}

fn work_time_state(app: &App, name: &str) -> WorkTimeDlg {
    let p = &app.s.project;
    let cal = p.calendar_named(name).unwrap_or(p.cal());
    let week: [String; 7] = std::array::from_fn(|d| Calendar::shifts_text(&cal.week[d]));
    WorkTimeDlg {
        calendar: cal.name.clone(),
        week: week.clone(),
        orig_week: week,
        exc_name: "Holiday".into(),
        exc_start: String::new(),
        exc_end: String::new(),
        exc_hours: String::new(),
        new_name: String::new(),
    }
}

/// A JSON template of a command's parameters, with the selection filled in.
pub fn params_template(app: &App, id: &str) -> String {
    let Some(c) = app.reg.find(id) else { return "{}".into() };
    let mut m = serde_json::Map::new();
    for p in &c.params {
        let v = match (p.name.as_str(), p.kind.as_str()) {
            ("ids", _) => {
                let ids: Vec<usize> = app.s.view.selected.iter().filter_map(|u| app.s.project.task_index(*u)).map(|i| i + 1).collect();
                if ids.is_empty() && !p.required {
                    continue;
                }
                json!(ids)
            }
            ("id", _) => match app.s.current_task() {
                Some(i) => json!(i + 1),
                None => json!(1),
            },
            (_, "number") | (_, "integer") => json!(0),
            (_, "boolean") => json!(false),
            (_, "object[]") => json!([]),
            (_, "string[]") | (_, "integer[]") => json!([]),
            _ => json!(""),
        };
        if p.required || p.name == "id" {
            m.insert(p.name.clone(), v);
        }
    }
    serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_else(|_| "{}".into())
}

fn row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(220.0));
    ui.end_row();
}

pub fn show(app: &mut App, ctx: &Context) {
    task_info(app, ctx);
    res_info(app, ctx);
    project_info(app, ctx);
    working_time(app, ctx);
    options(app, ctx);
    palette(app, ctx);
    find(app, ctx);
    reports(app, ctx);
    small_windows(app, ctx);
    view_windows(app, ctx);
    super::more_dialogs::show(app, ctx);
    info_windows(app, ctx);
    format_windows(app, ctx);
}

fn format_windows(app: &mut App, ctx: &Context) {
    if let Some(mut bc) = app.dialogs.bar_styles.take() {
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        dlg(ctx, "Bar Styles").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Colour of each kind of bar:");
            egui::Grid::new("barstyles").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                for (label, c) in [
                    ("Task", &mut bc.task),
                    ("Critical task", &mut bc.critical),
                    ("Manually scheduled task", &mut bc.manual),
                    ("Summary", &mut bc.summary),
                    ("Milestone", &mut bc.milestone),
                    ("Progress", &mut bc.progress),
                    ("Baseline", &mut bc.baseline),
                ] {
                    ui.label(label);
                    ui.color_edit_button_srgb(c);
                    ui.end_row();
                }
            });
            let (o, c) = buttons(ui);
            ok = o;
            cancel = c;
        });
        if ok {
            let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
            let steps: Vec<Value> = [("task", bc.task), ("critical", bc.critical), ("manual", bc.manual), ("summary", bc.summary), ("milestone", bc.milestone), ("progress", bc.progress), ("baseline", bc.baseline)]
                .iter()
                .map(|(p, c)| json!({"cmd": format!("format.barColor.{p}"), "params": {"color": hex(*c)}}))
                .collect();
            app.run("app.batch", json!({"steps": steps}));
        } else if open && !cancel {
            app.dialogs.bar_styles = Some(bc);
        }
    }
    if let Some((field, mut title)) = app.dialogs.field_settings.take() {
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        let std_title = fields::task_field(&field).map(|f| f.title).unwrap_or("");
        dlg(ctx, "Field Settings").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(format!("Field: {std_title}"));
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.add(egui::TextEdit::singleline(&mut title).desired_width(200.0));
            });
            ui.horizontal(|ui| {
                ui.label("Align text:");
                for (al, label) in [("left", "Left"), ("center", "Center"), ("right", "Right")] {
                    let cur = app.s.view.col_align.get(&field).map(|s| s.as_str()).unwrap_or("left");
                    if ui.radio(cur == al, label).clicked() {
                        app.s.view.col_align.insert(field.clone(), al.to_string());
                    }
                }
            });
            if ui.button("Best Fit").clicked() {
                app.run("view.column.bestFit", json!({"field": field}));
            }
            let (o, c) = buttons(ui);
            ok = o;
            cancel = c;
        });
        if ok {
            let t = if title.trim() == std_title { String::new() } else { title.clone() };
            app.run("view.column.title", json!({"field": field, "title": t}));
        } else if open && !cancel {
            app.dialogs.field_settings = Some((field, title));
        }
    }
}

fn info_windows(app: &mut App, ctx: &Context) {
    let Some(d) = app.dialogs.info.clone() else { return };
    let th = theme(app.s.view.dark);
    let mut open = true;
    let mut runs: Vec<(String, Value)> = vec![];
    let title = match d {
        Dialog::LevelingOptions => "Resource Leveling",
        Dialog::CustomFields => "Custom Fields",
        Dialog::Help => "Help",
        Dialog::GettingStarted => "Getting Started",
        Dialog::Layout => "Layout",
        _ => "Feedback",
    };
    dlg(ctx, title).open(&mut open).collapsible(false).default_width(480.0).show(ctx, |ui| match d {
        Dialog::Layout => {
            let v = &app.s.view;
            let cur = |s: &String, d: &str| if s.is_empty() { d.to_string() } else { s.clone() };
            let (links, size, dates) = (cur(&v.link_style, "elbow"), cur(&v.bar_size, "medium"), cur(&v.bar_date_format, "short"));
            ui.label(RichText::new("Links").strong());
            ui.horizontal(|ui| {
                for (id, label) in [("elbow", "Elbow (bent) lines"), ("straight", "Straight lines"), ("none", "No lines")] {
                    if ui.radio(links == id, label).clicked() {
                        runs.push((format!("view.layout.links.{id}"), json!({})));
                    }
                }
            });
            ui.label(RichText::new("Bar height").strong());
            ui.horizontal(|ui| {
                for (id, label) in [("small", "Small"), ("medium", "Medium"), ("large", "Large")] {
                    if ui.radio(size == id, label).clicked() {
                        runs.push((format!("view.layout.barSize.{id}"), json!({})));
                    }
                }
            });
            ui.label(RichText::new("Dates beside milestones").strong());
            ui.horizontal(|ui| {
                for (id, label) in [("short", "11/12"), ("long", "Thu 11/12/26"), ("none", "None")] {
                    if ui.radio(dates == id, label).clicked() {
                        runs.push((format!("view.layout.dateFormat.{id}"), json!({})));
                    }
                }
            });
            ui.label(RichText::new("Other").strong());
            for (flag, label) in [("barText", "Show resource names beside bars"), ("progress", "Show progress inside bars"), ("nonworkingTime", "Shade nonworking days"), ("todayLine", "Show today and status date lines")] {
                let mut on = crate::commands::view_flag(v, flag);
                if ui.checkbox(&mut on, label).clicked() {
                    runs.push((format!("view.toggle.{flag}"), json!({"on": on})));
                }
            }
        }
        Dialog::LevelingOptions => {
            ui.label(RichText::new("Leveling order").strong());
            ui.label("Tasks are looked at by start date; when two overlap on a resource beyond its Max. Units, the one with the lower priority (then the later one) is delayed to the finish of the task it clashes with.");
            ui.label(RichText::new("Not moved").strong());
            ui.label("Manually scheduled tasks, tasks already started and tasks with Must Start On / Must Finish On constraints stay where they are. Priority 1000 (Do Not Level) tasks are never delayed in favour of others.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (label, cmd) in [("Level All", "resource.level"), ("Level Selection", "resource.levelSelection"), ("Level Resource", "resource.levelResource"), ("Clear Leveling", "resource.clearLeveling")] {
                    if ui.button(label).clicked() {
                        runs.push((cmd.to_string(), json!({})));
                    }
                }
            });
            let over: Vec<String> = app.s.project.resources.iter().filter(|r| r.overallocated).map(|r| r.name.clone()).collect();
            ui.label(RichText::new(if over.is_empty() { "No resource is overallocated.".to_string() } else { format!("Overallocated now: {}", over.join(", ")) }).color(if over.is_empty() { th.dim } else { th.danger }));
        }
        Dialog::CustomFields => {
            ui.label("Every task has five Text, five Number and five Flag fields for your own data. Tick one to show it as a column; type in its cells like any other column.");
            egui::Grid::new("cf").num_columns(3).spacing([30.0, 4.0]).show(ui, |ui| {
                for k in 1..=5 {
                    for kind in ["text", "number", "flag"] {
                        let id = format!("{kind}{k}");
                        let mut on = app.s.view.columns.iter().any(|c| *c == id);
                        let title = fields::task_field(&id).map(|f| f.title).unwrap_or("");
                        if ui.checkbox(&mut on, title).clicked() {
                            runs.push((format!("view.column.{}.{id}", if on { "show" } else { "hide" }), json!({})));
                        }
                    }
                    ui.end_row();
                }
            });
        }
        Dialog::Help => {
            ui.label(RichText::new("Views").strong());
            ui.label("Pick a view from the big view button on the Task or View tab: Gantt Chart, Tracking Gantt, Task Usage, Network Diagram, Calendar, Task Sheet, Task Form, Team Planner, Resource Sheet, Resource Usage, Resource Graph, Resource Form and Timeline.");
            ui.label(RichText::new("Scheduling").strong());
            ui.label("Durations like 3d, 4h, 2w (add ? for an estimate). Predecessors like 2, 3SS, 4FS+2d. Typing a start date adds a Start No Earlier Than constraint.");
            ui.label(RichText::new("Keyboard").strong());
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                egui::Grid::new("keys").striped(true).show(ui, |ui| {
                    for c in app.reg.commands.iter().filter(|c| c.shortcut.is_some()) {
                        ui.label(RichText::new(c.shortcut.clone().unwrap_or_default()).monospace());
                        ui.label(&c.title);
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("Command Palette").clicked() {
                    runs.push(("app.commandPalette".into(), json!({})));
                }
                if ui.button("Scripts and Agents").clicked() {
                    runs.push(("help.agentAccess".into(), json!({})));
                }
            });
        }
        Dialog::GettingStarted => {
            for (k, step) in crate::commands::GETTING_STARTED.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{}.", k + 1)).strong().color(th.accent));
                    ui.label(*step);
                });
            }
            ui.add_space(4.0);
            if ui.button("Start a new empty plan").clicked() {
                runs.push(("file.new".into(), json!({})));
            }
        }
        _ => {
            ui.label("Found a problem or have an idea? Copy these details into your message so it can be reproduced:");
            let details = format!(
                "PlanCraft {}\n{} commands\nView: {}\nTasks: {}, resources: {}\nLast commands:\n{}",
                env!("CARGO_PKG_VERSION"),
                app.reg.len(),
                app.s.view.view.title(),
                app.s.project.tasks.len(),
                app.s.project.resources.len(),
                app.s.log.iter().rev().take(10).cloned().collect::<Vec<_>>().join("\n")
            );
            ui.label(RichText::new(&details).monospace().small());
            if ui.button("Copy details").clicked() {
                ui.ctx().copy_text(details);
            }
        }
    });
    for (c, p) in runs {
        app.run(&c, p);
    }
    if !open {
        app.dialogs.info = None;
    }
}

fn view_windows(app: &mut App, ctx: &Context) {
    if let Some(mut name) = app.dialogs.save_view.take() {
        let mut open = true;
        let (mut ok, mut cancel) = (false, false);
        dlg(ctx, "Save View").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Save the current view (table, filter, group, sort and zoom) as:");
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(260.0));
            let (o, c) = buttons(ui);
            ok = o;
            cancel = c;
        });
        if ok {
            app.run("view.save", json!({"name": name}));
        } else if open && !cancel {
            app.dialogs.save_view = Some(name);
        }
    }
    if app.dialogs.more_views {
        let mut open = true;
        let mut runs: Vec<(String, Value)> = vec![];
        let th = theme(app.s.view.dark);
        dlg(ctx, "More Views").open(&mut open).collapsible(false).default_width(320.0).show(ctx, |ui| {
            ui.label(RichText::new("Views:").strong());
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for vk in crate::session::ViewKind::menu_order() {
                    let sel = app.s.view.view == vk;
                    ui.horizontal(|ui| {
                        if let Some(src) = super::icons::for_command(&format!("view.show.{}", vk.id())) {
                            ui.add(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)));
                        }
                        if ui.selectable_label(sel, vk.title()).double_clicked() {
                            runs.push((format!("view.show.{}", vk.id()), json!({})));
                        }
                        if ui.small_button("Apply").clicked() {
                            runs.push((format!("view.show.{}", vk.id()), json!({})));
                        }
                    });
                }
                if !app.s.view.saved_views.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Saved views:").strong());
                    for sv in &app.s.view.saved_views {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}  ({})", sv.name, sv.view.title()));
                            if ui.small_button("Apply").clicked() {
                                runs.push(("view.apply".into(), json!({"name": sv.name})));
                            }
                            if ui.small_button("Delete").clicked() {
                                runs.push(("view.deleteSaved".into(), json!({"name": sv.name})));
                            }
                        });
                    }
                }
            });
            ui.label(RichText::new("Saved views are kept in the project file.").small().color(th.dim));
        });
        for (c, p) in runs {
            app.run(&c, p);
        }
        app.dialogs.more_views = open;
    }
}

/// Every pop-up window: centred when it first opens, rounded with a soft shadow, roomy margins,
/// and no collapse arrow.
pub fn dlg<'a>(ctx: &Context, title: impl Into<egui::WidgetText>) -> egui::Window<'a> {
    let dark = ctx.style().visuals.dark_mode;
    let frame = egui::Frame::window(&ctx.style())
        .inner_margin(egui::Margin::symmetric(18, 14))
        .corner_radius(egui::CornerRadius::same(10))
        .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 55 }) })
        .stroke(egui::Stroke::new(1.0_f32, if dark { Color32::from_gray(70) } else { Color32::from_gray(205) }));
    egui::Window::new(title).collapsible(false).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.screen_rect().center()).frame(frame)
}

/// The primary button of a dialog: filled with the accent colour.
pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let th = theme(ui.visuals().dark_mode);
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(th.accent).min_size(egui::vec2(88.0, 28.0)).corner_radius(egui::CornerRadius::same(4)))
}

pub fn secondary_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(egui::vec2(88.0, 28.0)).corner_radius(egui::CornerRadius::same(4)))
}

fn buttons(ui: &mut Ui) -> (bool, bool) {
    let mut ok = false;
    let mut cancel = false;
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            cancel = secondary_button(ui, "Cancel").clicked();
            ok = primary_button(ui, "OK").clicked();
        });
    });
    // Enter accepts and Escape cancels, as in other office dialogs
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    (ok, cancel)
}

fn task_info(app: &mut App, ctx: &Context) {
    let Some((mut d, orig)) = app.dialogs.task_info.take() else { return };
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    let th = theme(app.s.view.dark);
    let id = app.s.project.task_index(d.uid).map(|i| i + 1);
    let Some(id) = id else { return };
    let is_summary = app.s.project.tasks[id - 1].summary;
    let res_names: Vec<String> = app.s.project.resources.iter().map(|r| r.name.clone()).collect();
    let cal_names: Vec<String> = app.s.project.calendars.iter().map(|c| c.name.clone()).collect();
    dlg(ctx, "Task Information").open(&mut open).collapsible(false).resizable(true).default_width(560.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(300.0));
            ui.label("Duration:");
            ui.add_enabled(!is_summary, egui::TextEdit::singleline(&mut d.duration).desired_width(80.0));
            ui.checkbox(&mut d.estimated, "Estimated");
        });
        ui.horizontal(|ui| {
            for (k, t) in ["General", "Predecessors", "Resources", "Advanced", "Notes", "Custom Fields"].iter().enumerate() {
                ui.selectable_value(&mut d.tab, k as u8, *t);
            }
        });
        ui.separator();
        match d.tab {
            0 => {
                egui::Grid::new("ti_gen").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Percent complete:");
                    ui.add(egui::TextEdit::singleline(&mut d.pct).desired_width(60.0));
                    ui.label("Priority:");
                    ui.add(egui::TextEdit::singleline(&mut d.priority).desired_width(60.0));
                    ui.end_row();
                    ui.label("Start:");
                    ui.add_enabled(!is_summary, egui::TextEdit::singleline(&mut d.start).desired_width(140.0));
                    ui.label("Finish:");
                    ui.add_enabled(!is_summary, egui::TextEdit::singleline(&mut d.finish).desired_width(140.0));
                    ui.end_row();
                    ui.label("Schedule mode:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut d.manual, true, "Manually");
                        ui.radio_value(&mut d.manual, false, "Auto");
                    });
                    ui.checkbox(&mut d.active, "Active");
                    ui.end_row();
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut d.on_timeline, "Display on Timeline");
                    ui.add_space(12.0);
                    ui.checkbox(&mut d.hide_bar, "Hide Bar");
                });
                ui.label(RichText::new("Typing a start or finish on an auto-scheduled task adds a Start/Finish No Earlier Than constraint.").small().color(th.dim));
            }
            1 => {
                egui::Grid::new("ti_pred").num_columns(4).striped(true).show(ui, |ui| {
                    ui.label(RichText::new("ID").strong());
                    ui.label(RichText::new("Task Name").strong());
                    ui.label(RichText::new("Type").strong());
                    ui.label(RichText::new("Lag").strong());
                    ui.end_row();
                    let mut remove = None;
                    for (k, (pid, kind, lag)) in d.preds.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(pid).desired_width(40.0));
                        let name = pid.trim().parse::<usize>().ok().and_then(|n| app.s.project.tasks.get(n.wrapping_sub(1))).map(|t| t.name.clone()).unwrap_or_default();
                        ui.label(name);
                        egui::ComboBox::from_id_salt(("lk", k)).selected_text(kind.title()).show_ui(ui, |ui| {
                            for lk in LinkKind::ALL {
                                ui.selectable_value(kind, lk, format!("{} ({})", lk.title(), lk.abbr()));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(lag).desired_width(60.0).hint_text("0d"));
                            if ui.small_button("x").clicked() {
                                remove = Some(k);
                            }
                        });
                        ui.end_row();
                    }
                    if let Some(k) = remove {
                        d.preds.remove(k);
                    }
                });
                if ui.button("+ Add predecessor").clicked() {
                    d.preds.push((String::new(), LinkKind::FS, String::new()));
                }
            }
            2 => {
                if is_summary {
                    ui.label("Resources are assigned to subtasks, not summary tasks.");
                } else {
                    egui::Grid::new("ti_res").num_columns(2).striped(true).show(ui, |ui| {
                        ui.label(RichText::new("Resource Name").strong());
                        ui.label(RichText::new("Units").strong());
                        ui.end_row();
                        let mut remove = None;
                        for (k, (name, units)) in d.res.iter_mut().enumerate() {
                            egui::ComboBox::from_id_salt(("rn", k)).selected_text(name.clone()).width(200.0).show_ui(ui, |ui| {
                                for r in &res_names {
                                    ui.selectable_value(name, r.clone(), r);
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(units).desired_width(70.0));
                                if ui.small_button("x").clicked() {
                                    remove = Some(k);
                                }
                            });
                            ui.end_row();
                        }
                        if let Some(k) = remove {
                            d.res.remove(k);
                        }
                    });
                    if ui.button("+ Add resource").clicked() {
                        d.res.push((res_names.first().cloned().unwrap_or_default(), "100%".into()));
                    }
                }
            }
            3 => {
                egui::Grid::new("ti_adv").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Deadline:");
                    ui.add(egui::TextEdit::singleline(&mut d.deadline).desired_width(140.0).hint_text("NA"));
                    ui.end_row();
                    ui.label("Constraint type:");
                    egui::ComboBox::from_id_salt("ti_ct").selected_text(d.constraint.title()).show_ui(ui, |ui| {
                        for c in ConstraintKind::ALL {
                            ui.selectable_value(&mut d.constraint, c, c.title());
                        }
                    });
                    ui.end_row();
                    ui.label("Constraint date:");
                    ui.add_enabled(d.constraint.needs_date(), egui::TextEdit::singleline(&mut d.cdate).desired_width(140.0));
                    ui.end_row();
                    ui.label("Task type:");
                    egui::ComboBox::from_id_salt("ti_tt").selected_text(d.task_type.title()).show_ui(ui, |ui| {
                        for t in TaskType::ALL {
                            ui.selectable_value(&mut d.task_type, t, t.title());
                        }
                    });
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut d.effort, "Effort driven");
                    ui.end_row();
                    ui.label("Calendar:");
                    egui::ComboBox::from_id_salt("ti_cal").selected_text(if d.calendar.is_empty() { "None".to_string() } else { d.calendar.clone() }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.calendar, String::new(), "None");
                        for c in &cal_names {
                            ui.selectable_value(&mut d.calendar, c.clone(), c);
                        }
                    });
                    ui.end_row();
                    ui.label("");
                    ui.add_enabled(!is_summary, egui::Checkbox::new(&mut d.milestone, "Mark task as milestone"));
                    ui.end_row();
                    ui.label("Hyperlink:");
                    ui.add(egui::TextEdit::singleline(&mut d.hyperlink).desired_width(260.0));
                    ui.end_row();
                });
            }
            4 => {
                ui.add(egui::TextEdit::multiline(&mut d.notes).desired_rows(8).desired_width(f32::INFINITY));
            }
            _ => {
                egui::Grid::new("ti_custom").num_columns(4).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("Custom Field Name").strong());
                    ui.label(RichText::new("Value").strong());
                    ui.label(RichText::new("Custom Field Name").strong());
                    ui.label(RichText::new("Value").strong());
                    ui.end_row();
                    for k in 0..5 {
                        ui.label(format!("Text{}", k + 1));
                        ui.add(egui::TextEdit::singleline(&mut d.text[k]).desired_width(150.0));
                        ui.label(format!("Number{}", k + 1));
                        ui.add(egui::TextEdit::singleline(&mut d.number[k]).desired_width(80.0));
                        ui.end_row();
                    }
                    for k in 0..5 {
                        ui.label(format!("Flag{}", k + 1));
                        let yes = if d.flag[k] { "Yes" } else { "No" };
                        ui.checkbox(&mut d.flag[k], yes);
                        if k % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            }
        }
        let (o, c) = buttons(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let mut steps = vec![];
        let set = |steps: &mut Vec<Value>, f: &str, v: Value| steps.push(json!({"cmd": format!("task.set.{f}"), "params": {"uid": d.uid, "value": v}}));
        if d.name != orig.name {
            set(&mut steps, "name", json!(d.name));
        }
        if d.manual != orig.manual {
            set(&mut steps, "mode", json!(if d.manual { "manual" } else { "auto" }));
        }
        if d.task_type != orig.task_type {
            set(&mut steps, "taskType", json!(d.task_type.id()));
        }
        if d.effort != orig.effort {
            set(&mut steps, "effortDriven", json!(d.effort));
        }
        if d.milestone != orig.milestone {
            set(&mut steps, "milestone", json!(d.milestone));
        }
        if d.duration != orig.duration {
            set(&mut steps, "duration", json!(d.duration));
        }
        if d.start != orig.start {
            set(&mut steps, "start", json!(d.start));
        }
        if d.finish != orig.finish {
            set(&mut steps, "finish", json!(d.finish));
        }
        if d.constraint != orig.constraint || d.cdate != orig.cdate {
            set(&mut steps, "constraintType", json!(d.constraint.abbr()));
            if d.constraint.needs_date() && !d.cdate.trim().is_empty() {
                set(&mut steps, "constraintDate", json!(d.cdate));
            }
        }
        if d.deadline != orig.deadline {
            set(&mut steps, "deadline", json!(d.deadline));
        }
        if d.calendar != orig.calendar {
            set(&mut steps, "calendar", json!(d.calendar));
        }
        if d.priority != orig.priority {
            set(&mut steps, "priority", json!(d.priority));
        }
        if d.preds != orig.preds {
            let text: Vec<String> = d
                .preds
                .iter()
                .filter(|(p, _, _)| !p.trim().is_empty())
                .map(|(p, k, lag)| {
                    let lag = lag.trim();
                    if *k == LinkKind::FS && lag.is_empty() {
                        p.trim().to_string()
                    } else if lag.is_empty() {
                        format!("{}{}", p.trim(), k.abbr())
                    } else {
                        let sign = if lag.starts_with('-') || lag.starts_with('+') { "" } else { "+" };
                        format!("{}{}{}{}", p.trim(), k.abbr(), sign, lag)
                    }
                })
                .collect();
            set(&mut steps, "predecessors", json!(text.join(",")));
        }
        if d.res != orig.res {
            let text: Vec<String> = d.res.iter().filter(|(n, _)| !n.trim().is_empty()).map(|(n, u)| if u.trim().is_empty() { n.clone() } else { format!("{n}[{}]", u.trim()) }).collect();
            set(&mut steps, "resourceNames", json!(text.join(",")));
        }
        if d.active != orig.active {
            set(&mut steps, "active", json!(d.active));
        }
        if d.estimated != orig.estimated {
            set(&mut steps, "estimated", json!(d.estimated));
        }
        if d.on_timeline != orig.on_timeline {
            set(&mut steps, "onTimeline", json!(d.on_timeline));
        }
        if d.hide_bar != orig.hide_bar {
            set(&mut steps, "hideBar", json!(d.hide_bar));
        }
        for k in 0..5 {
            if d.text[k] != orig.text[k] {
                set(&mut steps, &format!("text{}", k + 1), json!(d.text[k]));
            }
            if d.number[k] != orig.number[k] {
                set(&mut steps, &format!("number{}", k + 1), json!(d.number[k]));
            }
            if d.flag[k] != orig.flag[k] {
                set(&mut steps, &format!("flag{}", k + 1), json!(d.flag[k]));
            }
        }
        if d.notes != orig.notes {
            set(&mut steps, "notes", json!(d.notes));
        }
        if d.hyperlink != orig.hyperlink {
            set(&mut steps, "hyperlink", json!(d.hyperlink));
        }
        if d.pct != orig.pct {
            set(&mut steps, "percentComplete", json!(d.pct));
        }
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
            if app.status_err {
                app.dialogs.task_info = Some((d, orig));
            }
        }
        return;
    }
    if open && !cancel {
        app.dialogs.task_info = Some((d, orig));
    }
}

fn res_info(app: &mut App, ctx: &Context) {
    let Some((mut d, orig)) = app.dialogs.res_info.take() else { return };
    let Some(ri) = app.s.project.resource_index(d.uid) else { return };
    let cal_names: Vec<String> = app.s.project.calendars.iter().map(|c| c.name.clone()).collect();
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    dlg(ctx, "Resource Information").open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
        egui::Grid::new("ri").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            row(ui, "Resource name:", &mut d.name);
            row(ui, "Initials:", &mut d.initials);
            ui.label("Type:");
            egui::ComboBox::from_id_salt("ri_t").selected_text(d.kind.title()).show_ui(ui, |ui| {
                for k in ResourceKind::ALL {
                    ui.selectable_value(&mut d.kind, k, k.title());
                }
            });
            ui.end_row();
            if d.kind == ResourceKind::Material {
                row(ui, "Material label:", &mut d.label);
            }
            row(ui, "Group:", &mut d.group);
            row(ui, "Email:", &mut d.email);
            if d.kind == ResourceKind::Work {
                row(ui, "Max. units:", &mut d.max);
                row(ui, "Standard rate (/hr):", &mut d.std);
                row(ui, "Overtime rate (/hr):", &mut d.ovt);
            } else if d.kind == ResourceKind::Material {
                row(ui, "Rate per unit:", &mut d.std);
            }
            if d.kind != ResourceKind::Cost {
                row(ui, "Cost/use:", &mut d.cpu);
            }
            ui.label("Base calendar:");
            egui::ComboBox::from_id_salt("ri_c").selected_text(if d.calendar.is_empty() { "(project calendar)".to_string() } else { d.calendar.clone() }).show_ui(ui, |ui| {
                ui.selectable_value(&mut d.calendar, String::new(), "(project calendar)");
                for c in &cal_names {
                    ui.selectable_value(&mut d.calendar, c.clone(), c);
                }
            });
            ui.end_row();
        });
        ui.label("Notes:");
        ui.add(egui::TextEdit::multiline(&mut d.notes).desired_rows(3).desired_width(f32::INFINITY));
        let (o, c) = buttons(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let mut steps = vec![];
        let id = ri + 1;
        let mut set = |f: &str, a: &String, b: &String| {
            if a != b {
                steps.push(json!({"cmd": format!("resource.set.{f}"), "params": {"resource": id, "value": a}}));
            }
        };
        set("name", &d.name, &orig.name);
        set("initials", &d.initials, &orig.initials);
        set("type", &d.kind.title().to_string(), &orig.kind.title().to_string());
        set("materialLabel", &d.label, &orig.label);
        set("group", &d.group, &orig.group);
        set("email", &d.email, &orig.email);
        set("maxUnits", &d.max, &orig.max);
        set("stdRate", &d.std, &orig.std);
        set("ovtRate", &d.ovt, &orig.ovt);
        set("costPerUse", &d.cpu, &orig.cpu);
        set("calendar", &d.calendar, &orig.calendar);
        set("notes", &d.notes, &orig.notes);
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.res_info = Some((d, orig));
    }
}

fn project_info(app: &mut App, ctx: &Context) {
    let Some((mut d, orig)) = app.dialogs.project_info.take() else { return };
    let cal_names: Vec<String> = app.s.project.calendars.iter().map(|c| c.name.clone()).collect();
    let finish = fmt_date(app.s.project.finish);
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    dlg(ctx, format!("Project Information for '{}'", orig.title)).open(&mut open).collapsible(false).default_width(440.0).show(ctx, |ui| {
        egui::Grid::new("pi").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            row(ui, "Title:", &mut d.title);
            row(ui, "Subject:", &mut d.subject);
            row(ui, "Manager:", &mut d.manager);
            row(ui, "Company:", &mut d.company);
            row(ui, "Start date:", &mut d.start);
            ui.label("Finish date:");
            ui.label(RichText::new(&finish).strong());
            ui.end_row();
            ui.label("Schedule from:");
            ui.label("Project Start Date");
            ui.end_row();
            row(ui, "Status date:", &mut d.status);
            ui.label("Calendar:");
            egui::ComboBox::from_id_salt("pi_c").selected_text(d.calendar.clone()).show_ui(ui, |ui| {
                for c in &cal_names {
                    ui.selectable_value(&mut d.calendar, c.clone(), c);
                }
            });
            ui.end_row();
        });
        ui.label("Notes:");
        ui.add(egui::TextEdit::multiline(&mut d.notes).desired_rows(3).desired_width(f32::INFINITY));
        let (o, c) = buttons(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let mut steps = vec![];
        for (f, a, b) in [
            ("title", &d.title, &orig.title),
            ("subject", &d.subject, &orig.subject),
            ("manager", &d.manager, &orig.manager),
            ("company", &d.company, &orig.company),
            ("start", &d.start, &orig.start),
            ("statusDate", &d.status, &orig.status),
            ("calendar", &d.calendar, &orig.calendar),
            ("notes", &d.notes, &orig.notes),
        ] {
            if a != b {
                steps.push(json!({"cmd": format!("project.set.{f}"), "params": {"value": a}}));
            }
        }
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.project_info = Some((d, orig));
    }
}

fn working_time(app: &mut App, ctx: &Context) {
    let Some(mut d) = app.dialogs.working_time.take() else { return };
    let mut open = true;
    let mut close = false;
    let mut runs: Vec<(String, Value)> = vec![];
    let cal_names: Vec<String> = app.s.project.calendars.iter().map(|c| c.name.clone()).collect();
    let exceptions: Vec<(String, String, String, String)> = app
        .s
        .project
        .calendar_named(&d.calendar)
        .map(|c| c.exceptions.iter().map(|e| (e.name.clone(), e.start.to_string(), e.end.to_string(), Calendar::shifts_text(&e.shifts))).collect())
        .unwrap_or_default();
    let is_project_cal = app.s.project.calendar == d.calendar;
    let mut switch_to: Option<String> = None;
    dlg(ctx, "Change Working Time").open(&mut open).collapsible(false).default_width(560.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("For calendar:");
            egui::ComboBox::from_id_salt("wt_c").selected_text(d.calendar.clone()).show_ui(ui, |ui| {
                for c in &cal_names {
                    if ui.selectable_label(*c == d.calendar, c).clicked() {
                        switch_to = Some(c.clone());
                    }
                }
            });
            if is_project_cal {
                ui.label(RichText::new("(Project Calendar)").italics());
            } else if ui.button("Use for project").clicked() {
                runs.push(("project.set.calendar".into(), json!({"value": d.calendar})));
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.new_name).hint_text("new calendar name").desired_width(160.0));
            if ui.button("Create New Calendar").clicked() && !d.new_name.trim().is_empty() {
                runs.push(("calendar.new".into(), json!({"name": d.new_name.trim(), "copyFrom": d.calendar})));
                switch_to = Some(d.new_name.trim().to_string());
            }
        });
        ui.separator();
        ui.label(RichText::new("Work week (hours like 08:00-12:00,13:00-17:00, or Nonworking)").strong());
        egui::Grid::new("wt_week").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            for (k, day) in WEEKDAYS.iter().enumerate() {
                ui.label(crate::commands::capitalize(day));
                ui.add(egui::TextEdit::singleline(&mut d.week[k]).desired_width(300.0));
                ui.end_row();
            }
        });
        if ui.button("Apply work week").clicked() {
            for k in 0..7 {
                if d.week[k] != d.orig_week[k] {
                    runs.push(("calendar.setWorkWeek".into(), json!({"calendar": d.calendar, "day": WEEKDAYS[k], "hours": d.week[k]})));
                }
            }
        }
        ui.separator();
        ui.label(RichText::new("Exceptions").strong());
        egui::Grid::new("wt_exc").num_columns(5).striped(true).show(ui, |ui| {
            for h in ["Name", "Start", "Finish", "Hours", ""] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (n, s, e, h) in &exceptions {
                ui.label(n);
                ui.label(s);
                ui.label(e);
                ui.label(h);
                if ui.small_button("Delete").clicked() {
                    runs.push(("calendar.removeException".into(), json!({"calendar": d.calendar, "start": s})));
                }
                ui.end_row();
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut d.exc_name).desired_width(110.0).hint_text("name"));
            ui.add(egui::TextEdit::singleline(&mut d.exc_start).desired_width(90.0).hint_text("2026-12-24"));
            ui.add(egui::TextEdit::singleline(&mut d.exc_end).desired_width(90.0).hint_text("end (optional)"));
            ui.add(egui::TextEdit::singleline(&mut d.exc_hours).desired_width(110.0).hint_text("hours or blank"));
            if ui.button("Add").clicked() {
                let mut p = json!({"calendar": d.calendar, "name": d.exc_name, "start": d.exc_start});
                if !d.exc_end.trim().is_empty() {
                    p["end"] = json!(d.exc_end);
                }
                if !d.exc_hours.trim().is_empty() {
                    p["hours"] = json!(d.exc_hours);
                }
                runs.push(("calendar.addException".into(), p));
            }
        });
        ui.separator();
        if ui.button("Close").clicked() {
            close = true;
        }
    });
    for (c, p) in runs {
        app.run(&c, p);
    }
    if let Some(n) = switch_to {
        d = work_time_state(app, &n);
    } else {
        let fresh = work_time_state(app, &d.calendar);
        if fresh.orig_week != d.orig_week {
            d.orig_week = fresh.orig_week.clone();
            d.week = fresh.week;
        }
    }
    if open && !close {
        app.dialogs.working_time = Some(d);
    }
}

fn options(app: &mut App, ctx: &Context) {
    let Some((mut d, orig)) = app.dialogs.options.take() else { return };
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    dlg(ctx, "Project Options").open(&mut open).collapsible(false).default_width(440.0).show(ctx, |ui| {
        ui.label(RichText::new("Calendar options").strong());
        egui::Grid::new("op1").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            row(ui, "Default start time:", &mut d.start);
            row(ui, "Default end time:", &mut d.finish);
            row(ui, "Hours per day:", &mut d.hpd);
            row(ui, "Hours per week:", &mut d.hpw);
            row(ui, "Days per month:", &mut d.dpm);
        });
        ui.separator();
        ui.label(RichText::new("Scheduling options").strong());
        egui::Grid::new("op2").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Duration is entered in:");
            egui::ComboBox::from_id_salt("op_du").selected_text(crate::commands::capitalize(d.dur_unit.id())).show_ui(ui, |ui| {
                for u in DurUnit::ALL {
                    ui.selectable_value(&mut d.dur_unit, u, crate::commands::capitalize(u.id()));
                }
            });
            ui.end_row();
            ui.label("Work is entered in:");
            egui::ComboBox::from_id_salt("op_wu").selected_text(crate::commands::capitalize(d.work_unit.id())).show_ui(ui, |ui| {
                for u in DurUnit::ALL {
                    ui.selectable_value(&mut d.work_unit, u, crate::commands::capitalize(u.id()));
                }
            });
            ui.end_row();
            ui.label("Default task type:");
            egui::ComboBox::from_id_salt("op_tt").selected_text(d.ttype.title()).show_ui(ui, |ui| {
                for t in TaskType::ALL {
                    ui.selectable_value(&mut d.ttype, t, t.title());
                }
            });
            ui.end_row();
            row(ui, "Critical when slack ≤ (days):", &mut d.crit);
            row(ui, "Currency symbol:", &mut d.currency);
        });
        ui.checkbox(&mut d.manual, "New tasks are manually scheduled");
        ui.checkbox(&mut d.effort, "New tasks are effort driven");
        ui.checkbox(&mut d.honor, "Tasks will always honor their constraint dates");
        ui.checkbox(&mut d.auto, "Calculate the schedule after every edit");
        let (o, c) = buttons(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let steps = schedule_steps(&d, &orig);
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.options = Some((d, orig));
    }
}

/// The scheduling options as the dialog edits them.
pub fn schedule_state(p: &Project) -> OptionsDlg {
    let st = &p.settings;
    OptionsDlg {
        hpd: fmt_num(st.minutes_per_day as f64 / 60.0),
        hpw: fmt_num(st.minutes_per_week as f64 / 60.0),
        dpm: st.days_per_month.to_string(),
        start: format!("{:02}:{:02}", st.default_start_min / 60, st.default_start_min % 60),
        finish: format!("{:02}:{:02}", st.default_finish_min / 60, st.default_finish_min % 60),
        crit: fmt_num(st.critical_slack as f64 / st.minutes_per_day as f64),
        honor: st.honor_constraints,
        manual: st.new_tasks_manual,
        effort: st.new_tasks_effort_driven,
        ttype: st.default_task_type,
        currency: st.currency.clone(),
        auto: st.auto_calc,
        dur_unit: st.duration_unit,
        work_unit: st.work_unit,
    }
}

/// The commands that apply what changed in the scheduling options.
pub fn schedule_steps(d: &OptionsDlg, orig: &OptionsDlg) -> Vec<Value> {
    {
        let mut steps = vec![];
        let mut opt = |id: &str, v: Value, changed: bool| {
            if changed {
                steps.push(json!({"cmd": format!("project.option.{id}"), "params": {"value": v}}));
            }
        };
        opt("defaultStartTime", json!(d.start), d.start != orig.start);
        opt("defaultFinishTime", json!(d.finish), d.finish != orig.finish);
        opt("hoursPerDay", json!(d.hpd), d.hpd != orig.hpd);
        opt("hoursPerWeek", json!(d.hpw), d.hpw != orig.hpw);
        opt("daysPerMonth", json!(d.dpm), d.dpm != orig.dpm);
        opt("defaultTaskType", json!(d.ttype.id()), d.ttype != orig.ttype);
        opt("criticalSlack", json!(d.crit), d.crit != orig.crit);
        opt("currency", json!(d.currency), d.currency != orig.currency);
        opt("newTasksManual", json!(d.manual), d.manual != orig.manual);
        opt("newTasksEffortDriven", json!(d.effort), d.effort != orig.effort);
        opt("honorConstraints", json!(d.honor), d.honor != orig.honor);
        opt("autoCalculate", json!(d.auto), d.auto != orig.auto);
        if d.dur_unit != orig.dur_unit {
            steps.push(json!({"cmd": format!("project.option.durationUnit.{}", d.dur_unit.id())}));
        }
        if d.work_unit != orig.work_unit {
            steps.push(json!({"cmd": format!("project.option.workUnit.{}", d.work_unit.id())}));
        }
        steps
    }
}

fn palette(app: &mut App, ctx: &Context) {
    if !app.dialogs.palette {
        return;
    }
    let mut open = true;
    let th = theme(app.s.view.dark);
    let mut run_it: Option<(String, String)> = None;
    let reg = app.reg;
    dlg(ctx, format!("Command Palette  ·  {} commands", reg.len())).open(&mut open).collapsible(false).default_width(640.0).default_height(520.0).show(ctx, |ui| {
        let r = ui.add(egui::TextEdit::singleline(&mut app.palette.query).hint_text("Type to search: insert, baseline, filter critical, zoom...").desired_width(f32::INFINITY));
        if !app.palette.focus {
            r.request_focus();
            app.palette.focus = true;
        }
        let words: Vec<String> = app.palette.query.to_lowercase().split_whitespace().map(String::from).collect();
        let matches: Vec<&crate::commands::Command> = reg
            .commands
            .iter()
            .filter(|c| {
                let hay = format!("{} {} {}", c.id, c.title, c.help).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter {
            if let Some(c) = matches.first() {
                app.palette.selected = Some(c.id.clone());
                app.palette.params = params_template(app, &c.id);
            }
        }
        ui.label(RichText::new(format!("{} matching", matches.len())).small().color(th.dim));
        egui::ScrollArea::vertical().max_height(230.0).id_salt("pal_list").show(ui, |ui| {
            for c in matches.iter().take(400) {
                let sel = app.palette.selected.as_deref() == Some(c.id.as_str());
                let label = format!("{}   —   {}", c.title, c.id);
                if ui.selectable_label(sel, label).on_hover_text(&c.help).clicked() {
                    app.palette.selected = Some(c.id.clone());
                    app.palette.params = params_template(app, &c.id);
                    app.palette.result.clear();
                }
            }
        });
        ui.separator();
        if let Some(id) = app.palette.selected.clone() {
            if let Some(c) = reg.find(&id) {
                ui.label(RichText::new(&c.title).strong().size(15.0));
                ui.label(&c.help);
                for p in &c.params {
                    ui.label(RichText::new(format!("• {} ({}{}) — {}", p.name, p.kind, if p.required { ", required" } else { "" }, p.help)).small());
                }
                ui.label("Parameters (JSON):");
                ui.add(egui::TextEdit::multiline(&mut app.palette.params).code_editor().desired_rows(4).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if ui.button("▶ Run").clicked() {
                        run_it = Some((id.clone(), app.palette.params.clone()));
                    }
                    ui.label(RichText::new(format!("CLI: plancraft-cli run plan.plan.json --cmd {} --params '{}'", id, app.palette.params.replace('\n', "").replace("  ", ""))).small().monospace().color(th.dim));
                });
            }
        }
        if !app.palette.result.is_empty() {
            ui.separator();
            egui::ScrollArea::vertical().max_height(160.0).id_salt("pal_res").show(ui, |ui| {
                ui.label(RichText::new(&app.palette.result).monospace().small());
            });
        }
    });
    if let Some((id, params)) = run_it {
        match serde_json::from_str::<Value>(if params.trim().is_empty() { "{}" } else { &params }) {
            Ok(p) => {
                let res = app.run_value(&id, p);
                app.palette.result = match res {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_default().chars().take(6000).collect(),
                    Err(e) => format!("Error: {e}"),
                };
            }
            Err(e) => app.palette.result = format!("Parameters are not valid JSON: {e}"),
        }
    }
    if !open {
        app.dialogs.palette = false;
    }
}

fn find(app: &mut App, ctx: &Context) {
    let Some(mut d) = app.dialogs.find.take() else { return };
    let mut open = true;
    let mut runs: Vec<(String, Value)> = vec![];
    let mut close = false;
    dlg(ctx, "Find and Replace").open(&mut open).collapsible(false).default_width(380.0).show(ctx, |ui| {
        egui::Grid::new("fr").num_columns(2).show(ui, |ui| {
            row(ui, "Find what:", &mut d.text);
            row(ui, "Replace with:", &mut d.replace);
            ui.label("Look in field:");
            egui::ComboBox::from_id_salt("fr_f").selected_text(fields::task_field(&d.field).map(|f| f.title).unwrap_or("Name")).show_ui(ui, |ui| {
                for f in TASK_FIELDS.iter().filter(|f| f.kind == fields::FieldKind::Text) {
                    ui.selectable_value(&mut d.field, f.id.to_string(), f.title);
                }
            });
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Find All").clicked() {
                runs.push(("edit.find".into(), json!({"text": d.text, "field": d.field})));
            }
            if ui.button("Replace All").clicked() {
                runs.push(("edit.replace".into(), json!({"find": d.text, "replace": d.replace, "field": d.field})));
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    });
    for (c, p) in runs {
        app.run(&c, p);
    }
    if open && !close {
        app.dialogs.find = Some(d);
    }
}

fn reports(app: &mut App, ctx: &Context) {
    let list = std::mem::take(&mut app.dialogs.reports);
    let mut keep = vec![];
    let th = theme(app.s.view.dark);
    for (title, v) in list {
        let mut open = true;
        let size = ctx.screen_rect().size();
        let (w, h) = (if title == "Project Overview" { 860.0_f32 } else { 900.0 }, if title == "Project Overview" { 600.0_f32 } else { 560.0 });
        dlg(ctx, &title)
            .id(egui::Id::new(("report", &title)))
            .open(&mut open)
            .default_size([w.min(size.x - 60.0), h.min(size.y - 80.0)])
            .min_width(480.0)
            .max_width(size.x - 40.0)
            .max_height(size.y - 60.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt(("rep_page", &title)).auto_shrink([false, false]).show(ui, |ui| {
                    super::report_view::show(app, ui, &title, &v);
                });
            });
        let _ = th;
        if open {
            keep.push((title, v));
        }
    }
    keep.extend(std::mem::take(&mut app.dialogs.reports));
    app.dialogs.reports = keep;
}

fn small_windows(app: &mut App, ctx: &Context) {
    let th = theme(app.s.view.dark);
    if app.dialogs.about {
        let mut open = true;
        dlg(ctx, "About").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.heading(crate::APP_NAME);
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label("A project scheduling app written entirely in Rust.");
            ui.label(format!("{} commands, shared by the menus, ribbon, dialogs, command line, JSON control channel and MCP server.", app.reg.len()));
        });
        app.dialogs.about = open;
    }
    if app.dialogs.agent {
        let mut open = true;
        let port = app.control_port;
        dlg(ctx, "Scripts and Agents").open(&mut open).collapsible(false).default_width(620.0).show(ctx, |ui| {
            ui.label("Everything in this window is a command. Scripts and AI agents run the same commands:");
            ui.add_space(4.0);
            ui.label(RichText::new("Command line (headless)").strong());
            ui.label(RichText::new("plancraft-cli run office.plan.json --cmd task.insert --params '{\"name\":\"Paint\",\"duration\":\"3d\"}' --out office.plan.json").monospace().small());
            ui.label(RichText::new("plancraft-cli batch --actions weekly.json --in ./plans --out ./updated").monospace().small());
            ui.add_space(4.0);
            ui.label(RichText::new("JSON control channel (this window)").strong());
            match port {
                Some(p) => ui.label(RichText::new(format!("tcp 127.0.0.1:{p} — send lines like {{\"id\":1,\"cmd\":\"task.list\"}}")).monospace().small()),
                None => ui.label(RichText::new(format!("off: {}", app.control_note)).color(th.danger)),
            };
            ui.add_space(4.0);
            ui.label(RichText::new("MCP server for AI agents").strong());
            let cfg = "{\"mcpServers\": {\"plancraft\": {\"command\": \"plancraft-cli\", \"args\": [\"mcp\", \"--bridge\"]}}}";
            ui.label(RichText::new(cfg).monospace().small());
            ui.horizontal(|ui| {
                if ui.button("Copy MCP config").clicked() {
                    ui.ctx().copy_text(cfg.to_string());
                }
                if ui.button("Open Command Palette").clicked() {
                    app.dialogs.palette = true;
                }
            });
            ui.label(RichText::new("Without --bridge the MCP server works on its own copy of a plan, with no window.").small().color(th.dim));
        });
        app.dialogs.agent = open;
    }
    if app.dialogs.stats {
        let mut open = true;
        let o = crate::reports::overview(&app.s.project);
        dlg(ctx, "Project Statistics").open(&mut open).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("stats").striped(true).show(ui, |ui| {
                if let Some(m) = o.as_object() {
                    for (k, v) in m {
                        if k == "warnings" {
                            continue;
                        }
                        ui.label(super::pretty(k));
                        let txt = match (k.as_str(), v) {
                            ("cost" | "baselineCost", Value::Number(n)) => app.s.project.fmt_cost(n.as_f64().unwrap_or(0.0)),
                            ("percentComplete", Value::Number(n)) => format!("{}%", n),
                            _ => fields::value_as_text(v),
                        };
                        ui.label(txt);
                        ui.end_row();
                    }
                }
            });
            for w in app.s.project.warnings.iter().take(10) {
                ui.label(RichText::new(format!("⚠ {w}")).color(th.danger));
            }
        });
        app.dialogs.stats = open;
    }
    if let Some(mut units) = app.dialogs.assign.take() {
        let mut open = true;
        let mut runs: Vec<(String, Value)> = vec![];
        let ids: Vec<usize> = app.s.view.selected.iter().filter_map(|u| app.s.project.task_index(*u)).map(|i| i + 1).collect();
        dlg(ctx, "Assign Resources").open(&mut open).collapsible(false).default_width(420.0).show(ctx, |ui| {
            let names: Vec<String> = ids.iter().map(|i| app.s.project.tasks[i - 1].name.clone()).collect();
            ui.label(if names.is_empty() { "Select tasks first.".to_string() } else { format!("Task: {}", names.join(", ")) });
            ui.horizontal(|ui| {
                ui.label("Units:");
                ui.add(egui::TextEdit::singleline(&mut units).desired_width(70.0));
            });
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                egui::Grid::new("asg").striped(true).show(ui, |ui| {
                    for r in &app.s.project.resources {
                        let assigned = ids.iter().any(|i| app.s.project.assignments_of_task(app.s.project.tasks[i - 1].uid).any(|a| a.resource == r.uid));
                        ui.label(if assigned { "✔" } else { "" });
                        ui.label(&r.name);
                        ui.label(RichText::new(r.kind.title()).color(th.dim));
                        if ui.add_enabled(!ids.is_empty(), egui::Button::new("Assign")).clicked() {
                            runs.push(("resource.assign".into(), json!({"ids": ids, "resource": r.name, "units": units.trim_end_matches('%').trim()})));
                        }
                        if ui.add_enabled(assigned, egui::Button::new("Remove")).clicked() {
                            runs.push(("resource.unassign".into(), json!({"ids": ids, "resource": r.name})));
                        }
                        ui.end_row();
                    }
                });
            });
        });
        for (c, p) in runs {
            app.run(&c, p);
        }
        if open {
            app.dialogs.assign = Some(units);
        }
    }
    if app.dialogs.columns {
        let mut open = true;
        let mut runs: Vec<String> = vec![];
        dlg(ctx, "Columns").open(&mut open).collapsible(false).default_width(260.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for f in TASK_FIELDS {
                    let mut on = app.s.view.columns.iter().any(|c| c == f.id);
                    if ui.checkbox(&mut on, f.title).clicked() {
                        runs.push(format!("view.column.{}.{}", if on { "show" } else { "hide" }, f.id));
                    }
                }
            });
        });
        for c in runs {
            app.run(&c, json!({}));
        }
        app.dialogs.columns = open;
    }
    if let Some((cmd, params)) = app.dialogs.confirm.take() {
        let mut choice = 0;
        dlg(ctx, "Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(format!("Do you want to save the changes to '{}'?", app.s.project.title));
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = 1;
                }
                if ui.button("Don't Save").clicked() {
                    choice = 2;
                }
                if ui.button("Cancel").clicked() {
                    choice = 3;
                }
            });
        });
        match choice {
            0 => app.dialogs.confirm = Some((cmd, params)),
            1 => {
                app.run("file.save", json!({}));
                if !app.s.dirty {
                    let mut p = params.clone();
                    p["force"] = json!(true);
                    app.run(&cmd, p);
                }
            }
            2 => {
                let mut p = params.clone();
                p["force"] = json!(true);
                app.run(&cmd, p);
            }
            _ => {}
        }
    }
    if let Some((t, m)) = app.dialogs.message.take() {
        let mut open = true;
        dlg(ctx, &t).open(&mut open).collapsible(false).show(ctx, |ui| {
            ui.label(&m);
        });
        if open {
            app.dialogs.message = Some((t, m));
        }
    }
}
