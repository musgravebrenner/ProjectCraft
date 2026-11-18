//! File > Options: a window with tabs down the left (General, Display, Schedule, Save, Language,
//! Advanced, Customize Ribbon, Quick Access Toolbar, Agents). It edits a copy; OK runs the
//! `app.options.set`, `view.toggle.*` and `project.option.*` commands for what changed.

use super::chrome::{glyph, Glyph};
use super::dialogs::{schedule_state, schedule_steps, OptionsDlg};
use super::{icons, theme, App};
use crate::commands::Command;
use crate::model::*;
use crate::prefs::{self, Prefs};
use crate::session::ViewKind;
use egui::{vec2, Color32, RichText, Sense, Stroke};
use serde_json::{json, Value};

pub struct OptionsState {
    pub tab: String,
    pub p: Prefs,
    orig: Prefs,
    sched: OptionsDlg,
    orig_sched: OptionsDlg,
    flags: Vec<(String, bool)>,
    orig_flags: Vec<(String, bool)>,
    qa_filter: String,
    qa_pick: Option<String>,
    qa_sel: Option<usize>,
    autosave_on: bool,
    autosave_text: String,
}

pub const TABS: &[(&str, &str, &str)] = &[
    ("general", "General", "General options for working with PlanCraft."),
    ("display", "Display", "Change how plans are displayed on the screen."),
    ("schedule", "Schedule", "Change options related to scheduling, calendars and calculations."),
    ("save", "Save", "Customize how plans are saved."),
    ("language", "Language", "Set the language and the way dates are written."),
    ("advanced", "Advanced", "Advanced options for working with PlanCraft."),
    ("-", "", ""),
    ("ribbon", "Customize Ribbon", "Choose which tabs the ribbon shows."),
    ("quickAccess", "Quick Access Toolbar", "Customize the Quick Access Toolbar in the title bar."),
    ("-", "", ""),
    ("agents", "Agents", "Let scripts and AI agents drive PlanCraft."),
];

/// Display elements offered on the Display tab (view flag ids).
const DISPLAY_FLAGS: &[(&str, &str)] = &[
    ("timeline", "Show the Timeline"),
    ("details", "Show the details pane"),
    ("todayLine", "Show the today line"),
    ("gridlines", "Show gridlines"),
    ("linkLines", "Show link lines between bars"),
    ("barText", "Show text next to bars"),
    ("progress", "Show progress on bars"),
    ("nonworkingTime", "Shade nonworking time"),
    ("outlineNumbers", "Show outline numbers"),
    ("projectSummary", "Show the project summary task"),
    ("summaryTasks", "Show summary tasks"),
    ("criticalPath", "Highlight critical tasks"),
];

/// Commands offered first when picking Quick Access Toolbar buttons.
const POPULAR: &[&str] = &[
    "file.save", "edit.undo", "edit.redo", "file.new", "file.open", "file.print", "file.saveAs", "file.export.pdf", "task.insert", "task.insertMilestone",
    "task.insertSummary", "task.indent", "task.outdent", "task.link", "task.unlink", "task.markComplete.100", "task.information", "task.notes", "resource.assignDialog",
    "resource.level", "project.information", "project.baseline.set.0", "view.zoom.entireProject", "view.goToToday", "view.toggle.timeline", "view.toggle.details",
    "view.toggle.criticalPath", "edit.find", "edit.copy", "edit.paste", "report.projectOverview", "app.commandPalette",
];

pub fn open(app: &mut App, tab: &str) {
    let tab = TABS.iter().find(|t| t.0.eq_ignore_ascii_case(tab) || t.1.eq_ignore_ascii_case(tab)).map(|t| t.0).unwrap_or("general");
    let p = prefs::get();
    let sched = schedule_state(&app.s.project);
    let flags: Vec<(String, bool)> = DISPLAY_FLAGS.iter().map(|(f, _)| (f.to_string(), crate::commands::view_flag(&app.s.view, f))).collect();
    app.app_options = Some(OptionsState {
        tab: tab.to_string(),
        autosave_on: p.autosave_minutes > 0,
        autosave_text: if p.autosave_minutes > 0 { p.autosave_minutes.to_string() } else { "10".into() },
        orig: p.clone(),
        p,
        orig_sched: sched.clone(),
        sched,
        orig_flags: flags.clone(),
        flags,
        qa_filter: String::new(),
        qa_pick: None,
        qa_sel: None,
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong().size(14.5));
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment([r.left_top() + vec2(0.0, 3.0), r.right_top() + vec2(-8.0, 3.0)], Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color));
    ui.add_space(9.0);
}

fn combo(ui: &mut egui::Ui, id: &str, value: &mut String, list: &[(&str, &str)], width: f32) -> bool {
    let mut changed = false;
    let cur = list.iter().find(|(k, _)| *k == value.as_str()).map(|x| x.1).unwrap_or(value.as_str()).to_string();
    egui::ComboBox::from_id_salt(id).width(width).selected_text(cur).show_ui(ui, |ui| {
        for (k, label) in list {
            if ui.selectable_label(value == k, *label).clicked() {
                *value = k.to_string();
                changed = true;
            }
        }
    });
    changed
}

fn label_cell(ui: &mut egui::Ui, text: &str) {
    ui.allocate_ui_with_layout(vec2(200.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(200.0);
        ui.label(text);
    });
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let Some(mut st) = app.app_options.take() else { return };
    let th = theme(app.s.view.dark);
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    let screen = ctx.screen_rect();
    let size = vec2(860.0_f32.min(screen.width() - 40.0), 620.0_f32.min(screen.height() - 60.0));
    super::dialogs::dlg(ctx, "PlanCraft Options")
        .id(egui::Id::new("app_options"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .fixed_size(size)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            // outlined text boxes and dropdowns, as in office options windows
            let v = ui.visuals_mut();
            v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, th.grid);
            v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, th.accent);
            let body_h = size.y - 48.0;
            ui.horizontal_top(|ui| {
                // tab list
                egui::Frame::new().fill(th.panel).stroke(Stroke::new(1.0_f32, th.grid)).inner_margin(egui::Margin::same(4)).show(ui, |ui| ui.vertical(|ui| {
                    ui.set_width(188.0);
                    ui.set_height(body_h);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (id, label, _) in TABS {
                        if *id == "-" {
                            ui.add_space(3.0);
                            ui.separator();
                            ui.add_space(3.0);
                            continue;
                        }
                        let sel = st.tab == *id;
                        let (r, resp) = ui.allocate_exact_size(vec2(188.0, 30.0), Sense::click());
                        if sel {
                            ui.painter().rect_filled(r, egui::CornerRadius::same(3), if app.s.view.dark { th.bg } else { Color32::WHITE });
                            ui.painter().rect_stroke(r, egui::CornerRadius::same(3), Stroke::new(1.5_f32, th.text), egui::StrokeKind::Inside);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, egui::CornerRadius::same(3), th.accent_light);
                        }
                        ui.painter().text(r.left_center() + vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, *label, egui::FontId::proportional(14.0), th.text);
                        if resp.clicked() {
                            st.tab = id.to_string();
                        }
                    }
                }));
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.set_width(size.x - 230.0);
                    egui::ScrollArea::vertical().id_salt(("opt_page", st.tab.clone())).max_height(body_h).auto_shrink([false, false]).show(ui, |ui| {
                        let (g, desc) = page_header(&st.tab);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                            glyph(ui.painter(), r.shrink(4.0), g, th.accent);
                            ui.label(RichText::new(desc).size(15.5));
                        });
                        match st.tab.as_str() {
                            "display" => display_tab(app, ui, &mut st),
                            "schedule" => schedule_tab(ui, &mut st),
                            "save" => save_tab(ui, &mut st),
                            "language" => language_tab(ui, &mut st),
                            "advanced" => advanced_tab(app, ui, &mut st),
                            "ribbon" => ribbon_tab(ui, &mut st),
                            "quickAccess" => quick_access_tab(app, ui, &mut st),
                            "agents" => agents_tab(app, ui),
                            _ => general_tab(ui, &mut st),
                        }
                    });
                });
            });
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Cancel").min_size(vec2(96.0, 28.0))).clicked() {
                    cancel = true;
                }
                if ui.add(egui::Button::new(RichText::new("OK").strong()).min_size(vec2(96.0, 28.0))).clicked() {
                    ok = true;
                }
            });
        });
    if ok {
        apply(app, &mut st);
        return;
    }
    if open && !cancel {
        app.app_options = Some(st);
    }
}

fn page_header(tab: &str) -> (Glyph, &'static str) {
    let desc = TABS.iter().find(|t| t.0 == tab).map(|t| t.2).unwrap_or("");
    let g = match tab {
        "display" => Glyph::Picture,
        "schedule" => Glyph::Table,
        "save" => Glyph::Save,
        "language" => Glyph::Web,
        "advanced" => Glyph::Options,
        "ribbon" | "quickAccess" => Glyph::Doc,
        "agents" => Glyph::Agent,
        _ => Glyph::Info,
    };
    (g, desc)
}

fn general_tab(ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "User interface options");
    ui.horizontal(|ui| {
        label_cell(ui, "ScreenTip style:");
        let mut v = if st.p.screen_tips { "on".to_string() } else { "off".to_string() };
        if combo(ui, "opt_tips", &mut v, &[("on", "Show feature descriptions in ScreenTips"), ("off", "Don't show ScreenTips")], 300.0) {
            st.p.screen_tips = v == "on";
        }
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Text size:");
        ui.add(egui::Slider::new(&mut st.p.text_size, 9.0..=24.0).step_by(1.0).suffix(" pt"));
    });
    section(ui, "Plan view");
    ui.horizontal(|ui| {
        label_cell(ui, "Default view:");
        let views: Vec<(String, String)> = ViewKind::menu_order().iter().map(|k| (k.id().to_string(), k.title().to_string())).collect();
        let list: Vec<(&str, &str)> = views.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        combo(ui, "opt_view", &mut st.p.default_view, &list, 240.0);
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Date format:");
        combo(ui, "opt_date_g", &mut st.p.date_style, prefs::DATE_STYLES, 240.0);
    });
    section(ui, "Personalize your copy of PlanCraft");
    ui.horizontal(|ui| {
        label_cell(ui, "User name:");
        ui.add(egui::TextEdit::singleline(&mut st.p.user_name).desired_width(260.0));
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Initials:");
        ui.add(egui::TextEdit::singleline(&mut st.p.initials).desired_width(60.0).char_limit(4));
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Theme:");
        combo(ui, "opt_theme", &mut st.p.theme, prefs::THEMES, 240.0);
    });
    ui.label(RichText::new("New plans use your name as their manager.").small().weak());
    section(ui, "Start up options");
    ui.checkbox(&mut st.p.show_start_screen, "Show the File screen when PlanCraft starts");
    ui.checkbox(&mut st.p.open_last_on_startup, "Open the last plan when PlanCraft starts");
}

fn display_tab(app: &App, ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "Show these elements");
    egui::Grid::new("opt_flags").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
        for (k, (id, title)) in DISPLAY_FLAGS.iter().enumerate() {
            if let Some(f) = st.flags.iter_mut().find(|f| f.0 == *id) {
                ui.checkbox(&mut f.1, *title);
            }
            if k % 2 == 1 {
                ui.end_row();
            }
        }
        ui.checkbox(&mut st.p.show_status_bar, "Show the status bar");
        ui.end_row();
    });
    section(ui, "Currency options for this plan");
    ui.horizontal(|ui| {
        label_cell(ui, "Symbol:");
        ui.add(egui::TextEdit::singleline(&mut st.sched.currency).desired_width(60.0));
        ui.label(RichText::new(format!("Example: {}1,234.50", st.sched.currency)).weak());
    });
    let _ = app;
}

fn schedule_tab(ui: &mut egui::Ui, st: &mut OptionsState) {
    let d = &mut st.sched;
    section(ui, "Calendar options for this plan");
    for (label, v) in [("Default start time:", &mut d.start), ("Default end time:", &mut d.finish), ("Hours per day:", &mut d.hpd), ("Hours per week:", &mut d.hpw), ("Days per month:", &mut d.dpm)] {
        ui.horizontal(|ui| {
            label_cell(ui, label);
            ui.add(egui::TextEdit::singleline(v).desired_width(80.0));
        });
    }
    section(ui, "Scheduling options for this plan");
    ui.horizontal(|ui| {
        label_cell(ui, "New tasks created:");
        let mut v = if d.manual { "manual".to_string() } else { "auto".to_string() };
        if combo(ui, "opt_newmode", &mut v, &[("auto", "Auto Scheduled"), ("manual", "Manually Scheduled")], 200.0) {
            d.manual = v == "manual";
        }
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Duration is entered in:");
        egui::ComboBox::from_id_salt("opt_du").width(200.0).selected_text(crate::commands::capitalize(d.dur_unit.id())).show_ui(ui, |ui| {
            for u in DurUnit::ALL {
                ui.selectable_value(&mut d.dur_unit, u, crate::commands::capitalize(u.id()));
            }
        });
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Work is entered in:");
        egui::ComboBox::from_id_salt("opt_wu").width(200.0).selected_text(crate::commands::capitalize(d.work_unit.id())).show_ui(ui, |ui| {
            for u in DurUnit::ALL {
                ui.selectable_value(&mut d.work_unit, u, crate::commands::capitalize(u.id()));
            }
        });
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Default task type:");
        egui::ComboBox::from_id_salt("opt_tt").width(200.0).selected_text(d.ttype.title()).show_ui(ui, |ui| {
            for t in TaskType::ALL {
                ui.selectable_value(&mut d.ttype, t, t.title());
            }
        });
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Critical when slack is at most:");
        ui.add(egui::TextEdit::singleline(&mut d.crit).desired_width(60.0));
        ui.label("days");
    });
    ui.checkbox(&mut d.effort, "New tasks are effort driven");
    ui.checkbox(&mut d.honor, "Tasks will always honor their constraint dates");
    section(ui, "Calculation");
    ui.label("Calculate the plan after each edit:");
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.radio_value(&mut d.auto, true, "On");
        ui.radio_value(&mut d.auto, false, "Off");
    });
}

fn save_tab(ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "Save plans");
    ui.horizontal(|ui| {
        label_cell(ui, "Save files in this format:");
        combo(ui, "opt_fmt", &mut st.p.save_format, prefs::SAVE_FORMATS, 260.0);
    });
    ui.horizontal(|ui| {
        label_cell(ui, "Default file location:");
        ui.add(egui::TextEdit::singleline(&mut st.p.default_folder).hint_text("Documents").desired_width(300.0));
        if ui.button("Browse...").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                st.p.default_folder = dir.to_string_lossy().to_string();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut st.autosave_on, "Auto save every");
        ui.add_enabled(st.autosave_on, egui::TextEdit::singleline(&mut st.autosave_text).desired_width(40.0));
        ui.label("minutes");
    });
    ui.label(RichText::new("Auto save writes the open plan to its file; a plan that hasn't been saved yet is left alone.").small().weak());
    section(ui, "Opening other formats");
    ui.label("PlanCraft opens .plan.json, XML interchange files and .mpp files from the desktop scheduling program (2010 and later). An .mpp opens as a new plan, since .mpp can't be written.");
}

fn language_tab(ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "Display language");
    ui.horizontal(|ui| {
        label_cell(ui, "Interface language:");
        combo(ui, "opt_lang", &mut st.p.language, prefs::LANGUAGES, 260.0);
    });
    ui.label(RichText::new("More interface languages aren't available yet.").small().weak());
    section(ui, "Dates");
    ui.label("Show and read dates as:");
    for (k, label) in prefs::DATE_STYLES {
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.radio_value(&mut st.p.date_style, k.to_string(), *label);
        });
    }
    ui.label(RichText::new("Dates you type follow the same order; 2026-11-16 always works.").small().weak());
}

fn advanced_tab(app: &mut App, ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "General");
    ui.horizontal(|ui| {
        label_cell(ui, "Undo levels:");
        let mut n = st.p.undo_levels as f64;
        if ui.add(egui::DragValue::new(&mut n).range(1.0..=1000.0).speed(1.0)).changed() {
            st.p.undo_levels = n as usize;
        }
    });
    section(ui, "Display");
    ui.horizontal(|ui| {
        label_cell(ui, "Show this number of recent plans:");
        let mut n = st.p.recent_count as f64;
        if ui.add(egui::DragValue::new(&mut n).range(0.0..=50.0).speed(1.0)).changed() {
            st.p.recent_count = n as usize;
        }
    });
    ui.checkbox(&mut st.p.show_status_bar, "Show the status bar");
    ui.checkbox(&mut st.p.show_quick_access, "Show the Quick Access Toolbar");
    section(ui, "Reset");
    if ui.button("Reset all settings to their defaults").clicked() {
        st.p = Prefs::default();
        st.autosave_on = false;
    }
    let _ = app;
}

fn ribbon_tab(ui: &mut egui::Ui, st: &mut OptionsState) {
    section(ui, "Main tabs");
    for (id, title) in prefs::RIBBON_TABS {
        let mut shown = !st.p.hidden_tabs.iter().any(|t| t == id);
        let label = if *id == "format" { "Format (named after the view)".to_string() } else { title.to_string() };
        if ui.checkbox(&mut shown, label).changed() {
            st.p.hidden_tabs.retain(|t| t != id);
            if !shown {
                st.p.hidden_tabs.push(id.to_string());
            }
        }
    }
    ui.add_space(6.0);
    ui.label(RichText::new("File is always shown. Every command stays available from All Commands and the search box.").small().weak());
}

fn command_row(ui: &mut egui::Ui, c: &Command, selected: bool, width: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    let th = theme(ui.visuals().dark_mode);
    if selected {
        ui.painter().rect_filled(r, egui::CornerRadius::same(2), th.select);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, egui::CornerRadius::same(2), th.accent_light);
    }
    let icon = egui::Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(16.0, 16.0));
    match icons::for_command(&c.id) {
        Some(src) => {
            egui::Image::new(src).fit_to_exact_size(vec2(16.0, 16.0)).paint_at(ui, icon);
        }
        None => {
            if let Some(g) = super::chrome::command_glyph(&c.id) {
                glyph(ui.painter(), icon, g, th.text);
            }
        }
    }
    ui.painter().text(r.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, &c.title, egui::FontId::proportional(13.5), th.text);
    resp.on_hover_text(format!("{}\n{}", c.help, c.id))
}

fn quick_access_tab(app: &mut App, ui: &mut egui::Ui, st: &mut OptionsState) {
    let reg = app.reg;
    ui.add_space(8.0);
    let list_w = 215.0;
    let list_h = 320.0;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.label("Choose commands from:");
            ui.add(egui::TextEdit::singleline(&mut st.qa_filter).hint_text("Popular commands (type to search all)").desired_width(list_w));
            egui::Frame::new().stroke(Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color)).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("qa_left").max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
                    ui.set_width(list_w);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let q = st.qa_filter.to_lowercase();
                    let cmds: Vec<&'static Command> = if q.is_empty() {
                        POPULAR.iter().filter_map(|id| reg.find(id)).collect()
                    } else {
                        reg.commands.iter().filter(|c| c.title.to_lowercase().contains(&q) || c.id.to_lowercase().contains(&q)).take(200).collect()
                    };
                    for c in cmds {
                        let sel = st.qa_pick.as_deref() == Some(c.id.as_str());
                        let r = command_row(ui, c, sel, list_w);
                        if r.clicked() {
                            st.qa_pick = Some(c.id.clone());
                        }
                        if r.double_clicked() && !st.p.quick_access.contains(&c.id) {
                            st.p.quick_access.push(c.id.clone());
                        }
                    }
                });
            });
        });
        ui.vertical(|ui| {
            ui.add_space(list_h / 2.0);
            let can_add = st.qa_pick.as_ref().map(|id| !st.p.quick_access.contains(id)).unwrap_or(false);
            if ui.add_enabled(can_add, egui::Button::new("Add >>").min_size(vec2(84.0, 26.0))).clicked() {
                if let Some(id) = st.qa_pick.clone() {
                    st.p.quick_access.push(id);
                    st.qa_sel = Some(st.p.quick_access.len() - 1);
                }
            }
            if ui.add_enabled(st.qa_sel.is_some(), egui::Button::new("<< Remove").min_size(vec2(84.0, 26.0))).clicked() {
                if let Some(i) = st.qa_sel.take() {
                    if i < st.p.quick_access.len() {
                        st.p.quick_access.remove(i);
                    }
                }
            }
        });
        ui.vertical(|ui| {
            ui.label("Shown in the title bar:");
            ui.add_space(26.0);
            egui::Frame::new().stroke(Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color)).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("qa_right").max_height(list_h).min_scrolled_height(list_h).show(ui, |ui| {
                    ui.set_width(list_w);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let ids = st.p.quick_access.clone();
                    for (i, id) in ids.iter().enumerate() {
                        if let Some(c) = reg.find(id) {
                            if command_row(ui, c, st.qa_sel == Some(i), list_w).clicked() {
                                st.qa_sel = Some(i);
                            }
                        }
                    }
                });
            });
        });
        ui.vertical(|ui| {
            ui.add_space(list_h / 2.0);
            if let Some(i) = st.qa_sel {
                if ui.add_enabled(i > 0, egui::Button::new(" Up ")).on_hover_text("Move up").clicked() {
                    st.p.quick_access.swap(i, i - 1);
                    st.qa_sel = Some(i - 1);
                }
                if ui.add_enabled(i + 1 < st.p.quick_access.len(), egui::Button::new("Down")).on_hover_text("Move down").clicked() {
                    st.p.quick_access.swap(i, i + 1);
                    st.qa_sel = Some(i + 1);
                }
            }
        });
    });
    ui.add_space(8.0);
    ui.checkbox(&mut st.p.show_quick_access, "Show Quick Access Toolbar");
    if ui.button("Reset to Save, Undo, Redo").clicked() {
        st.p.quick_access = Prefs::default().quick_access;
        st.qa_sel = None;
    }
}

fn agents_tab(app: &App, ui: &mut egui::Ui) {
    section(ui, "This window");
    match app.control_port {
        Some(p) => ui.label(format!("Listening on 127.0.0.1:{p} for the JSON control channel. Agents can run any of the {} commands; the status bar shows what they did.", app.reg.len())),
        None => ui.label(format!("Agent control is off for this window{}.", if app.control_note.is_empty() { String::new() } else { format!(" ({})", app.control_note) })),
    };
    section(ui, "Connect an agent");
    for (label, text) in [("MCP server for this window:", "plancraft-cli mcp --bridge"), ("MCP server without a window:", "plancraft-cli mcp"), ("Change a setting from a script:", "plancraft-cli run --cmd app.options.set --params '{\"key\":\"theme\",\"value\":\"dark\"}'")] {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.label(RichText::new(text).monospace());
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(text.to_string());
            }
        });
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Every option on these tabs is also a command: app.options.get lists them, app.options.set changes one.").small().weak());
}

fn apply(app: &mut App, st: &mut OptionsState) {
    if st.autosave_on {
        st.p.autosave_minutes = st.autosave_text.trim().parse::<u32>().unwrap_or(10).clamp(1, 120);
    } else {
        st.p.autosave_minutes = 0;
    }
    let mut steps: Vec<Value> = vec![];
    let a = serde_json::to_value(&st.p).unwrap_or_default();
    let b = serde_json::to_value(&st.orig).unwrap_or_default();
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for (k, v) in a {
            if b.get(k) != Some(v) {
                steps.push(json!({"cmd": "app.options.set", "params": {"key": k, "value": v}}));
            }
        }
    }
    for ((id, on), (_, was)) in st.flags.iter().zip(&st.orig_flags) {
        if on != was {
            steps.push(json!({"cmd": format!("view.toggle.{id}"), "params": {"on": on}}));
        }
    }
    steps.extend(schedule_steps(&st.sched, &st.orig_sched));
    if !steps.is_empty() {
        app.run("app.batch", json!({"steps": steps}));
    }
}
