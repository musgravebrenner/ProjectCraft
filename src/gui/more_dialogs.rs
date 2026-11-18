//! Dialogs matching the original's: Text Styles, Task Inspector, Update Tasks and Set/Clear
//! Baseline. On OK each runs registry commands, so scripts can do the same.

use super::dialogs::{dlg, primary_button, secondary_button};
use super::{theme, App};
use crate::commands::TEXT_STYLE_ITEMS;
use crate::model::*;
use egui::{vec2, Color32, Context, RichText, Sense, Stroke};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct TextStylesDlg {
    pub item: String,
    pub styles: BTreeMap<String, TextStyle>,
    pub orig: BTreeMap<String, TextStyle>,
}

#[derive(Clone, PartialEq)]
pub struct UpdateDlg {
    pub pct: String,
    pub actual_dur: String,
    pub remaining_dur: String,
    pub actual_start: String,
    pub actual_finish: String,
}

#[derive(Clone)]
pub struct BaselineDlg {
    pub clear: bool,
    pub which: usize,
    pub selected_only: bool,
}

/// The colours offered in colour pickers, like the original's standard colours.
pub const SWATCHES: &[(&str, [u8; 3])] = &[
    ("Black", [0, 0, 0]),
    ("Dark Gray", [89, 89, 89]),
    ("Gray", [166, 166, 166]),
    ("Dark Red", [192, 0, 0]),
    ("Red", [255, 0, 0]),
    ("Orange", [255, 192, 0]),
    ("Yellow", [255, 255, 0]),
    ("Light Green", [146, 208, 80]),
    ("Green", [0, 176, 80]),
    ("Light Blue", [0, 176, 240]),
    ("Blue", [0, 112, 192]),
    ("Dark Blue", [0, 32, 96]),
    ("Purple", [112, 48, 160]),
    ("White", [255, 255, 255]),
];

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// A colour dropdown: "Automatic" plus the standard colours. Returns true when changed.
pub fn color_picker(ui: &mut egui::Ui, id: impl std::hash::Hash, value: &mut Option<[u8; 3]>, auto_label: &str) -> bool {
    let mut changed = false;
    let name = value.map(|c| SWATCHES.iter().find(|s| s.1 == c).map(|s| s.0.to_string()).unwrap_or_else(|| hex(c))).unwrap_or_else(|| auto_label.to_string());
    egui::ComboBox::from_id_salt(id).width(150.0).selected_text(name).show_ui(ui, |ui| {
        if ui.selectable_label(value.is_none(), auto_label).clicked() {
            *value = None;
            changed = true;
        }
        for (n, c) in SWATCHES {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
                ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::from_gray(150)), egui::StrokeKind::Inside);
                if ui.selectable_label(*value == Some(*c), *n).clicked() {
                    *value = Some(*c);
                    changed = true;
                }
            });
        }
    });
    changed
}

pub fn open(app: &mut App, d: &crate::session::Dialog) {
    use crate::session::Dialog;
    match d {
        Dialog::TextStyles => {
            let styles = app.s.view.text_styles.clone();
            app.dialogs.text_styles = Some(TextStylesDlg { item: "all".into(), orig: styles.clone(), styles });
        }
        Dialog::Inspector => app.dialogs.inspector = true,
        Dialog::UpdateTasks => {
            let Some(i) = app.s.current_task() else { return };
            let t = &app.s.project.tasks[i];
            let pr = &app.s.project;
            let actual = (t.duration.minutes as f64 * t.percent_complete / 100.0).round() as i64;
            let fmt_d = |m: i64| Dur { minutes: m, unit: t.duration.unit, ..Default::default() }.format(&pr.settings);
            let dlg = UpdateDlg {
                pct: crate::model::fmt_num(t.percent_complete),
                actual_dur: fmt_d(actual),
                remaining_dur: fmt_d(t.duration.minutes - actual),
                actual_start: t.actual_start.or(if t.percent_complete > 0.0 { Some(t.start) } else { None }).map(fmt_date).unwrap_or_else(|| "NA".into()),
                actual_finish: t.actual_finish.or(if t.percent_complete >= 100.0 { Some(t.finish) } else { None }).map(fmt_date).unwrap_or_else(|| "NA".into()),
            };
            app.dialogs.update_tasks = Some((dlg.clone(), dlg));
        }
        Dialog::SetBaseline => app.dialogs.set_baseline = Some(BaselineDlg { clear: false, which: 0, selected_only: false }),
        _ => {}
    }
}

pub fn show(app: &mut App, ctx: &Context) {
    text_styles(app, ctx);
    inspector(app, ctx);
    update_tasks(app, ctx);
    set_baseline(app, ctx);
}

fn ok_cancel(ui: &mut egui::Ui) -> (bool, bool) {
    let (mut ok, mut cancel) = (false, false);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        cancel = secondary_button(ui, "Cancel").clicked();
        ok = primary_button(ui, "OK").clicked();
    });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    (ok, cancel)
}

fn text_styles(app: &mut App, ctx: &Context) {
    let Some(mut d) = app.dialogs.text_styles.take() else { return };
    let (mut open, mut ok, mut cancel) = (true, false, false);
    let th = theme(app.s.view.dark);
    dlg(ctx, "Text Styles").open(&mut open).resizable(false).default_width(520.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Item to Change:");
            let cur = TEXT_STYLE_ITEMS.iter().find(|x| x.0 == d.item).map(|x| x.1).unwrap_or("All");
            egui::ComboBox::from_id_salt("ts_item").width(240.0).selected_text(cur).show_ui(ui, |ui| {
                for (k, label) in TEXT_STYLE_ITEMS {
                    ui.selectable_value(&mut d.item, k.to_string(), *label);
                }
            });
        });
        ui.add_space(8.0);
        let st = d.styles.entry(d.item.clone()).or_default();
        egui::Grid::new("ts_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.label("Font:");
            let mut font = st.font.clone().unwrap_or_else(|| "sans".into());
            egui::ComboBox::from_id_salt("ts_font").width(150.0).selected_text(if font == "mono" { "Monospace" } else { "Sans (default)" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut font, "sans".to_string(), "Sans (default)");
                ui.selectable_value(&mut font, "mono".to_string(), "Monospace");
            });
            st.font = if font == "mono" { Some(font) } else { None };
            ui.end_row();
            ui.label("Font style:");
            let mut fs = match (st.bold.unwrap_or(false), st.italic.unwrap_or(false)) {
                (false, false) => 0,
                (false, true) => 1,
                (true, false) => 2,
                (true, true) => 3,
            };
            egui::ComboBox::from_id_salt("ts_style").width(150.0).selected_text(["Regular", "Italic", "Bold", "Bold Italic"][fs]).show_ui(ui, |ui| {
                for (k, n) in ["Regular", "Italic", "Bold", "Bold Italic"].iter().enumerate() {
                    ui.selectable_value(&mut fs, k, *n);
                }
            });
            st.bold = if fs >= 2 { Some(true) } else if st.bold.is_some() { Some(false) } else { None };
            st.italic = if fs % 2 == 1 { Some(true) } else if st.italic.is_some() { Some(false) } else { None };
            ui.end_row();
            ui.label("Size:");
            let mut size = st.size.unwrap_or(0.0);
            egui::ComboBox::from_id_salt("ts_size").width(150.0).selected_text(if size == 0.0 { "Default".to_string() } else { format!("{size}") }).show_ui(ui, |ui| {
                ui.selectable_value(&mut size, 0.0, "Default");
                for n in [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0] {
                    ui.selectable_value(&mut size, n, format!("{n}"));
                }
            });
            st.size = if size == 0.0 { None } else { Some(size) };
            ui.end_row();
            ui.label("");
            let mut u = st.underline.unwrap_or(false);
            if ui.checkbox(&mut u, "Underline").changed() {
                st.underline = Some(u);
            }
            ui.end_row();
            ui.label("Color:");
            color_picker(ui, "ts_color", &mut st.color, "Automatic");
            ui.end_row();
            ui.label("Background color:");
            color_picker(ui, "ts_bg", &mut st.background, "Automatic");
            ui.end_row();
        });
        // sample
        ui.add_space(10.0);
        ui.label(RichText::new("Sample").color(th.dim));
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
        let bg = st.background.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(th.bg);
        ui.painter().rect_filled(r, 4.0, bg);
        ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0_f32, th.grid), egui::StrokeKind::Inside);
        let col = st.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(th.text);
        let style = st.clone();
        super::gantt::styled_text(ui.painter(), r.center(), egui::Align2::CENTER_CENTER, "AaBbYyZz", &style, app.s.view.font_size, col);
        let (o, c) = ok_cancel(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let mut steps: Vec<Value> = vec![];
        for (k, _) in TEXT_STYLE_ITEMS {
            let now = d.styles.get(*k).cloned().unwrap_or_default();
            let was = d.orig.get(*k).cloned().unwrap_or_default();
            if now != was {
                let c = |x: Option<[u8; 3]>| x.map(hex).unwrap_or_else(|| "automatic".into());
                steps.push(json!({"cmd": "format.textStyle.set", "params": {
                    "item": k, "bold": now.bold.unwrap_or(false), "italic": now.italic.unwrap_or(false), "underline": now.underline.unwrap_or(false),
                    "color": c(now.color), "background": c(now.background), "size": now.size.unwrap_or(0.0), "font": now.font.clone().unwrap_or_else(|| "sans".into())
                }}));
            }
        }
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.text_styles = Some(d);
    }
}

fn inspector(app: &mut App, ctx: &Context) {
    if !app.dialogs.inspector {
        return;
    }
    let mut open = true;
    let th = theme(app.s.view.dark);
    let info = app.s.current_task().map(|i| crate::schedule::inspect(&app.s.project, i));
    let mut goto: Option<u64> = None;
    dlg(ctx, "Task Inspector").open(&mut open).default_width(360.0).default_pos(ctx.screen_rect().right_top() + vec2(-200.0, 200.0)).show(ctx, |ui| {
        let Some(v) = info else {
            ui.label(RichText::new("Select a task to see what drives its dates.").color(th.dim));
            return;
        };
        ui.label(RichText::new(format!("Factors affecting: {} {}", v["id"], v["name"].as_str().unwrap_or(""))).strong().size(15.0));
        ui.label(RichText::new(format!("{}   ·   Start {}   ·   Finish {}", v["mode"].as_str().unwrap_or(""), v["start"].as_str().unwrap_or(""), v["finish"].as_str().unwrap_or(""))).color(th.dim));
        ui.add_space(8.0);
        for f in v["factors"].as_array().into_iter().flatten() {
            egui::Frame::new().fill(th.panel).corner_radius(egui::CornerRadius::same(4)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(f["text"].as_str().unwrap_or(""));
            });
            ui.add_space(4.0);
        }
        if let Some(preds) = v["predecessors"].as_array().filter(|p| !p.is_empty()) {
            ui.add_space(6.0);
            ui.label(RichText::new("Predecessor Tasks").strong());
            for (k, p) in preds.iter().enumerate() {
                ui.horizontal(|ui| {
                    let text = format!("{}  {}  ({} {}) allows start {}", p["id"], p["name"].as_str().unwrap_or(""), p["type"].as_str().unwrap_or(""), p["lag"].as_str().unwrap_or(""), p["allowsStartFrom"].as_str().unwrap_or(""));
                    let r = ui.link(RichText::new(text).color(if k == 0 { th.accent } else { th.text }));
                    if r.clicked() {
                        goto = p["id"].as_u64();
                    }
                });
            }
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Calendars").strong());
        for c in v["calendars"].as_array().into_iter().flatten() {
            ui.label(c.as_str().unwrap_or(""));
        }
        if let Some(w) = v["warnings"].as_array().filter(|w| !w.is_empty()) {
            ui.add_space(6.0);
            ui.label(RichText::new("Warnings").strong().color(th.danger));
            for x in w {
                ui.label(RichText::new(x.as_str().unwrap_or("")).color(th.danger));
            }
        }
    });
    if let Some(id) = goto {
        app.run("edit.select", json!({"ids": [id]}));
    }
    app.dialogs.inspector = open;
}

fn update_tasks(app: &mut App, ctx: &Context) {
    let Some((mut d, orig)) = app.dialogs.update_tasks.take() else { return };
    let (mut open, mut ok, mut cancel) = (true, false, false);
    let Some(i) = app.s.current_task() else { return };
    let ids: Vec<usize> = app.s.view.selected.iter().filter_map(|u| app.s.project.task_index(*u)).map(|i| i + 1).collect();
    let (name, dur, start, finish) = {
        let t = &app.s.project.tasks[i];
        (t.name.clone(), t.duration.format(&app.s.project.settings), fmt_date(t.start), fmt_date(t.finish))
    };
    dlg(ctx, "Update Tasks").open(&mut open).resizable(false).default_width(500.0).show(ctx, |ui| {
        egui::Grid::new("ut_head").num_columns(4).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Name:");
            ui.label(RichText::new(if ids.len() > 1 { format!("{} tasks", ids.len()) } else { name.clone() }).strong());
            ui.label("Duration:");
            ui.label(&dur);
            ui.end_row();
            ui.label("% Complete:");
            ui.add(egui::TextEdit::singleline(&mut d.pct).desired_width(70.0));
            ui.label("Actual dur:");
            ui.add(egui::TextEdit::singleline(&mut d.actual_dur).desired_width(90.0));
            ui.end_row();
            ui.label("");
            ui.label("");
            ui.label("Remaining dur:");
            ui.add(egui::TextEdit::singleline(&mut d.remaining_dur).desired_width(90.0));
            ui.end_row();
        });
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            cols[0].label(RichText::new("Actual").strong());
            egui::Grid::new("ut_act").num_columns(2).spacing([10.0, 6.0]).show(&mut cols[0], |ui| {
                ui.label("Start:");
                ui.add(egui::TextEdit::singleline(&mut d.actual_start).desired_width(140.0));
                ui.end_row();
                ui.label("Finish:");
                ui.add(egui::TextEdit::singleline(&mut d.actual_finish).desired_width(140.0));
                ui.end_row();
            });
            cols[1].label(RichText::new("Current").strong());
            egui::Grid::new("ut_cur").num_columns(2).spacing([10.0, 6.0]).show(&mut cols[1], |ui| {
                ui.label("Start:");
                ui.label(&start);
                ui.end_row();
                ui.label("Finish:");
                ui.label(&finish);
                ui.end_row();
            });
        });
        let (o, c) = ok_cancel(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        let mut steps: Vec<Value> = vec![];
        let set = |steps: &mut Vec<Value>, f: &str, v: Value| steps.push(json!({"cmd": format!("task.set.{f}"), "params": {"ids": ids, "value": v}}));
        let total = app.s.project.tasks[i].duration.minutes.max(1) as f64;
        let settings = app.s.project.settings.clone();
        let parse_d = |s: &str| Dur::parse(s, settings.duration_unit, &settings).map(|d| d.minutes as f64).ok();
        let mut pct: Option<f64> = None;
        if d.pct != orig.pct {
            pct = d.pct.trim().trim_end_matches('%').parse::<f64>().ok();
        } else if d.actual_dur != orig.actual_dur {
            pct = parse_d(&d.actual_dur).map(|a| a / total * 100.0);
        } else if d.remaining_dur != orig.remaining_dur {
            pct = parse_d(&d.remaining_dur).map(|r| (total - r) / total * 100.0);
        }
        if d.actual_start != orig.actual_start {
            set(&mut steps, "actualStart", json!(if d.actual_start.trim().eq_ignore_ascii_case("na") { String::new() } else { d.actual_start.clone() }));
        }
        if let Some(p) = pct {
            set(&mut steps, "percentComplete", json!(p.clamp(0.0, 100.0).round()));
        }
        if d.actual_finish != orig.actual_finish {
            set(&mut steps, "actualFinish", json!(if d.actual_finish.trim().eq_ignore_ascii_case("na") { String::new() } else { d.actual_finish.clone() }));
        }
        if !steps.is_empty() {
            app.run("app.batch", json!({"steps": steps}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.update_tasks = Some((d, orig));
    }
}

fn set_baseline(app: &mut App, ctx: &Context) {
    let Some(mut d) = app.dialogs.set_baseline.take() else { return };
    let (mut open, mut ok, mut cancel) = (true, false, false);
    let th = theme(app.s.view.dark);
    let saved: Vec<Option<chrono::NaiveDateTime>> = (0..=10).map(|n| app.s.project.baseline_saved.get(n).copied().flatten()).collect();
    let has_sel = !app.s.view.selected.is_empty();
    dlg(ctx, if d.clear { "Clear Baseline" } else { "Set Baseline" }).id(egui::Id::new("set_baseline")).open(&mut open).resizable(false).default_width(420.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut d.clear, false, "Set baseline");
            ui.selectable_value(&mut d.clear, true, "Clear baseline");
        });
        ui.add_space(8.0);
        let name = |n: usize| if n == 0 { "Baseline".to_string() } else { format!("Baseline {n}") };
        let label = |n: usize| match saved[n] {
            Some(t) => format!("{} (last saved on {})", name(n), fmt_date(t)),
            None => name(n),
        };
        egui::ComboBox::from_id_salt("bl_which").width(300.0).selected_text(label(d.which)).show_ui(ui, |ui| {
            for n in 0..=10 {
                ui.selectable_value(&mut d.which, n, label(n));
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new("For:").strong());
        ui.radio_value(&mut d.selected_only, false, "Entire project");
        ui.add_enabled_ui(has_sel && !d.clear, |ui| {
            ui.radio_value(&mut d.selected_only, true, "Selected tasks");
        });
        if d.clear {
            ui.label(RichText::new("Clearing removes the baseline from every task.").small().color(th.dim));
        }
        let (o, c) = ok_cancel(ui);
        ok = o;
        cancel = c;
    });
    if ok {
        if d.clear {
            app.run(&format!("project.baseline.clear.{}", d.which), json!({}));
        } else if d.selected_only {
            let ids: Vec<usize> = app.s.view.selected.iter().filter_map(|u| app.s.project.task_index(*u)).map(|i| i + 1).collect();
            app.run(&format!("project.baseline.set.{}", d.which), json!({"ids": ids}));
        } else {
            app.run(&format!("project.baseline.set.{}", d.which), json!({}));
        }
        return;
    }
    if open && !cancel {
        app.dialogs.set_baseline = Some(d);
    }
}
