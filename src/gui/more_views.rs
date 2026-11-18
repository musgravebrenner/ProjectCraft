//! Task Form, Resource Form, Resource Graph and Team Planner.

use super::gantt::{chart_x, for_buckets, timescale, HEADER_H};
use super::{rgb, theme, App};
use crate::fields;
use crate::model::*;
use crate::schedule;
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use serde_json::{json, Value};

/// Previous / Next buttons that walk through tasks (or resources) in ID order.
fn nav_bar(app: &mut App, ui: &mut Ui, resources: bool) {
    let (prev, next) = ui.horizontal(|ui| (ui.button("Previous").clicked(), ui.button("Next").clicked())).inner;
    if !(prev || next) {
        return;
    }
    let step: i64 = if next { 1 } else { -1 };
    if resources {
        let n = app.s.project.resources.len() as i64;
        if n == 0 {
            return;
        }
        let cur = app.s.view.selected_resources.iter().next().and_then(|u| app.s.project.resource_index(*u)).map(|i| i as i64).unwrap_or(-1);
        let k = (cur + step).clamp(0, n - 1) as usize;
        app.run("resource.select", json!({"resource": k + 1}));
    } else {
        let n = app.s.project.tasks.len() as i64;
        if n == 0 {
            return;
        }
        let cur = app.s.current_task().map(|i| i as i64).unwrap_or(-1);
        let k = (cur + step).clamp(0, n - 1) as usize;
        app.run("edit.select", json!({"ids": [k + 1]}));
    }
}

pub fn task_form_view(app: &mut App, ui: &mut Ui) {
    ui.add_space(6.0);
    nav_bar(app, ui, false);
    ui.separator();
    if app.s.current_task().is_none() && !app.s.project.tasks.is_empty() {
        let uid = app.s.project.tasks[0].uid;
        app.s.view.selected.insert(uid);
    }
    egui::ScrollArea::vertical().id_salt("taskform").show(ui, |ui| super::views::details_pane(app, ui));
}

fn current_resource(app: &mut App) -> Option<usize> {
    let cur = app.s.view.selected_resources.iter().next().and_then(|u| app.s.project.resource_index(*u));
    if cur.is_none() && !app.s.project.resources.is_empty() {
        let uid = app.s.project.resources[0].uid;
        app.s.view.selected_resources.insert(uid);
        return Some(0);
    }
    cur
}

pub fn resource_form_view(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    ui.add_space(6.0);
    nav_bar(app, ui, true);
    ui.separator();
    let Some(ri) = current_resource(app) else {
        ui.label(RichText::new("There are no resources yet. Add one on the Resource tab.").color(th.dim));
        return;
    };
    let id = ri + 1;
    let mut edit = false;
    {
        let pr = &app.s.project;
        let r = &pr.resources[ri];
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", id, r.name)).strong().size(16.0));
            ui.label(RichText::new(r.kind.title()).color(th.dim));
            edit = ui.button("Edit...").clicked();
        });
        egui::Grid::new("resform").num_columns(4).spacing([24.0, 6.0]).show(ui, |ui| {
            for pair in [["initials", "maxUnits"], ["group", "stdRate"], ["code", "ovtRate"], ["calendar", "costPerUse"], ["email", "work"]] {
                for f in pair {
                    let title = fields::resource_field(f).map(|d| d.title).unwrap_or(f);
                    ui.label(RichText::new(format!("{title}:")).color(th.dim));
                    ui.label(fields::resource_text(pr, ri, f));
                }
                ui.end_row();
            }
        });
        ui.separator();
        ui.label(RichText::new("Assignments").strong());
        egui::Grid::new("resform_asg").striped(true).spacing([20.0, 4.0]).show(ui, |ui| {
            for h in ["ID", "Task Name", "Units", "Work", "Start", "Finish", "Cost"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for a in pr.assignments_of_resource(r.uid) {
                let Some(ti) = pr.task_index(a.task) else { continue };
                let t = &pr.tasks[ti];
                ui.label((ti + 1).to_string());
                ui.label(&t.name);
                ui.label(match r.kind {
                    ResourceKind::Work => format!("{}%", fmt_num(a.units * 100.0)),
                    ResourceKind::Material => format!("{} {}", fmt_num(a.units), r.material_label),
                    ResourceKind::Cost => pr.fmt_cost(a.cost_amount),
                });
                ui.label(if r.kind == ResourceKind::Work { pr.fmt_work(a.work) } else { String::new() });
                ui.label(fmt_date(t.start));
                ui.label(fmt_date(t.finish));
                ui.label(pr.fmt_cost(a.cost));
                ui.end_row();
            }
        });
    }
    if edit {
        app.run("resource.information", json!({"resource": id}));
    }
}

/// Average units a resource works in [a, b): assigned work divided by available working time.
fn load_in(pr: &Project, r: &Resource, a: chrono::NaiveDateTime, b: chrono::NaiveDateTime) -> Option<f64> {
    let cal = r.calendar.as_deref().and_then(|c| pr.calendar_named(c)).unwrap_or(pr.cal());
    let avail = cal.work_between(a, b);
    if avail <= 0 {
        return None;
    }
    let w: i64 = pr.assignments_of_resource(r.uid).map(|asg| schedule::assignment_work_in(pr, asg, a, b)).sum();
    Some(w as f64 / avail as f64)
}

pub fn resource_graph_view(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let side_w = 220.0;
    let chart_left = full.left() + side_w;
    let hdr = Rect::from_min_max(pos2(chart_left, full.top()), pos2(full.right(), full.top() + HEADER_H));
    timescale(app, ui, hdr, &th);
    let resp = ui.interact(hdr, Id::new("rg_hdr"), Sense::drag());
    if resp.dragged() {
        app.left_days -= resp.drag_delta().x as f64 / app.s.view.px_per_day as f64;
    }
    let side = Rect::from_min_max(full.min, pos2(chart_left - 4.0, full.bottom()));
    ui.painter().rect_filled(side, 0.0, th.panel);
    let ri = current_resource(app);
    let mut pick: Option<usize> = None;
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(side.shrink(6.0)), |ui| {
        ui.label(RichText::new("Resource").strong());
        egui::ScrollArea::vertical().id_salt("rg_list").max_height(side.height() * 0.6).show(ui, |ui| {
            for (i, r) in app.s.project.resources.iter().enumerate() {
                if r.kind != ResourceKind::Work {
                    continue;
                }
                let txt = if r.overallocated { RichText::new(&r.name).color(th.danger) } else { RichText::new(&r.name) };
                if ui.selectable_label(Some(i) == ri, txt).clicked() {
                    pick = Some(i + 1);
                }
            }
        });
        ui.separator();
        for (label, col) in [("Allocated", Color32::from_rgb(70, 120, 190)), ("Overallocated", th.danger)] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                ui.painter().rect_filled(r, 1.0, col);
                ui.label(label);
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(2.0_f32, th.text));
            ui.label("Max. units");
        });
    });
    if let Some(id) = pick {
        app.run("resource.select", json!({"resource": id}));
    }
    let Some(mut ri) = current_resource(app) else { return };
    // the graph is about people and machines: skip material and cost resources
    if app.s.project.resources[ri].kind != ResourceKind::Work {
        if let Some(k) = app.s.project.resources.iter().position(|r| r.kind == ResourceKind::Work) {
            ri = k;
            app.s.view.selected_resources = [app.s.project.resources[k].uid].into_iter().collect();
        }
    }
    let pr = &app.s.project;
    let r = pr.resources[ri].clone();
    let body = Rect::from_min_max(pos2(chart_left, full.top() + HEADER_H), pos2(full.right(), full.bottom() - 4.0));
    let p = ui.painter().with_clip_rect(body);
    p.rect_filled(body, 0.0, th.bg);
    // what to plot: units (as %), hours, or money; cumulative modes add up as they go
    let mode = if app.s.view.graph_mode.is_empty() { "peakUnits".to_string() } else { app.s.view.graph_mode.clone() };
    #[derive(PartialEq)]
    enum Unit {
        Pct,
        Hours,
        Money,
    }
    let unit = match mode.as_str() {
        "work" | "cumulativeWork" => Unit::Hours,
        "cost" | "cumulativeCost" => Unit::Money,
        _ => Unit::Pct,
    };

    let mut top_val = match unit {
        Unit::Pct => r.max_units.max(0.01),
        Unit::Hours => 8.0,
        Unit::Money => 100.0,
    };
    let mut bars: Vec<(f32, f32, f64, f64)> = vec![]; // x0, x1, normal part, over part
    let mut running = 0.0f64;
    for_buckets(app, body.left(), body.right(), |a, b, xa, xb| {
        let work: i64 = pr.assignments_of_resource(r.uid).map(|asg| schedule::assignment_work_in(pr, asg, a, b)).sum();
        let hours = work as f64 / 60.0;
        let (normal, over) = match mode.as_str() {
            "work" => (hours, 0.0),
            "cumulativeWork" => {
                running += hours;
                (running, 0.0)
            }
            "cost" => (hours * r.std_rate, 0.0),
            "cumulativeCost" => {
                running += hours * r.std_rate;
                (running, 0.0)
            }
            m => match load_in(pr, &r, a, b) {
                Some(u) => match m {
                    "overallocation" => (0.0, (u - r.max_units).max(0.0)),
                    "percentAllocation" => (u.min(r.max_units), 0.0),
                    "remainingAvailability" => ((r.max_units - u).max(0.0), 0.0),
                    _ => (u.min(r.max_units), (u - r.max_units).max(0.0)),
                },
                None => return,
            },
        };
        top_val = top_val.max(normal + over);
        bars.push((xa, xb, normal, over));
    });
    let scale_top = top_val * 1.25;
    let y_of = |u: f64| body.bottom() - 18.0 - ((u / scale_top) as f32) * (body.height() - 30.0);
    let fmt_val = |v: f64| match unit {
        Unit::Pct => format!("{}%", fmt_num((v * 100.0).round())),
        Unit::Hours => format!("{}h", fmt_num(v.round())),
        Unit::Money => pr.fmt_cost(v),
    };
    // gridlines at round steps
    let raw = scale_top / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(raw);
    let step = if unit == Unit::Pct { step.max(0.25) } else { step };
    let mut g = 0.0f64;
    while g <= scale_top + 1e-9 {
        let y = y_of(g);
        p.line_segment([pos2(body.left(), y), pos2(body.right(), y)], Stroke::new(1.0_f32, th.grid));
        p.text(pos2(body.left() + 3.0, y - 2.0), Align2::LEFT_BOTTOM, fmt_val(g), FontId::proportional(10.0), th.dim);
        g += step;
    }
    let base_col = match mode.as_str() {
        "remainingAvailability" => Color32::from_rgb(70, 160, 110),
        "cost" | "cumulativeCost" => Color32::from_rgb(220, 160, 50),
        _ => Color32::from_rgb(70, 120, 190),
    };
    for (xa, xb, normal, over) in bars {
        let w = (xb - xa).max(2.0);
        let x0 = xa + w * 0.15;
        let x1 = xb - w * 0.15;
        if normal > 0.0 {
            p.rect_filled(Rect::from_min_max(pos2(x0, y_of(normal)), pos2(x1, y_of(0.0))), 0.0, base_col);
        }
        if over > 1e-6 {
            p.rect_filled(Rect::from_min_max(pos2(x0, y_of(normal + over)), pos2(x1, y_of(normal))), 0.0, th.danger);
        }
        if normal + over > 0.0 && xb - xa > 34.0 {
            p.text(pos2((x0 + x1) / 2.0, y_of(normal + over) - 2.0), Align2::CENTER_BOTTOM, fmt_val(normal + over), FontId::proportional(10.0), th.text);
        }
    }
    let mode_title = crate::commands::GRAPH_MODES.iter().find(|(k, _)| *k == mode).map(|(_, t)| *t).unwrap_or("Peak Units");
    if unit == Unit::Pct && mode != "remainingAvailability" {
        let ym = y_of(r.max_units);
        p.line_segment([pos2(body.left(), ym), pos2(body.right(), ym)], Stroke::new(2.0_f32, th.text));
        p.text(pos2(body.right() - 6.0, ym - 3.0), Align2::RIGHT_BOTTOM, format!("max {}%", fmt_num(r.max_units * 100.0)), FontId::proportional(12.0), th.text);
    }
    p.text(pos2(body.right() - 6.0, body.top() + 6.0), Align2::RIGHT_TOP, format!("{}: {}", r.name, mode_title), FontId::proportional(13.0), th.text);
    let dx = ui.input(|i| if i.pointer.hover_pos().map(|p| body.contains(p)).unwrap_or(false) { i.smooth_scroll_delta.x } else { 0.0 });
    if dx != 0.0 {
        app.left_days -= dx as f64 / app.s.view.px_per_day as f64;
    }
}

type Lane = (String, Option<u32>, Vec<usize>);

pub fn team_planner_view(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let side_w = 200.0;
    let chart_left = full.left() + side_w;
    let hdr = Rect::from_min_max(pos2(chart_left, full.top()), pos2(full.right(), full.top() + HEADER_H));
    timescale(app, ui, hdr, &th);
    let hresp = ui.interact(hdr, Id::new("tp_hdr"), Sense::drag());
    if hresp.dragged() {
        app.left_days -= hresp.drag_delta().x as f64 / app.s.view.px_per_day as f64;
    }
    {
        let hl = Rect::from_min_max(full.min, pos2(chart_left, full.top() + HEADER_H));
        ui.painter().rect_filled(hl, 0.0, th.header);
        ui.painter().text(pos2(hl.left() + 8.0, hl.bottom() - 12.0), Align2::LEFT_CENTER, "Resource Name", FontId::proportional(app.s.view.font_size - 0.5), th.text);
    }
    let pr = &app.s.project;
    let mut lanes: Vec<Lane> = vec![];
    for r in pr.resources.iter().filter(|r| r.kind == ResourceKind::Work) {
        let tasks: Vec<usize> = pr.assignments_of_resource(r.uid).filter_map(|a| pr.task_index(a.task)).filter(|&i| !pr.tasks[i].summary && pr.tasks[i].active).collect();
        lanes.push((r.name.clone(), Some(r.uid), tasks));
    }
    let unassigned: Vec<usize> = (0..pr.tasks.len())
        .filter(|&i| {
            let t = &pr.tasks[i];
            !t.summary && t.active && !t.is_milestone() && !pr.assignments_of_task(t.uid).any(|a| pr.resource_by_uid(a.resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false))
        })
        .collect();
    lanes.push(("Unassigned Tasks".to_string(), None, unassigned));
    let row_h = app.s.view.row_height;
    let bar_h = row_h - 4.0;
    let fs = app.s.view.font_size;
    let body = Rect::from_min_max(pos2(full.left(), full.top() + HEADER_H), full.max);
    let mut runs: Vec<(String, Value)> = vec![];
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body), |ui| {
        egui::ScrollArea::vertical().id_salt("teamplanner").auto_shrink([false, false]).show(ui, |ui| {
            let left = ui.cursor().left();
            let clip = ui.clip_rect();
            let cclip = Rect::from_min_max(pos2(chart_left, clip.top()), clip.max);
            for (name, ruid, tasks) in &lanes {
                // stack overlapping tasks into sub-rows
                let mut sub_ends: Vec<chrono::NaiveDateTime> = vec![];
                let mut placed: Vec<(usize, usize)> = vec![];
                let mut sorted = tasks.clone();
                sorted.sort_by_key(|&i| (pr.tasks[i].start, i));
                for i in sorted {
                    let t = &pr.tasks[i];
                    let k = match sub_ends.iter().position(|e| *e <= t.start) {
                        Some(k) => k,
                        None => {
                            sub_ends.push(t.start);
                            sub_ends.len() - 1
                        }
                    };
                    sub_ends[k] = t.finish;
                    placed.push((i, k));
                }
                let h = sub_ends.len().max(1) as f32 * row_h + 8.0;
                let (lane, _) = ui.allocate_exact_size(vec2(full.width(), h), Sense::hover());
                let lane = Rect::from_min_size(pos2(left, lane.top()), lane.size());
                let over = ruid.and_then(|u| pr.resource_by_uid(u)).map(|r| r.overallocated).unwrap_or(false);
                let p = ui.painter();
                p.rect_filled(Rect::from_min_max(lane.min, pos2(chart_left, lane.bottom())), 0.0, if ruid.is_none() { th.panel } else { th.header });
                p.line_segment([lane.left_bottom(), lane.right_bottom()], Stroke::new(1.0_f32, th.grid));
                let cy = lane.top() + 4.0 + row_h / 2.0;
                if ruid.is_some() {
                    p.circle_filled(pos2(lane.left() + 16.0, cy), 9.0, Color32::from_rgb(70, 120, 190));
                    let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                    p.text(pos2(lane.left() + 16.0, cy), Align2::CENTER_CENTER, initial, FontId::proportional(10.0), Color32::WHITE);
                }
                p.text(pos2(lane.left() + 32.0, cy), Align2::LEFT_CENTER, name, FontId::proportional(fs), if over { th.danger } else { th.text });
                let cp = ui.painter().with_clip_rect(cclip);
                for (i, k) in placed {
                    let t = &pr.tasks[i];
                    let x0 = chart_x(app, chart_left, t.start);
                    let x1 = chart_x(app, chart_left, t.finish).max(x0 + 4.0);
                    let top = lane.top() + 4.0 + k as f32 * row_h + 2.0;
                    let r = Rect::from_min_max(pos2(x0, top), pos2(x1, top + bar_h));
                    let col = if t.percent_complete >= 100.0 {
                        Color32::from_rgb(160, 160, 160)
                    } else if t.critical && app.s.view.show_critical {
                        rgb(app.s.view.bar_colors.critical)
                    } else if ruid.is_none() {
                        Color32::from_rgb(240, 196, 110)
                    } else {
                        rgb(app.s.view.bar_colors.task)
                    };
                    cp.rect_filled(r, 3.0, col);
                    let sel = app.s.view.selected.contains(&t.uid);
                    cp.rect_stroke(r, 3.0, Stroke::new(if sel { 2.0_f32 } else { 0.8 }, if sel { th.accent } else { col.gamma_multiply(0.6) }), StrokeKind::Inside);
                    cp.with_clip_rect(r.intersect(cclip)).text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, &t.name, FontId::proportional(fs - 1.5), Color32::from_rgb(20, 30, 50));
                    let resp = ui
                        .interact(r.intersect(cclip), Id::new(("tpbar", t.uid, name.as_str())), Sense::click())
                        .on_hover_text(format!("{}\n{} - {}\n{}", t.name, fmt_date(t.start), fmt_date(t.finish), t.duration.format(&pr.settings)));
                    if resp.clicked() {
                        runs.push(("edit.select".into(), json!({"ids": [i + 1]})));
                    }
                    if resp.double_clicked() {
                        runs.push(("task.information".into(), json!({"id": i + 1})));
                    }
                }
            }
        });
    });
    let dx = ui.input(|i| if i.pointer.hover_pos().map(|p| p.x > chart_left && body.contains(p)).unwrap_or(false) { i.smooth_scroll_delta.x } else { 0.0 });
    if dx != 0.0 {
        app.left_days -= dx as f64 / app.s.view.px_per_day as f64;
    }
    for (c, p) in runs {
        app.run(&c, p);
    }
}
