//! Resource Sheet / Resource Usage, Network Diagram, Calendar, Timeline and the details pane.

use super::gantt::{chart_x, for_buckets, HEADER_H};
use super::{rgb, theme, App, CellEdit};
use crate::fields::{self, FieldDef};
use crate::model::*;
use crate::schedule;
use chrono::{Datelike, Duration, NaiveDate};
use egui::{pos2, vec2, Align2, Color32, CursorIcon, FontId, Id, Key, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------------------------
// Resource Sheet / Resource Usage
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
enum RRow {
    Res(usize),
    Asg(usize),
}

pub fn resource_view(app: &mut App, ui: &mut Ui, usage: bool) {
    let th = theme(app.s.view.dark);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let col_ids: Vec<String> = if usage { vec!["id".into(), "name".into(), "work".into(), "peak".into()] } else { app.s.view.resource_columns.clone() };
    let cols: Vec<&'static FieldDef> = col_ids.iter().filter_map(|c| fields::resource_field(c)).collect();
    let table_w = if usage { app.s.view.table_width.clamp(160.0, (full.width() - 160.0).max(160.0)).min(420.0) } else { full.width() };
    let mut widths: Vec<f32> = cols.iter().map(|c| c.width).collect();
    let sum: f32 = widths.iter().sum();
    if sum < table_w {
        if let Some(k) = cols.iter().position(|c| c.id == "name") {
            widths[k] += table_w - sum;
        }
    }
    let chart_left = full.left() + table_w + 5.0;
    let fs = app.s.view.font_size;
    // header
    let header = Rect::from_min_size(full.min, vec2(table_w, HEADER_H));
    {
        let p = ui.painter().with_clip_rect(header);
        p.rect_filled(header, 0.0, th.header);
        let mut x = header.left();
        for (k, c) in cols.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, header.top()), vec2(widths[k], HEADER_H));
            p.text(pos2(r.left() + 5.0, r.bottom() - 12.0), Align2::LEFT_CENTER, c.title, FontId::proportional(fs - 0.5), th.text);
            p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, th.grid));
            let resp = ui.interact(r, Id::new(("rhdr", c.id)), Sense::click());
            let cid = c.id;
            if !usage {
                resp.context_menu(|ui| {
                    if ui.button(format!("Hide Column '{}'", c.title)).clicked() {
                        app.run(&format!("view.resourceColumn.hide.{cid}"), json!({}));
                        ui.close_menu();
                    }
                    ui.menu_button("Insert Column", |ui| {
                        for f in fields::RESOURCE_FIELDS {
                            if ui.button(f.title).clicked() {
                                app.run(&format!("view.resourceColumn.show.{}", f.id), json!({}));
                                ui.close_menu();
                            }
                        }
                    });
                });
            }
            x += widths[k];
        }
        p.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0_f32, th.grid));
    }
    if usage {
        let hdr = Rect::from_min_max(pos2(chart_left, full.top()), pos2(full.right(), full.top() + HEADER_H));
        super::gantt::timescale(app, ui, hdr, &th);
        let resp = ui.interact(hdr, Id::new("rtshdr"), Sense::drag());
        if resp.dragged() {
            app.left_days -= resp.drag_delta().x as f64 / app.s.view.px_per_day as f64;
        }
        let split = Rect::from_min_max(pos2(full.left() + table_w, full.top()), pos2(chart_left, full.bottom()));
        ui.painter().rect_filled(split, 0.0, th.panel);
        let sr = ui.interact(split, Id::new("rsplit"), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
        if sr.dragged() {
            app.s.view.table_width = (app.s.view.table_width + sr.drag_delta().x).clamp(160.0, full.width() - 160.0);
        }
        let dx = ui.input(|i| if i.pointer.hover_pos().map(|p| p.x > chart_left).unwrap_or(false) { i.smooth_scroll_delta.x } else { 0.0 });
        if dx != 0.0 {
            app.left_days -= dx as f64 / app.s.view.px_per_day as f64;
        }
    }
    let mut rows = vec![];
    for (i, r) in app.s.project.resources.iter().enumerate() {
        rows.push(RRow::Res(i));
        if usage {
            for (ai, a) in app.s.project.assignments.iter().enumerate() {
                if a.resource == r.uid {
                    rows.push(RRow::Asg(ai));
                }
            }
        }
    }
    let body = Rect::from_min_max(pos2(full.left(), full.top() + HEADER_H), full.max);
    let row_h = app.s.view.row_height;
    let n = rows.len() + if usage { 0 } else { 6 };
    let mut runs: Vec<(String, Value)> = vec![];
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body), |ui| {
        egui::ScrollArea::vertical().id_salt(("resrows", usage)).auto_shrink([false, false]).show_rows(ui, row_h, n, |ui, range| {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), range.len() as f32 * row_h), Sense::hover());
            let clip = ui.clip_rect();
            let tclip = Rect::from_min_max(pos2(rect.left(), clip.top()), pos2(rect.left() + table_w, clip.bottom()));
            let cclip = Rect::from_min_max(pos2(chart_left, clip.top()), clip.max);
            for k in range.clone() {
                let y = rect.top() + (k - range.start) as f32 * row_h;
                let trow = Rect::from_min_size(pos2(rect.left(), y), vec2(table_w, row_h));
                let row = rows.get(k).cloned();
                let ruid = match &row {
                    Some(RRow::Res(i)) => Some(app.s.project.resources[*i].uid),
                    _ => None,
                };
                let selected = ruid.map(|u| app.s.view.selected_resources.contains(&u)).unwrap_or(false);
                let p = ui.painter().with_clip_rect(tclip);
                p.rect_filled(trow, 0.0, if selected { th.select } else if matches!(row, Some(RRow::Asg(_))) { th.panel } else { th.bg });
                let mut x = rect.left();
                for (ci, c) in cols.iter().enumerate() {
                    let cell = Rect::from_min_size(pos2(x, y), vec2(widths[ci], row_h));
                    if c.id == "id" {
                        p.rect_filled(cell, 0.0, if selected { th.select } else { th.header });
                    }
                    p.line_segment([cell.right_top(), cell.right_bottom()], Stroke::new(1.0_f32, th.grid));
                    let cy = cell.center().y;
                    match &row {
                        Some(RRow::Res(i)) => {
                            let r = &app.s.project.resources[*i];
                            let col = if r.overallocated { th.danger } else { th.text };
                            let text = fields::resource_text(&app.s.project, *i, c.id);
                            if c.id == "id" && r.overallocated {
                                p.circle_filled(pos2(cell.right() - 10.0, cy), 5.0, th.danger);
                                p.text(pos2(cell.right() - 10.0, cy), Align2::CENTER_CENTER, "!", FontId::proportional(9.0), Color32::WHITE);
                            }
                            let num = matches!(c.kind, fields::FieldKind::Cost | fields::FieldKind::Percent | fields::FieldKind::Work);
                            if num {
                                p.text(pos2(cell.right() - 5.0, cy), Align2::RIGHT_CENTER, text, FontId::proportional(fs), col);
                            } else {
                                p.text(pos2(cell.left() + 5.0, cy), Align2::LEFT_CENTER, text, FontId::proportional(fs), if c.id == "id" { th.dim } else { col });
                            }
                        }
                        Some(RRow::Asg(ai)) => {
                            let a = &app.s.project.assignments[*ai];
                            let text = match c.id {
                                "name" => app.s.project.task_by_uid(a.task).map(|t| format!("      {}", t.name)).unwrap_or_default(),
                                "work" => app.s.project.fmt_work(a.work),
                                "peak" => match app.s.project.resource_by_uid(a.resource).map(|r| (r.kind, r.material_label.clone())) {
                                    Some((ResourceKind::Material, label)) => format!("{} {}", fmt_num(a.units), label),
                                    Some((ResourceKind::Cost, _)) => app.s.project.fmt_cost(a.cost_amount),
                                    _ => format!("{}%", fmt_num(a.units * 100.0)),
                                },
                                _ => String::new(),
                            };
                            p.text(pos2(cell.left() + 5.0, cy), Align2::LEFT_CENTER, text, FontId::proportional(fs - 0.5), th.dim);
                        }
                        None => {}
                    }
                    x += widths[ci];
                }
                p.line_segment([trow.left_bottom(), trow.right_bottom()], Stroke::new(1.0_f32, th.grid));
                let resp = ui.interact(trow, Id::new(("rrow", usage, k)), Sense::click());
                if resp.clicked() {
                    app.edit = None;
                    if let Some(u) = ruid {
                        app.s.view.selected_resources = [u].into_iter().collect();
                    }
                }
                if resp.double_clicked() && !usage {
                    let px = resp.interact_pointer_pos().map(|p| p.x).unwrap_or(0.0);
                    let mut cx = rect.left();
                    let mut field = "name";
                    for (ci, c) in cols.iter().enumerate() {
                        if px >= cx && px < cx + widths[ci] {
                            field = c.id;
                        }
                        cx += widths[ci];
                    }
                    let editable = fields::resource_field(field).map(|f| f.editable).unwrap_or(false);
                    match (&row, editable) {
                        (Some(RRow::Res(i)), true) => {
                            let r = &app.s.project.resources[*i];
                            let mut buf = fields::resource_text(&app.s.project, *i, field);
                            if field == "stdRate" || field == "ovtRate" || field == "costPerUse" {
                                buf = fmt_num(match field {
                                    "stdRate" => r.std_rate,
                                    "ovtRate" => r.ovt_rate,
                                    _ => r.cost_per_use,
                                });
                            }
                            app.edit = Some(CellEdit { uid: Some(r.uid), field: field.into(), buf, focused: false, resource: true });
                        }
                        (Some(RRow::Res(i)), false) => {
                            let id = i + 1;
                            runs.push(("resource.information".into(), json!({"resource": id})));
                        }
                        (None, _) => app.edit = Some(CellEdit { uid: None, field: "name".into(), buf: String::new(), focused: false, resource: true }),
                        _ => {}
                    }
                }
                if let Some(RRow::Res(i)) = &row {
                    let id = i + 1;
                    resp.context_menu(|ui| {
                        if ui.button("Resource Information...").clicked() {
                            ui.close_menu();
                            app.run("resource.information", json!({"resource": id}));
                        }
                        if ui.button("Delete Resource").clicked() {
                            ui.close_menu();
                            app.run("resource.delete", json!({"resource": id}));
                        }
                        if ui.button("Level All").clicked() {
                            ui.close_menu();
                            app.run("resource.level", json!({}));
                        }
                    });
                }
                // in-cell editor
                if let Some(e) = app.edit.clone().filter(|e| e.resource) {
                    let this = match &row {
                        Some(RRow::Res(i)) => e.uid == Some(app.s.project.resources[*i].uid),
                        None => e.uid.is_none() && k == rows.len(),
                        _ => false,
                    };
                    if this {
                        let mut x = rect.left();
                        let mut cell = None;
                        for (ci, c) in cols.iter().enumerate() {
                            if c.id == e.field {
                                cell = Some(Rect::from_min_size(pos2(x, y), vec2(widths[ci], row_h)));
                            }
                            x += widths[ci];
                        }
                        if let Some(cell) = cell {
                            let ed = app.edit.as_mut().unwrap();
                            let r = ui.put(cell.shrink(1.0), egui::TextEdit::singleline(&mut ed.buf).margin(vec2(3.0, 1.0)));
                            if !ed.focused {
                                r.request_focus();
                                ed.focused = true;
                            }
                            if ui.input(|i| i.key_pressed(Key::Escape)) {
                                app.edit = None;
                            } else if r.lost_focus() {
                                let e = app.edit.take().unwrap();
                                let v = e.buf.trim().to_string();
                                match e.uid {
                                    Some(u) => {
                                        if let Some(ri) = app.s.project.resource_index(u) {
                                            runs.push((format!("resource.set.{}", e.field), json!({"resource": ri + 1, "value": v})));
                                        }
                                    }
                                    None if !v.is_empty() => runs.push(("resource.add".into(), json!({"name": v}))),
                                    None => {}
                                }
                            }
                        }
                    }
                }
                // usage cells
                if usage {
                    let cp = ui.painter().with_clip_rect(cclip);
                    let crow = Rect::from_min_max(pos2(chart_left, y), pos2(rect.right(), y + row_h));
                    cp.line_segment([crow.left_bottom(), crow.right_bottom()], Stroke::new(1.0_f32, th.grid));
                    let pr = &app.s.project;
                    let (asgs, max_units, cal): (Vec<&Assignment>, f64, &crate::calendar::Calendar) = match &row {
                        Some(RRow::Res(i)) => {
                            let r = &pr.resources[*i];
                            (pr.assignments_of_resource(r.uid).collect(), r.max_units, r.calendar.as_deref().and_then(|c| pr.calendar_named(c)).unwrap_or(pr.cal()))
                        }
                        Some(RRow::Asg(ai)) => (vec![&pr.assignments[*ai]], f64::MAX, pr.cal()),
                        None => (vec![], 1.0, pr.cal()),
                    };
                    let is_res = matches!(row, Some(RRow::Res(_)));
                    let work_kind = match &row {
                        Some(RRow::Res(i)) => pr.resources[*i].kind == ResourceKind::Work,
                        Some(RRow::Asg(ai)) => pr.resource_by_uid(pr.assignments[*ai].resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false),
                        None => false,
                    };
                    if work_kind {
                        for_buckets(app, chart_left, crow.right(), |a, b, xa, xb| {
                            cp.line_segment([pos2(xa, y), pos2(xa, y + row_h)], Stroke::new(1.0_f32, th.grid));
                            let w: i64 = asgs.iter().map(|asg| schedule::assignment_work_in(pr, asg, a, b)).sum();
                            if w <= 0 {
                                return;
                            }
                            let cap = cal.work_between(a, b) as f64 * max_units;
                            let over = is_res && w as f64 > cap + 0.5;
                            if over {
                                cp.rect_filled(Rect::from_min_max(pos2(xa + 1.0, y + 1.0), pos2(xb - 1.0, y + row_h - 1.0)), 0.0, th.danger.gamma_multiply(0.18));
                            }
                            if xb - xa > 17.0 {
                                cp.text(pos2(xb - 3.0, y + row_h / 2.0), Align2::RIGHT_CENTER, format!("{}h", fmt_num(w as f64 / 60.0)), FontId::proportional(fs - 1.5), if over { th.danger } else if is_res { th.text } else { th.dim });
                            }
                        });
                    }
                }
            }
        });
    });
    for (c, p) in runs {
        app.run(&c, p);
    }
}

// ---------------------------------------------------------------------------------------------
// Network Diagram
// ---------------------------------------------------------------------------------------------

pub fn network_view(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    let pr = &app.s.project;
    let n = pr.tasks.len();
    // column = longest chain of predecessors
    let mut depth = vec![0usize; n];
    let leaves: Vec<usize> = (0..n).filter(|&i| !pr.tasks[i].summary).collect();
    for _ in 0..n.max(1) {
        let mut changed = false;
        for &i in &leaves {
            for l in &pr.tasks[i].predecessors {
                if let Some(pi) = pr.task_index(l.pred) {
                    let preds: Vec<usize> = if pr.tasks[pi].summary { pr.descendants(pi).filter(|j| !pr.tasks[*j].summary).collect() } else { vec![pi] };
                    for q in preds {
                        if depth[q] + 1 > depth[i] && depth[q] < n {
                            depth[i] = depth[q] + 1;
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let (bw, bh, gx, gy) = (190.0f32, 78.0f32, 50.0f32, 22.0f32);
    let mut col_count: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut pos: std::collections::HashMap<usize, (usize, usize)> = std::collections::HashMap::new();
    for &i in &leaves {
        let c = depth[i];
        let r = *col_count.get(&c).unwrap_or(&0);
        col_count.insert(c, r + 1);
        pos.insert(i, (c, r));
    }
    let max_c = depth.iter().copied().max().unwrap_or(0) + 1;
    let max_r = col_count.values().copied().max().unwrap_or(1);
    let size = vec2(30.0 + max_c as f32 * (bw + gx), 30.0 + max_r as f32 * (bh + gy));
    let mut runs: Vec<(String, Value)> = vec![];
    egui::ScrollArea::both().id_salt("network").auto_shrink([false, false]).show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(size.max(ui.available_size()), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 0.0, th.bg);
        let box_at = |c: usize, r: usize| Rect::from_min_size(rect.min + vec2(20.0 + c as f32 * (bw + gx), 20.0 + r as f32 * (bh + gy)), vec2(bw, bh));
        let pr = &app.s.project;
        // arrows first
        for &i in &leaves {
            let (c, r) = pos[&i];
            let b = box_at(c, r);
            for l in &pr.tasks[i].predecessors {
                let Some(pi) = pr.task_index(l.pred) else { continue };
                let preds: Vec<usize> = if pr.tasks[pi].summary { pr.descendants(pi).filter(|j| !pr.tasks[*j].summary).collect() } else { vec![pi] };
                for q in preds {
                    let Some(&(qc, qr)) = pos.get(&q) else { continue };
                    let a = box_at(qc, qr);
                    let crit = pr.tasks[q].critical && pr.tasks[i].critical;
                    let col = if crit { th.danger } else { th.dim };
                    let s = pos2(a.right(), a.center().y);
                    let e = pos2(b.left(), b.center().y);
                    let mx = (s.x + e.x) / 2.0;
                    p.line_segment([s, pos2(mx, s.y)], Stroke::new(1.2_f32, col));
                    p.line_segment([pos2(mx, s.y), pos2(mx, e.y)], Stroke::new(1.2_f32, col));
                    p.line_segment([pos2(mx, e.y), e], Stroke::new(1.2_f32, col));
                    p.add(Shape::convex_polygon(vec![e, e + vec2(-7.0, -4.0), e + vec2(-7.0, 4.0)], col, Stroke::NONE));
                }
            }
        }
        for &i in &leaves {
            let (c, r) = pos[&i];
            let b = box_at(c, r);
            let t = &pr.tasks[i];
            let sel = app.s.view.selected.contains(&t.uid);
            let crit = t.critical;
            let border = if crit { th.danger } else { Color32::from_rgb(70, 110, 170) };
            let fill = if sel {
                th.select
            } else if crit {
                th.danger.gamma_multiply(0.10)
            } else {
                Color32::from_rgb(70, 110, 170).gamma_multiply(0.08)
            };
            p.rect_filled(b, 3.0, th.bg);
            p.rect_filled(b, 3.0, fill);
            p.rect_stroke(b, 3.0, Stroke::new(if t.is_milestone() { 2.5_f32 } else { 1.4 }, border), StrokeKind::Inside);
            if t.percent_complete >= 100.0 {
                p.line_segment([b.left_top(), b.right_bottom()], Stroke::new(1.0_f32, border.gamma_multiply(0.5)));
                p.line_segment([b.right_top(), b.left_bottom()], Stroke::new(1.0_f32, border.gamma_multiply(0.5)));
            } else if t.percent_complete > 0.0 {
                p.line_segment([b.left_top(), b.right_bottom()], Stroke::new(1.0_f32, border.gamma_multiply(0.5)));
            }
            let fs = 12.0;
            let name: String = t.name.chars().take(28).collect();
            p.text(b.left_top() + vec2(8.0, 12.0), Align2::LEFT_CENTER, &name, FontId::proportional(fs + 0.5), th.text);
            p.line_segment([b.left_top() + vec2(0.0, 24.0), b.right_top() + vec2(0.0, 24.0)], Stroke::new(1.0_f32, border.gamma_multiply(0.6)));
            p.text(b.left_top() + vec2(8.0, 36.0), Align2::LEFT_CENTER, format!("Start: {}", fmt_date_short(t.start)), FontId::proportional(fs - 1.0), th.text);
            p.text(b.right_top() + vec2(-8.0, 36.0), Align2::RIGHT_CENTER, format!("ID: {}", i + 1), FontId::proportional(fs - 1.0), th.text);
            p.text(b.left_top() + vec2(8.0, 52.0), Align2::LEFT_CENTER, format!("Finish: {}", fmt_date_short(t.finish)), FontId::proportional(fs - 1.0), th.text);
            p.text(b.right_top() + vec2(-8.0, 52.0), Align2::RIGHT_CENTER, format!("Dur: {}", t.duration.format(&pr.settings)), FontId::proportional(fs - 1.0), th.text);
            let res = pr.resource_names(t.uid);
            p.text(b.left_top() + vec2(8.0, 68.0), Align2::LEFT_CENTER, format!("Res: {}", res.chars().take(30).collect::<String>()), FontId::proportional(fs - 1.5), th.dim);
            let resp = ui.interact(b, Id::new(("nbox", t.uid)), Sense::click());
            if resp.clicked() {
                app.s.view.selected = [t.uid].into_iter().collect();
            }
            if resp.double_clicked() {
                runs.push(("task.information".into(), json!({"id": i + 1})));
            }
        }
    });
    for (c, p) in runs {
        app.run(&c, p);
    }
}

// ---------------------------------------------------------------------------------------------
// Calendar view
// ---------------------------------------------------------------------------------------------

pub fn calendar_view(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    let month = app.cal_month.unwrap_or_else(|| {
        let d = app.s.current_task().map(|i| app.s.project.tasks[i].start.date()).unwrap_or(app.s.project.start.date());
        NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
    });
    ui.horizontal(|ui| {
        if ui.button("<").clicked() {
            app.cal_month = Some(add_month(month, -1));
        }
        if ui.button("Today").clicked() {
            let t = chrono::Local::now().date_naive();
            app.cal_month = Some(NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap());
        }
        if ui.button(">").clicked() {
            app.cal_month = Some(add_month(month, 1));
        }
        ui.heading(month.format("%B %Y").to_string());
    });
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, th.bg);
    let first = month - Duration::days(month.weekday().num_days_from_sunday() as i64);
    let weeks = 6;
    let cw = rect.width() / 7.0;
    let hh = 22.0;
    let wh = (rect.height() - hh) / weeks as f32;
    for (k, d) in ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"].iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(k as f32 * cw, 0.0), vec2(cw, hh));
        p.rect_filled(r, 0.0, th.header);
        p.text(r.center(), Align2::CENTER_CENTER, *d, FontId::proportional(12.0), th.text);
    }
    let pr = &app.s.project;
    let cal = pr.cal();
    let leaves: Vec<usize> = (0..pr.tasks.len()).filter(|&i| !pr.tasks[i].summary && pr.tasks[i].active).collect();
    let mut runs: Vec<(String, Value)> = vec![];
    for w in 0..weeks {
        let wstart = first + Duration::days(w as i64 * 7);
        let wy = rect.top() + hh + w as f32 * wh;
        for dd in 0..7 {
            let date = wstart + Duration::days(dd);
            let r = Rect::from_min_size(pos2(rect.left() + dd as f32 * cw, wy), vec2(cw, wh));
            let fill = if date.month() != month.month() {
                th.panel
            } else if !cal.is_working_day(date) {
                th.nonwork
            } else {
                th.bg
            };
            p.rect_filled(r, 0.0, fill);
            p.rect_stroke(r, 0.0, Stroke::new(1.0_f32, th.grid), StrokeKind::Inside);
            p.text(r.right_top() + vec2(-6.0, 9.0), Align2::RIGHT_CENTER, date.day().to_string(), FontId::proportional(11.0), th.dim);
        }
        // bars, stacked
        let wend = wstart + Duration::days(7);
        let mut lane = 0;
        for &i in &leaves {
            let t = &pr.tasks[i];
            let (s, f) = (t.start.date(), if t.finish.time() == chrono::NaiveTime::MIN && t.finish > t.start { t.finish.date() - Duration::days(1) } else { t.finish.date() });
            if f < wstart || s >= wend {
                continue;
            }
            let ys = wy + 18.0 + lane as f32 * 18.0;
            if ys + 16.0 > wy + wh {
                p.text(pos2(rect.left() + 4.0, wy + wh - 8.0), Align2::LEFT_CENTER, "more...", FontId::proportional(10.0), th.dim);
                break;
            }
            let c0 = (s - wstart).num_days().max(0) as f32;
            let c1 = ((f - wstart).num_days().min(6) + 1) as f32;
            let br = Rect::from_min_max(pos2(rect.left() + c0 * cw + 3.0, ys), pos2(rect.left() + c1 * cw - 3.0, ys + 15.0));
            let col = if t.is_milestone() {
                Color32::from_rgb(60, 60, 60)
            } else if t.critical && app.s.view.show_critical {
                rgb(app.s.view.bar_colors.critical)
            } else {
                rgb(app.s.view.bar_colors.task)
            };
            let sel = app.s.view.selected.contains(&t.uid);
            p.rect_filled(br, 3.0, col);
            if sel {
                p.rect_stroke(br, 3.0, Stroke::new(2.0_f32, th.accent), StrokeKind::Outside);
            }
            let label = if t.is_milestone() { format!("◆ {}", t.name) } else { t.name.clone() };
            let txt_col = if t.is_milestone() { Color32::WHITE } else { Color32::from_rgb(20, 30, 50) };
            ui.painter().with_clip_rect(br).text(br.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), txt_col);
            let resp = ui.interact(br, Id::new(("calbar", t.uid, w)), Sense::click());
            if resp.clicked() {
                app.s.view.selected = [t.uid].into_iter().collect();
            }
            if resp.double_clicked() {
                runs.push(("task.information".into(), json!({"id": i + 1})));
            }
            lane += 1;
        }
    }
    for (c, p) in runs {
        app.run(&c, p);
    }
}

fn add_month(d: NaiveDate, m: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + m;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
}

// ---------------------------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------------------------

pub fn timeline_strip(app: &mut App, ui: &mut Ui, full_view: bool) {
    let th = theme(app.s.view.dark);
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, th.bg);
    let pr = &app.s.project;
    let start = pr.cal().snap_forward(pr.start);
    let finish = pr.finish.max(start + Duration::days(1));
    let pad = 70.0;
    let x0 = rect.left() + pad;
    let x1 = rect.right() - pad;
    let total = (finish - start).num_minutes().max(1) as f32;
    let xo = |t: chrono::NaiveDateTime| x0 + (x1 - x0) * ((t - start).num_minutes() as f32 / total).clamp(-0.05, 1.05);
    let band_top = if full_view { rect.top() + 60.0 } else { rect.top() + 20.0 };
    let band_h = if full_view { 60.0 } else { 26.0 };
    let band = Rect::from_min_max(pos2(x0, band_top), pos2(x1, band_top + band_h));
    p.rect_filled(band, 4.0, if app.s.view.dark { th.header } else { Color32::from_rgb(232, 238, 246) });
    p.text(pos2(rect.left() + 8.0, band.center().y), Align2::LEFT_CENTER, format!("Start\n{}", fmt_date_short(start)), FontId::proportional(11.0), th.dim);
    p.text(pos2(rect.right() - 8.0, band.center().y), Align2::RIGHT_CENTER, format!("Finish\n{}", fmt_date_short(finish)), FontId::proportional(11.0), th.dim);
    // the today label sits in the tick row; ticks leave room for it
    let today = chrono::Local::now().naive_local();
    let today_rect = (today > start && today < finish).then(|| Rect::from_center_size(pos2(xo(today), band.top() - 13.0), vec2(40.0, 14.0)));
    // month ticks, every 1, 2, 3, 6 or 12 months so their labels never touch
    let month_px = (x1 - x0) * (30.4 * 1440.0) / total;
    let tick_font = FontId::proportional(10.5);
    let step = [1u32, 2, 3, 6, 12, 24, 60].into_iter().find(|s| *s as f32 * month_px >= 64.0).unwrap_or(120);
    let mut m = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();
    if step >= 12 {
        m = NaiveDate::from_ymd_opt(start.year(), 1, 1).unwrap();
    } else {
        while (m.month() - 1) % step != 0 {
            m = add_month(m, -1);
        }
    }
    let mut guard = 0;
    let mut last_right = f32::MIN;
    while m.and_hms_opt(0, 0, 0).unwrap() <= finish && guard < 400 {
        let x = xo(m.and_hms_opt(0, 0, 0).unwrap());
        if x >= x0 {
            let label = if step >= 12 { m.format("%Y").to_string() } else { m.format("%b %Y").to_string() };
            let galley = p.layout_no_wrap(label, tick_font.clone(), th.dim);
            let lr = Rect::from_min_size(pos2(x + 3.0, rect.top() + 8.0 - galley.size().y / 2.0), galley.size());
            let clear = lr.left() > last_right + 6.0 && lr.right() < x1 + pad - 4.0 && today_rect.map(|t| !t.intersects(lr)).unwrap_or(true);
            if clear {
                last_right = lr.right();
                p.galley(lr.min, galley, th.dim);
            }
            p.line_segment([pos2(x, rect.top() + 2.0), pos2(x, band.bottom())], Stroke::new(1.0_f32, th.grid));
        }
        m = add_month(m, step as i32);
        guard += 1;
    }
    // top-level summaries (or tasks) as blocks
    let palette = [[120, 170, 220], [140, 200, 150], [240, 190, 120], [200, 150, 210], [130, 200, 200], [230, 150, 140]];
    // tasks added with "Add to Timeline"; with none, the top-level phases and every milestone
    let chosen = pr.tasks.iter().any(|t| t.on_timeline);
    let tops: Vec<usize> = (0..pr.tasks.len())
        .filter(|&i| {
            let t = &pr.tasks[i];
            if chosen {
                return t.on_timeline && !t.is_milestone();
            }
            t.active && (t.outline_level == 2 || (t.outline_level == 1 && !t.summary)) && !t.is_milestone()
        })
        .collect();
    let mut tips: Vec<(Rect, String)> = vec![];
    for (k, &i) in tops.iter().enumerate() {
        let t = &pr.tasks[i];
        let a = xo(t.start);
        let b = xo(t.finish).max(a + 3.0);
        let r = Rect::from_min_max(pos2(a, band.top() + 3.0), pos2(b, band.bottom() - 3.0));
        p.rect_filled(r, 3.0, rgb(palette[k % palette.len()]));
        // the name, shortened to fit; too narrow a block shows it on hover only
        let f = FontId::proportional(11.5);
        let name = super::gantt::fit_text(&p, &t.name, &f, r.width() - 10.0);
        if !name.is_empty() {
            p.with_clip_rect(r.intersect(rect)).text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, name, f, Color32::from_rgb(25, 30, 40));
        }
        tips.push((r, format!("{}\n{} - {}", t.name, fmt_date(t.start), fmt_date(t.finish))));
    }
    // milestones: a diamond each; labels go in up to two lanes and are left out where they
    // would overlap (hover a diamond for its name)
    let lanes: usize = if full_view { 3 } else { 1 };
    let mut lane_right = vec![f32::MIN; lanes];
    let mut ms: Vec<usize> = (0..pr.tasks.len()).filter(|&i| pr.tasks[i].is_milestone() && pr.tasks[i].active && (!chosen || pr.tasks[i].on_timeline)).collect();
    ms.sort_by_key(|&i| pr.tasks[i].start);
    let xs: Vec<f32> = ms.iter().map(|&i| xo(pr.tasks[i].start)).collect();
    for (k, &i) in ms.iter().enumerate() {
        let t = &pr.tasks[i];
        let x = xo(t.start);
        // the first lane's label must stop before the next diamond
        let next_diamond = xs.iter().skip(k + 1).copied().find(|nx| *nx > x + 1.0).unwrap_or(f32::MAX) - 9.0;
        let c = pos2(x, band.bottom() + 8.0);
        p.add(Shape::convex_polygon(vec![c + vec2(0.0, -6.0), c + vec2(6.0, 0.0), c + vec2(0.0, 6.0), c + vec2(-6.0, 0.0)], if app.s.view.dark { Color32::from_rgb(220, 220, 220) } else { Color32::from_rgb(40, 40, 40) }, Stroke::NONE));
        let label = format!("{} {}", t.name, fmt_date_short(t.start));
        tips.push((Rect::from_center_size(c, vec2(14.0, 14.0)), format!("{}\n{}", t.name, fmt_date(t.start))));
        let f = FontId::proportional(10.5);
        let galley = p.layout_no_wrap(label, f, th.text);
        for (lane, right) in lane_right.iter_mut().enumerate() {
            let y = c.y + lane as f32 * 15.0;
            let lr = Rect::from_min_size(pos2(c.x + 8.0, y - galley.size().y / 2.0), galley.size());
            let room = lane > 0 || lr.right() <= next_diamond;
            if room && lr.left() > *right + 8.0 && lr.bottom() <= rect.bottom() {
                if lane > 0 {
                    p.line_segment([pos2(c.x, c.y + 6.0), pos2(c.x, y)], Stroke::new(1.0_f32, th.grid));
                }
                *right = lr.right();
                p.galley(lr.min, galley, th.text);
                break;
            }
        }
    }
    for (r, tip) in tips {
        let resp = ui.interact(r, ui.id().with(("tl_tip", r.min.x as i32, r.min.y as i32)), Sense::hover());
        resp.on_hover_text(tip);
    }
    if today > start && today < finish {
        let x = xo(today);
        p.line_segment([pos2(x, band.top() - 4.0), pos2(x, band.bottom() + 4.0)], Stroke::new(2.0_f32, Color32::from_rgb(230, 140, 40)));
        p.text(pos2(x, band.top() - 8.0), Align2::CENTER_BOTTOM, "Today", FontId::proportional(10.0), Color32::from_rgb(200, 110, 20));
    }
    if !full_view {
        p.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, th.grid));
    } else {
        p.text(pos2(rect.left() + 10.0, rect.top() + 30.0), Align2::LEFT_CENTER, &pr.title, FontId::proportional(18.0), th.text);
    }
    let _ = chart_x;
}

// ---------------------------------------------------------------------------------------------
// Details pane (task form)
// ---------------------------------------------------------------------------------------------

pub fn details_pane(app: &mut App, ui: &mut Ui) {
    let th = theme(app.s.view.dark);
    let Some(i) = app.s.current_task() else {
        ui.label(RichText::new("Select a task to see its details.").color(th.dim));
        return;
    };
    let id = i + 1;
    let (tname, manual) = (app.s.project.tasks[i].name.clone(), app.s.project.tasks[i].manual);
    let mut edit = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}  {}", id, tname)).strong().size(15.0));
        ui.label(RichText::new(if manual { "Manually Scheduled" } else { "Auto Scheduled" }).color(th.dim));
        edit = ui.button("Edit...").clicked();
    });
    if edit {
        app.run("task.information", json!({"id": id}));
        return;
    }
    let pr = &app.s.project;
    let t = &pr.tasks[i];
    ui.horizontal_wrapped(|ui| {
        for (k, v) in [
            ("Duration", t.duration.format(&pr.settings)),
            ("Start", fmt_date(t.start)),
            ("Finish", fmt_date(t.finish)),
            ("% Complete", format!("{}%", fmt_num(t.percent_complete))),
            ("Type", t.task_type.title().to_string()),
            ("Constraint", t.constraint.title().to_string()),
            ("Total Slack", Dur { minutes: t.total_slack, unit: pr.settings.duration_unit, ..Default::default() }.format(&pr.settings)),
            ("Work", pr.fmt_work(t.work)),
            ("Cost", pr.fmt_cost(t.cost)),
        ] {
            ui.label(RichText::new(format!("{k}:")).color(th.dim));
            ui.label(v);
            ui.add_space(10.0);
        }
    });
    ui.separator();
    ui.columns(2, |cols| {
        cols[0].label(RichText::new("Resources").strong());
        egui::Grid::new("det_res").striped(true).show(&mut cols[0], |ui| {
            ui.label("Name");
            ui.label("Units");
            ui.label("Work");
            ui.end_row();
            for a in pr.assignments_of_task(t.uid) {
                ui.label(pr.resource_by_uid(a.resource).map(|r| r.name.clone()).unwrap_or_default());
                ui.label(format!("{}%", fmt_num(a.units * 100.0)));
                ui.label(pr.fmt_work(a.work));
                ui.end_row();
            }
        });
        cols[1].label(RichText::new("Predecessors").strong());
        egui::Grid::new("det_pred").striped(true).show(&mut cols[1], |ui| {
            ui.label("ID");
            ui.label("Name");
            ui.label("Type");
            ui.label("Lag");
            ui.end_row();
            for l in &t.predecessors {
                if let Some(pi) = pr.task_index(l.pred) {
                    ui.label((pi + 1).to_string());
                    ui.label(&pr.tasks[pi].name);
                    ui.label(l.kind.abbr());
                    ui.label(l.lag.format(&pr.settings));
                    ui.end_row();
                }
            }
        });
    });
}
