//! The task table with the Gantt chart (or usage grid) beside it.

use super::{rgb, theme, App, BarDrag, CellEdit, DragMode, Theme};
use crate::fields::{self, FieldDef};
use crate::model::*;
use crate::schedule;
use crate::session::*;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use egui::{pos2, vec2, Align2, Color32, CursorIcon, FontId, Id, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Gantt,
    Tracking,
    Sheet,
    Usage,
}

#[derive(Clone)]
enum DRow {
    Task(usize),
    Asg(usize),
    Group(String),
    Summary,
}

pub const HEADER_H: f32 = 40.0;

fn usage_columns() -> Vec<String> {
    ["id", "name", "work", "duration", "start", "finish"].iter().map(|s| s.to_string()).collect()
}

pub fn chart_x(app: &App, left: f32, t: NaiveDateTime) -> f32 {
    left + ((app.days_of(t) - app.left_days) * app.s.view.px_per_day as f64) as f32
}

fn col_widths(cols: &[&'static FieldDef], table_w: f32, custom: &std::collections::BTreeMap<String, f32>) -> Vec<f32> {
    let mut w: Vec<f32> = cols.iter().map(|c| custom.get(c.id).copied().unwrap_or(if c.id == "id" { 52.0 } else { c.width })).collect();
    let sum: f32 = w.iter().sum();
    if sum < table_w {
        if let Some(k) = cols.iter().position(|c| c.id == "name") {
            w[k] += table_w - sum;
        } else if let Some(last) = w.last_mut() {
            *last += table_w - sum;
        }
    }
    w
}

pub fn task_view(app: &mut App, ui: &mut Ui, mode: Mode) {
    let th = theme(app.s.view.dark);
    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    let has_chart = mode != Mode::Sheet;
    app.path_colors = task_path_colors(app);
    let table_w = if has_chart { app.s.view.table_width.clamp(120.0, (full.width() - 160.0).max(120.0)) } else { full.width() };
    let col_ids: Vec<String> = if mode == Mode::Usage { usage_columns() } else { app.s.view.columns.clone() };
    let cols: Vec<&'static FieldDef> = col_ids.iter().filter_map(|c| fields::task_field(c)).collect();
    let widths = col_widths(&cols, table_w, &app.s.view.col_widths);
    let chart_left = full.left() + table_w + 5.0;
    let chart_rect = Rect::from_min_max(pos2(chart_left, full.top()), full.max);

    // ---- headers ----
    let header = Rect::from_min_size(full.min, vec2(table_w, HEADER_H));
    {
        let p = ui.painter().with_clip_rect(header);
        p.rect_filled(header, 0.0, th.header);
        let mut x = header.left();
        for (k, c) in cols.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, header.top()), vec2(widths[k], HEADER_H));
            let sorted = app.s.view.sort.as_ref().filter(|(f, _)| f == c.id).map(|(_, a)| *a);
            let title = app.s.view.col_titles.get(c.id).cloned().unwrap_or_else(|| c.title.to_string());
            let txt = if c.id == "mode" && !app.s.view.col_titles.contains_key("mode") { "Mode".to_string() } else { title };
            if app.s.view.active_column == c.id && mode != Mode::Usage {
                p.rect_filled(r, 0.0, th.select);
            }
            let hfont = FontId::proportional(app.s.view.font_size - 0.5);
            let tr = if c.id == "indicators" && !app.s.view.col_titles.contains_key("indicators") {
                // the original's (i) symbol heads this column
                let cc = pos2(r.center().x, r.bottom() - 12.0);
                p.circle_stroke(cc, 7.0, Stroke::new(1.2_f32, Color32::from_rgb(40, 110, 190)));
                p.line_segment([cc + vec2(0.0, -1.0), cc + vec2(0.0, 4.0)], Stroke::new(1.5_f32, Color32::from_rgb(40, 110, 190)));
                p.circle_filled(cc + vec2(0.0, -3.6), 1.0, Color32::from_rgb(40, 110, 190));
                Rect::from_center_size(cc, vec2(14.0, 14.0))
            } else {
                let txt = fit_text(&p, &txt, &hfont, r.width() - (if sorted.is_some() { 26.0 } else { 9.0 }));
                p.with_clip_rect(r.intersect(header)).text(pos2(r.left() + 5.0, r.bottom() - 12.0), Align2::LEFT_CENTER, txt, hfont, th.text)
            };
            if let Some(asc) = sorted {
                let cpt = pos2(tr.right() + 8.0, tr.center().y);
                let tri = if asc { vec![cpt + vec2(-4.0, 2.5), cpt + vec2(4.0, 2.5), cpt + vec2(0.0, -3.0)] } else { vec![cpt + vec2(-4.0, -2.5), cpt + vec2(4.0, -2.5), cpt + vec2(0.0, 3.0)] };
                p.add(Shape::convex_polygon(tri, th.dim, Stroke::NONE));
            }
            p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0_f32, th.grid));
            let resp = ui.interact(r, Id::new(("colhdr", c.id)), Sense::click()).on_hover_text("Click to select the column (for alignment and column settings); double-click for Field Settings; right-click to sort or hide");
            if resp.double_clicked() && mode != Mode::Usage {
                app.run("view.column.title", json!({"field": c.id}));
            } else if resp.clicked() && mode != Mode::Usage {
                app.run("view.column.select", json!({"field": c.id}));
            }
            let cid = c.id;
            resp.context_menu(|ui| {
                if mode != Mode::Usage && ui.button(format!("Hide Column '{}'", c.title)).clicked() {
                    app.run(&format!("view.column.hide.{cid}"), json!({}));
                    ui.close_menu();
                }
                if ui.button("Insert Column...").clicked() {
                    app.run("view.columns.pick", json!({}));
                    ui.close_menu();
                }
                if SORT_FIELDS.contains(&cid) {
                    if ui.button("Sort Ascending").clicked() {
                        app.run(&format!("view.sort.{cid}.asc"), json!({}));
                        ui.close_menu();
                    }
                    if ui.button("Sort Descending").clicked() {
                        app.run(&format!("view.sort.{cid}.desc"), json!({}));
                        ui.close_menu();
                    }
                }
                if ui.button("Remove Sort").clicked() {
                    app.run("view.sort.none", json!({}));
                    ui.close_menu();
                }
            });
            x += widths[k];
        }
        p.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0_f32, th.grid));
    }
    if has_chart {
        let hdr = Rect::from_min_max(pos2(chart_left, full.top()), pos2(full.right(), full.top() + HEADER_H));
        if mode == Mode::Usage {
            usage_header(app, ui, hdr, &th);
        } else {
            timescale(app, ui, hdr, &th);
        }
        let resp = ui.interact(hdr, Id::new("tshdr"), Sense::drag()).on_hover_text("Drag to scroll the timescale; Ctrl+wheel zooms");
        if resp.dragged() {
            app.left_days -= resp.drag_delta().x as f64 / app.s.view.px_per_day as f64;
        }
        // splitter
        let split = Rect::from_min_max(pos2(full.left() + table_w, full.top()), pos2(chart_left, full.bottom()));
        ui.painter().rect_filled(split, 0.0, th.panel);
        let sr = ui.interact(split, Id::new("splitter"), Sense::drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);
        if sr.dragged() {
            app.s.view.table_width = (app.s.view.table_width + sr.drag_delta().x).clamp(120.0, full.width() - 160.0);
        }
        // wheel: horizontal pan and ctrl zoom over the chart
        let (hover, dx, zoom) = ui.input(|i| (i.pointer.hover_pos().map(|p| chart_rect.contains(p)).unwrap_or(false), i.smooth_scroll_delta.x, i.zoom_delta()));
        if hover {
            if dx != 0.0 {
                app.left_days -= dx as f64 / app.s.view.px_per_day as f64;
            }
            if zoom != 1.0 {
                let pp = (app.s.view.px_per_day * zoom).clamp(0.3, 400.0);
                app.s.view.px_per_day = pp;
                app.s.view.timescale = TimeUnit::ALL.into_iter().rev().find(|t| t.px_per_day() >= pp * 0.7).unwrap_or(TimeUnit::Hours);
            }
        }
    }

    // ---- rows ----
    let body = Rect::from_min_max(pos2(full.left(), full.top() + HEADER_H), full.max);
    let rows = display_rows(app, mode);
    let row_h = app.s.view.row_height;
    let blank = if mode == Mode::Usage { 0 } else { 6 };
    let n = rows.len() + blank;
    let scroll_to = app.scroll_to_row.take();
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body), |ui| {
        let mut sa = egui::ScrollArea::vertical().id_salt(("taskrows", mode as u8)).auto_shrink([false, false]);
        if let Some(uid) = scroll_to {
            if let Some(k) = rows.iter().position(|r| matches!(r, DRow::Task(i) if app.s.project.tasks[*i].uid == uid)) {
                sa = sa.vertical_scroll_offset((k as f32 * row_h - 3.0 * row_h).max(0.0));
            }
        }
        sa.show_rows(ui, row_h, n, |ui, range| {
            body_rows(app, ui, &rows, range, mode, &cols, &widths, table_w, chart_left, &th);
        });
    });
}

fn display_rows(app: &App, mode: Mode) -> Vec<DRow> {
    let mut out = vec![];
    for r in app.s.task_rows() {
        match r {
            Row::Task(i) => {
                out.push(DRow::Task(i));
                if mode == Mode::Usage {
                    let uid = app.s.project.tasks[i].uid;
                    for (ai, a) in app.s.project.assignments.iter().enumerate() {
                        if a.task == uid {
                            out.push(DRow::Asg(ai));
                        }
                    }
                }
            }
            Row::Group(g) => out.push(DRow::Group(g)),
            Row::ProjectSummary => out.push(DRow::Summary),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn body_rows(app: &mut App, ui: &mut Ui, rows: &[DRow], range: std::ops::Range<usize>, mode: Mode, cols: &[&'static FieldDef], widths: &[f32], table_w: f32, chart_left: f32, th: &Theme) {
    let row_h = app.s.view.row_height;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, range.len() as f32 * row_h), Sense::hover());
    let left = rect.left();
    let clip = ui.clip_rect();
    let table_clip = Rect::from_min_max(pos2(left, clip.top()), pos2(left + table_w, clip.bottom()));
    let chart_clip = Rect::from_min_max(pos2(chart_left, clip.top()), pos2(clip.right(), clip.bottom()));
    let y_of = |k: usize| rect.top() + (k as f32 - range.start as f32) * row_h;
    let has_chart = mode != Mode::Sheet;
    let ppd = app.s.view.px_per_day as f64;
    let fs = app.s.view.font_size;

    // chart background: nonworking days, gridlines, today/status lines
    if has_chart {
        let p = ui.painter().with_clip_rect(chart_clip);
        let cal = app.s.project.cal().clone();
        let d0 = app.left_days.floor() as i64;
        let d1 = (app.left_days + (clip.right() - chart_left) as f64 / ppd).ceil() as i64;
        if app.s.view.show_nonworking && ppd >= 2.0 && mode != Mode::Usage {
            for d in d0..=d1 {
                let date = app.date_of_days(d as f64).date();
                if !cal.is_working_day(date) {
                    let x0 = chart_x(app, chart_left, date.and_hms_opt(0, 0, 0).unwrap());
                    p.rect_filled(Rect::from_min_max(pos2(x0, rect.top()), pos2(x0 + ppd as f32, rect.bottom())), 0.0, th.nonwork);
                }
            }
        }
        if app.s.view.show_today && mode != Mode::Usage {
            let now = chrono::Local::now().naive_local();
            let x = chart_x(app, chart_left, now);
            p.extend(Shape::dashed_line(&[pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.2_f32, Color32::from_rgb(230, 140, 40)), 5.0, 3.0));
            if let Some(sd) = app.s.project.status_date {
                let x = chart_x(app, chart_left, sd);
                p.extend(Shape::dashed_line(&[pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(60, 120, 170)), 2.0, 3.0));
            }
        }
    }

    // row positions for link lines (all rows, not just visible)
    let mut row_of_uid: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (k, r) in rows.iter().enumerate() {
        if let DRow::Task(i) = r {
            row_of_uid.insert(app.s.project.tasks[*i].uid, k);
        }
    }

    let highlight = app.s.view.highlight;
    let mut runs: Vec<(String, Value)> = vec![];

    for k in range.clone() {
        let y = y_of(k);
        let trow = Rect::from_min_size(pos2(left, y), vec2(table_w, row_h));
        let crow = Rect::from_min_max(pos2(chart_left, y), pos2(rect.right(), y + row_h));
        let p = ui.painter().with_clip_rect(table_clip);
        let row = rows.get(k).cloned();
        let (uid, selected) = match &row {
            Some(DRow::Task(i)) => {
                let u = app.s.project.tasks[*i].uid;
                (Some(u), app.s.view.selected.contains(&u))
            }
            _ => (None, false),
        };
        // backgrounds
        let hl = match (&row, highlight) {
            (Some(DRow::Task(i)), Some(h)) => h.matches(&app.s.project, *i),
            _ => false,
        };
        let bg = if selected {
            th.select
        } else if hl {
            th.highlight
        } else if matches!(row, Some(DRow::Group(_))) {
            Color32::from_rgb(255, 236, 170).gamma_multiply(if app.s.view.dark { 0.35 } else { 1.0 })
        } else if matches!(row, Some(DRow::Asg(_))) {
            th.panel
        } else {
            th.bg
        };
        p.rect_filled(trow, 0.0, bg);
        if selected && has_chart {
            ui.painter().with_clip_rect(chart_clip).rect_filled(crow, 0.0, th.select.gamma_multiply(0.45));
        }
        // cells
        let mut x = left;
        for (ci, c) in cols.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(x, y), vec2(widths[ci], row_h));
            if c.id == "id" {
                p.rect_filled(cell, 0.0, if selected { th.select } else { th.header });
            }
            if app.s.view.show_gridlines {
                p.line_segment([cell.right_top(), cell.right_bottom()], Stroke::new(1.0_f32, th.grid));
            }
            draw_cell(app, &p, cell, &row, c, th, fs);
            x += widths[ci];
        }
        if app.s.view.show_gridlines {
            p.line_segment([trow.left_bottom(), trow.right_bottom()], Stroke::new(1.0_f32, th.grid));
        }

        // interaction: table row
        let resp = ui.interact(trow, Id::new(("trow", mode as u8, k)), Sense::click());
        if resp.clicked() || resp.secondary_clicked() {
            if let Some(u) = uid {
                if !(resp.secondary_clicked() && selected) {
                    select_row(app, rows, u, ui.input(|i| i.modifiers));
                }
            }
        }
        if resp.double_clicked() {
            let px = resp.interact_pointer_pos().map(|p| p.x).unwrap_or(left);
            let mut cx = left;
            let mut field = "name";
            for (ci, c) in cols.iter().enumerate() {
                if px >= cx && px < cx + widths[ci] {
                    field = c.id;
                }
                cx += widths[ci];
            }
            start_edit(app, &row, field);
        }
        if uid.is_some() || row.is_none() {
            resp.context_menu(|ui| row_menu(app, ui));
        }
        // summary expand/collapse triangle
        if let Some(DRow::Task(i)) = &row {
            let t = &app.s.project.tasks[*i];
            if t.summary {
                if let Some(nx) = name_cell_x(cols, widths, left) {
                    let indent = (t.outline_level as f32 - 1.0) * 14.0;
                    let tri = Rect::from_min_size(pos2(nx + 3.0 + indent, y + 2.0), vec2(14.0, row_h - 4.0));
                    let collapsed = t.collapsed;
                    let tu = t.uid;
                    let id = *i + 1;
                    let tr = ui.interact(tri, Id::new(("tri", tu)), Sense::click());
                    if tr.clicked() {
                        runs.push((if collapsed { "task.expand".into() } else { "task.collapse".into() }, json!({"id": id})));
                    }
                }
            }
        }
        // in-cell editor
        edit_cell(app, ui, &row, cols, widths, left, y, row_h, k, rows.len(), &mut runs);

        // chart part
        if has_chart {
            let cr = ui.interact(crow, Id::new(("crow", mode as u8, k)), Sense::click());
            if cr.clicked() {
                if let Some(u) = uid {
                    select_row(app, rows, u, ui.input(|i| i.modifiers));
                }
            }
            if cr.double_clicked() {
                if let Some(DRow::Task(i)) = &row {
                    runs.push(("task.information".into(), json!({"id": i + 1})));
                }
            }
            if uid.is_some() {
                cr.context_menu(|ui| row_menu(app, ui));
            }
            let cp = ui.painter().with_clip_rect(chart_clip);
            match (&row, mode) {
                (Some(DRow::Task(i)), Mode::Gantt | Mode::Tracking) => draw_bar(app, ui, &cp, *i, y, chart_left, mode, th, &mut runs, rows, &row_of_uid, rect.top(), range.start),
                (Some(r), Mode::Usage) => usage_cells(app, &cp, r, y, chart_left, crow.right(), th),
                (Some(DRow::Summary), _) => {
                    let cal = app.s.project.cal();
                    let s = cal.snap_forward(app.s.project.start);
                    let x0 = chart_x(app, chart_left, s);
                    let x1 = chart_x(app, chart_left, app.s.project.finish);
                    summary_bar(&cp, x0, x1, y, row_h, Color32::from_rgb(110, 110, 110));
                }
                _ => {}
            }
            if app.s.view.show_gridlines && mode == Mode::Usage {
                cp.line_segment([crow.left_bottom(), crow.right_bottom()], Stroke::new(1.0_f32, th.grid));
            }
        }
    }

    // link lines (drawn after bars, for every visible pair)
    if mode == Mode::Gantt || mode == Mode::Tracking {
        if app.s.view.show_links && app.s.view.link_style != "none" {
            let cp = ui.painter().with_clip_rect(chart_clip);
            for (k, r) in rows.iter().enumerate() {
                let DRow::Task(i) = r else { continue };
                let t = &app.s.project.tasks[*i];
                for l in &t.predecessors {
                    let Some(&pk) = row_of_uid.get(&l.pred) else { continue };
                    let lo = pk.min(k);
                    let hi = pk.max(k);
                    if hi < range.start || lo >= range.end {
                        continue;
                    }
                    let Some(pt) = app.s.project.task_by_uid(l.pred) else { continue };
                    let crit = app.s.view.show_critical && pt.critical && t.critical || mode == Mode::Tracking && pt.critical && t.critical;
                    let col = if crit { rgb(app.s.view.bar_colors.critical).gamma_multiply(1.2) } else if app.s.view.dark { Color32::from_rgb(140, 170, 220) } else { Color32::from_rgb(50, 80, 130) };
                    if app.s.view.link_style == "straight" {
                        let from_finish = matches!(l.kind, LinkKind::FS | LinkKind::FF);
                        let to_start = matches!(l.kind, LinkKind::FS | LinkKind::SS);
                        let a = pos2(chart_x(app, chart_left, if from_finish { pt.finish } else { pt.start }), y_of(pk) + row_h / 2.0);
                        let b = pos2(chart_x(app, chart_left, if to_start { t.start } else { t.finish }), y_of(k) + row_h / 2.0);
                        cp.line_segment([a, b], Stroke::new(1.0_f32, col));
                        let d = (b - a).normalized();
                        let n = vec2(-d.y, d.x);
                        cp.add(Shape::convex_polygon(vec![b, b - d * 6.0 + n * 3.5, b - d * 6.0 - n * 3.5], col, Stroke::NONE));
                    } else {
                        draw_link(app, &cp, pt, t, l.kind, y_of(pk), y_of(k), chart_left, row_h, col);
                    }
                }
            }
        }
        draw_drawings(app, ui, &row_of_uid, rect.top(), range.start, chart_left, chart_clip, &mut runs);
        // drag preview
        if let Some(d) = app.drag.clone() {
            if let Some(i) = app.s.project.task_index(d.uid) {
                if let Some(&k) = row_of_uid.get(&d.uid) {
                    let t = &app.s.project.tasks[i];
                    let y = y_of(k);
                    let x0 = chart_x(app, chart_left, t.start);
                    let x1 = chart_x(app, chart_left, t.finish).max(x0 + 2.0);
                    let cp = ui.painter().with_clip_rect(chart_clip);
                    let st = Stroke::new(1.5_f32, th.accent);
                    match d.mode {
                        DragMode::Move => {
                            cp.rect_stroke(Rect::from_min_max(pos2(x0 + d.dx, y + 4.0), pos2(x1 + d.dx, y + row_h - 4.0)), 2.0, st, StrokeKind::Middle);
                        }
                        DragMode::Resize => {
                            cp.rect_stroke(Rect::from_min_max(pos2(x0, y + 4.0), pos2((x1 + d.dx).max(x0), y + row_h - 4.0)), 2.0, st, StrokeKind::Middle);
                        }
                        DragMode::Link => {
                            if let Some(pp) = ui.input(|i| i.pointer.hover_pos()) {
                                cp.line_segment([pos2(x1, y + row_h / 2.0), pp], st);
                            }
                        }
                        DragMode::Undecided => {}
                    }
                }
            }
        }
    }
    for (c, p) in runs {
        app.run(&c, p);
    }
}

fn name_cell_x(cols: &[&'static FieldDef], widths: &[f32], left: f32) -> Option<f32> {
    let mut x = left;
    for (k, c) in cols.iter().enumerate() {
        if c.id == "name" {
            return Some(x);
        }
        x += widths[k];
    }
    None
}

fn select_row(app: &mut App, rows: &[DRow], uid: u32, m: egui::Modifiers) {
    app.edit = None;
    // Format Painter: the next row clicked takes the picked-up formatting
    if app.painter_armed {
        app.painter_armed = false;
        if let Some(i) = app.s.project.task_index(uid) {
            app.run("format.painter.apply", json!({"ids": [i + 1]}));
        }
    }
    if m.command {
        if !app.s.view.selected.remove(&uid) {
            app.s.view.selected.insert(uid);
        }
        app.anchor_row = Some(uid);
    } else if m.shift && app.anchor_row.is_some() {
        let order: Vec<u32> = rows.iter().filter_map(|r| if let DRow::Task(i) = r { Some(app.s.project.tasks[*i].uid) } else { None }).collect();
        let a = order.iter().position(|u| Some(*u) == app.anchor_row);
        let b = order.iter().position(|u| *u == uid);
        if let (Some(a), Some(b)) = (a, b) {
            let (lo, hi) = (a.min(b), a.max(b));
            app.s.view.selected = order[lo..=hi].iter().copied().collect();
        }
    } else {
        app.s.view.selected = [uid].into_iter().collect();
        app.anchor_row = Some(uid);
    }
}

fn start_edit(app: &mut App, row: &Option<DRow>, field: &str) {
    match row {
        Some(DRow::Task(i)) => {
            let fd = fields::task_field(field);
            match fd {
                Some(fd) if fd.editable => {
                    let t = &app.s.project.tasks[*i];
                    let mut buf = fields::task_text(&app.s.project, *i, field);
                    if fd.kind == fields::FieldKind::Date {
                        buf = match field {
                            "start" => fmt_datetime(t.start),
                            "finish" => fmt_datetime(t.finish),
                            _ => if buf == "NA" { String::new() } else { fields::value_as_text(&fields::task_value(&app.s.project, *i, field)) },
                        };
                    }
                    if field == "notes" {
                        buf = t.notes.clone();
                    }
                    app.edit = Some(CellEdit { uid: Some(t.uid), field: field.to_string(), buf, focused: false, resource: false });
                }
                _ => {
                    let id = *i + 1;
                    app.run("task.information", json!({"id": id}));
                }
            }
        }
        None => {
            let f = if fields::task_field(field).map(|f| f.editable).unwrap_or(false) { field } else { "name" };
            app.edit = Some(CellEdit { uid: None, field: f.to_string(), buf: String::new(), focused: false, resource: false });
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn edit_cell(app: &mut App, ui: &mut Ui, row: &Option<DRow>, cols: &[&'static FieldDef], widths: &[f32], left: f32, y: f32, row_h: f32, k: usize, nrows: usize, runs: &mut Vec<(String, Value)>) {
    let Some(e) = app.edit.clone() else { return };
    if e.resource {
        return;
    }
    let this = match row {
        Some(DRow::Task(i)) => e.uid == Some(app.s.project.tasks[*i].uid),
        None => e.uid.is_none() && k == nrows,
        _ => false,
    };
    if !this {
        return;
    }
    let mut x = left;
    let mut cell = None;
    for (ci, c) in cols.iter().enumerate() {
        if c.id == e.field {
            cell = Some(Rect::from_min_size(pos2(x, y), vec2(widths[ci], row_h)));
        }
        x += widths[ci];
    }
    let Some(cell) = cell else {
        app.edit = None;
        return;
    };
    let edit = app.edit.as_mut().unwrap();
    let resp = ui.put(cell.shrink(1.0), egui::TextEdit::singleline(&mut edit.buf).margin(vec2(3.0, 1.0)));
    if !edit.focused {
        resp.request_focus();
        edit.focused = true;
    }
    let esc = ui.input(|i| i.key_pressed(Key::Escape));
    if esc {
        app.edit = None;
        return;
    }
    if resp.lost_focus() {
        let e = app.edit.take().unwrap();
        let value = e.buf.trim().to_string();
        match e.uid {
            Some(uid) => {
                if let Some(i) = app.s.project.task_index(uid) {
                    if fields::task_text(&app.s.project, i, &e.field) != value {
                        runs.push((format!("task.set.{}", e.field), json!({"uid": uid, "value": value})));
                    }
                }
            }
            None => {
                if !value.is_empty() {
                    let at = app.s.project.tasks.len() + 1;
                    let mut p = json!({"at": at});
                    match e.field.as_str() {
                        "name" => p["name"] = json!(value),
                        "duration" => p["duration"] = json!(value),
                        "predecessors" => p["predecessors"] = json!(value),
                        "resourceNames" => p["resources"] = json!(value),
                        _ => p["name"] = json!(value),
                    }
                    runs.push(("task.insert".into(), p));
                }
            }
        }
    }
}

/// The text style a task row is drawn with: the "all" text style, then its kind's (critical or
/// not, milestone, summary, manual, inactive), then its own Font formatting.
pub fn task_style(app: &App, i: usize) -> TextStyle {
    let t = &app.s.project.tasks[i];
    let ts = &app.s.view.text_styles;
    let mut st = ts.get("all").cloned().unwrap_or_default();
    let kinds = [
        if t.critical && !t.summary { "critical" } else { "noncritical" },
        if t.is_milestone() { "milestone" } else { "" },
        if t.summary { "summary" } else { "" },
        if t.manual { "manual" } else { "" },
        if !t.active { "inactive" } else { "" },
    ];
    for k in kinds {
        if let Some(s) = ts.get(k) {
            st = st.merged(s);
        }
    }
    if let Some(s) = &t.style {
        st = st.merged(s);
    }
    st
}

/// Draw text with a style: size, font, italic and underline from the layout, bold by drawing
/// twice half a pixel apart. Returns where it was drawn.
pub fn styled_text(p: &egui::Painter, pos: Pos2, align: Align2, text: &str, st: &TextStyle, base_size: f32, color: Color32) -> Rect {
    let size = st.size.unwrap_or(base_size);
    let family = if st.font.as_deref() == Some("mono") { egui::FontFamily::Monospace } else { egui::FontFamily::Proportional };
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(size, family),
            color,
            italics: st.italic.unwrap_or(false),
            underline: if st.underline.unwrap_or(false) { Stroke::new(1.0_f32, color) } else { Stroke::NONE },
            ..Default::default()
        },
    );
    let galley = p.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    if st.bold.unwrap_or(false) {
        p.galley(rect.min + vec2(0.6, 0.0), galley.clone(), color);
    }
    p.galley(rect.min, galley, color);
    rect
}

/// Text cut to a width, ending in "..." when it doesn't fit.
pub fn fit_text(p: &egui::Painter, text: &str, font: &FontId, width: f32) -> String {
    let w = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if width <= 0.0 {
        return String::new();
    }
    if w(text) <= width {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    // binary search the longest prefix that fits with the ellipsis
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let s: String = chars[..mid].iter().collect::<String>() + "...";
        if w(&s) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        return String::new();
    }
    chars[..lo].iter().collect::<String>().trim_end().to_string() + "..."
}

fn draw_cell(app: &App, p: &egui::Painter, cell: Rect, row: &Option<DRow>, c: &FieldDef, th: &Theme, fs: f32) {
    // nothing a cell draws may spill into its neighbours
    let clipped = p.with_clip_rect(cell.shrink2(vec2(1.0, 0.0)).intersect(p.clip_rect()));
    let p = &clipped;
    let font = FontId::proportional(fs);
    let pr = &app.s.project;
    let cy = cell.center().y;
    match row {
        Some(DRow::Task(i)) => {
            let t = &pr.tasks[*i];
            // Text Styles for this kind of row, then the row's own Font formatting
            let st = task_style(app, *i);
            let base_color = if !t.active { th.dim } else if t.summary { th.summary_text } else { th.text };
            let color = st.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(base_color);
            if let Some(bg) = st.background {
                if !matches!(c.id, "id" | "indicators") {
                    p.rect_filled(cell, 0.0, Color32::from_rgb(bg[0], bg[1], bg[2]));
                }
            }
            let fs = st.size.unwrap_or(fs);
            let font = if st.font.as_deref() == Some("mono") { FontId::monospace(fs) } else { FontId::proportional(fs) };
            match c.id {
                "id" => {
                    if app.s.view.columns.iter().any(|c| c == "indicators") {
                        p.text(pos2(cell.left() + 4.0, cy), Align2::LEFT_CENTER, (i + 1).to_string(), font.clone(), th.dim);
                    } else {
                        // older views without an Indicators column: icons on the right, the
                        // number cut short before them
                        let left = indicators(app, p, cell, *i, th, false);
                        let idp = p.with_clip_rect(Rect::from_min_max(cell.min, pos2(left, cell.bottom())).intersect(p.clip_rect()));
                        idp.text(pos2(cell.left() + 4.0, cy), Align2::LEFT_CENTER, (i + 1).to_string(), font.clone(), th.dim);
                    }
                }
                "indicators" => {
                    indicators(app, p, cell, *i, th, true);
                }
                "mode" => mode_icon(p, cell, t.manual, th),
                "name" => {
                    let indent = (t.outline_level as f32 - 1.0) * 14.0;
                    let mut x = cell.left() + 4.0 + indent;
                    if t.summary {
                        let c0 = pos2(x + 6.0, cy);
                        let tri = if t.collapsed {
                            vec![c0 + vec2(-3.0, -4.0), c0 + vec2(3.0, 0.0), c0 + vec2(-3.0, 4.0)]
                        } else {
                            vec![c0 + vec2(-4.0, -2.5), c0 + vec2(4.0, -2.5), c0 + vec2(0.0, 3.0)]
                        };
                        p.add(Shape::convex_polygon(tri, th.dim, Stroke::NONE));
                    }
                    x += 16.0;
                    let mut name = t.name.clone();
                    if app.s.view.show_outline_numbers {
                        name = format!("{} {}", pr.outline_number(*i), name);
                    }
                    let name = fit_text(p, &name, &font, cell.right() - x - 4.0);
                    // summary tasks are bold unless a style says otherwise
                    let mut ns = st.clone();
                    if ns.bold.is_none() && t.summary {
                        ns.bold = Some(true);
                    }
                    let r = styled_text(p, pos2(x, cy), Align2::LEFT_CENTER, &name, &ns, fs, color);
                    if !t.active {
                        p.line_segment([pos2(r.left(), cy), pos2(r.right(), cy)], Stroke::new(1.0_f32, th.dim));
                    }
                }
                f => {
                    let text = fields::task_text(pr, *i, f);
                    let num = matches!(c.kind, fields::FieldKind::Cost | fields::FieldKind::Work | fields::FieldKind::Number | fields::FieldKind::Percent);
                    let late_col = if f == "finish" && t.missed_deadline { th.danger } else { color };
                    let align = app.s.view.col_align.get(f).map(|s| s.as_str()).unwrap_or(if num { "right" } else { "left" });
                    let cp = p.with_clip_rect(cell.intersect(p.clip_rect()));
                    let text = fit_text(p, &text, &font, cell.width() - 9.0);
                    match align {
                        "right" => styled_text(&cp, pos2(cell.right() - 5.0, cy), Align2::RIGHT_CENTER, &text, &st, fs, late_col),
                        "center" => styled_text(&cp, pos2(cell.center().x, cy), Align2::CENTER_CENTER, &text, &st, fs, late_col),
                        _ => styled_text(&cp, pos2(cell.left() + 5.0, cy), Align2::LEFT_CENTER, &text, &st, fs, late_col),
                    };
                }
            }
        }
        Some(DRow::Asg(ai)) => {
            let a = &pr.assignments[*ai];
            let rname = pr.resource_by_uid(a.resource).map(|r| r.name.clone()).unwrap_or_default();
            let text = match c.id {
                "name" => {
                    let lvl = pr.task_by_uid(a.task).map(|t| t.outline_level).unwrap_or(1);
                    let x = cell.left() + 24.0 + lvl as f32 * 14.0;
                    let f = FontId::proportional(fs - 0.5);
                    let rname = fit_text(p, &rname, &f, cell.right() - x - 4.0);
                    p.text(pos2(x, cy), Align2::LEFT_CENTER, rname, f, th.accent);
                    return;
                }
                "work" => pr.fmt_work(a.work),
                "start" => pr.task_by_uid(a.task).map(|t| fmt_date(t.start)).unwrap_or_default(),
                "finish" => pr.task_by_uid(a.task).map(|t| fmt_date(t.finish)).unwrap_or_default(),
                _ => String::new(),
            };
            let f = FontId::proportional(fs - 0.5);
            let text = fit_text(p, &text, &f, cell.width() - 9.0);
            p.text(pos2(cell.left() + 5.0, cy), Align2::LEFT_CENTER, text, f, th.dim);
        }
        Some(DRow::Group(g)) => {
            if c.id == "name" {
                p.text(pos2(cell.left() + 6.0, cy), Align2::LEFT_CENTER, g, font.clone(), th.text);
                p.text(pos2(cell.left() + 6.6, cy), Align2::LEFT_CENTER, g, font, th.text);
            }
        }
        Some(DRow::Summary) => {
            let cal = pr.cal();
            let s = cal.snap_forward(pr.start);
            let d = Dur { minutes: cal.work_between(s, pr.finish), unit: pr.settings.duration_unit, ..Default::default() };
            let text = match c.id {
                "id" => "0".to_string(),
                "name" => pr.title.clone(),
                "duration" => d.format(&pr.settings),
                "start" => fmt_date(s),
                "finish" => fmt_date(pr.finish),
                "work" => pr.fmt_work(pr.tasks.iter().filter(|t| !t.summary).map(|t| t.work).sum()),
                "cost" => pr.fmt_cost(pr.tasks.iter().filter(|t| !t.summary).map(|t| t.cost).sum()),
                _ => String::new(),
            };
            let text = fit_text(p, &text, &font, cell.width() - 10.0);
            p.text(pos2(cell.left() + 5.0, cy), Align2::LEFT_CENTER, &text, font.clone(), th.summary_text);
            p.text(pos2(cell.left() + 5.6, cy), Align2::LEFT_CENTER, &text, font, th.summary_text);
        }
        None => {}
    }
}

/// Draw a task's indicator icons: from the left of their own column, or right to left at the
/// end of the ID cell. Returns the left edge of what was drawn.
fn indicators(app: &App, p: &egui::Painter, cell: Rect, i: usize, th: &Theme, from_left: bool) -> f32 {
    let t = &app.s.project.tasks[i];
    let cy = cell.center().y;
    let count = [t.percent_complete >= 100.0 && !t.summary, !t.notes.is_empty(), t.missed_deadline || t.total_slack < 0 || t.constraint.needs_date(), !t.active].iter().filter(|b| **b).count();
    if count == 0 {
        return cell.right();
    }
    // icons are placed right to left at 12 px steps; in their own column the row starts at the left
    let mut x = if from_left { (cell.left() + 8.0 + (count as f32 - 1.0) * 12.0).min(cell.right() - 7.0) } else { cell.right() - 10.0 };
    let left_edge = x - (count as f32 - 1.0) * 12.0 - 6.0;
    if t.percent_complete >= 100.0 && !t.summary {
        let g = Color32::from_rgb(40, 150, 70);
        p.line_segment([pos2(x - 4.0, cy), pos2(x - 1.0, cy + 3.5)], Stroke::new(2.0_f32, g));
        p.line_segment([pos2(x - 1.0, cy + 3.5), pos2(x + 4.0, cy - 4.0)], Stroke::new(2.0_f32, g));
        x -= 12.0;
    }
    if !t.notes.is_empty() {
        let r = Rect::from_center_size(pos2(x, cy), vec2(8.0, 10.0));
        p.rect_filled(r, 1.0, Color32::from_rgb(255, 250, 220));
        p.rect_stroke(r, 1.0, Stroke::new(1.0_f32, th.dim), StrokeKind::Inside);
        for k in 0..3 {
            let yy = r.top() + 3.0 + k as f32 * 2.2;
            p.line_segment([pos2(r.left() + 1.5, yy), pos2(r.right() - 1.5, yy)], Stroke::new(0.8_f32, th.dim));
        }
        x -= 12.0;
    }
    if t.missed_deadline || t.total_slack < 0 {
        p.circle_filled(pos2(x, cy), 5.0, th.danger);
        p.text(pos2(x, cy), Align2::CENTER_CENTER, "!", FontId::proportional(9.0), Color32::WHITE);
        x -= 12.0;
    } else if t.constraint.needs_date() {
        let r = Rect::from_center_size(pos2(x, cy), vec2(9.0, 9.0));
        p.rect_stroke(r, 1.0, Stroke::new(1.0_f32, Color32::from_rgb(60, 110, 170)), StrokeKind::Inside);
        p.line_segment([r.left_top() + vec2(0.0, 3.0), r.right_top() + vec2(0.0, 3.0)], Stroke::new(1.0_f32, Color32::from_rgb(60, 110, 170)));
        x -= 12.0;
    }
    if !t.active {
        p.line_segment([pos2(x - 4.0, cy), pos2(x + 4.0, cy)], Stroke::new(1.5_f32, th.dim));
    }
    left_edge
}

fn mode_icon(p: &egui::Painter, cell: Rect, manual: bool, th: &Theme) {
    let c = cell.center();
    if manual {
        let col = Color32::from_rgb(40, 150, 150);
        p.circle_filled(c + vec2(2.0, -3.0), 3.5, col);
        p.line_segment([c + vec2(0.0, -1.0), c + vec2(-4.0, 5.0)], Stroke::new(1.5_f32, col));
    } else {
        let col = Color32::from_rgb(70, 110, 170);
        p.rect_filled(Rect::from_min_size(c + vec2(-8.0, -3.0), vec2(10.0, 6.0)), 1.0, col);
        p.add(Shape::convex_polygon(vec![c + vec2(3.0, -5.0), c + vec2(8.0, 0.0), c + vec2(3.0, 5.0)], col, Stroke::NONE));
    }
    let _ = th;
}

fn row_menu(app: &mut App, ui: &mut Ui) {
    let items: &[(&str, &str)] = &[
        ("Cut", "edit.cut"),
        ("Copy", "edit.copy"),
        ("Paste", "edit.paste"),
        ("-", ""),
        ("Insert Task", "task.insert"),
        ("Insert Milestone", "task.insertMilestone"),
        ("Delete Task", "edit.delete"),
        ("-", ""),
        ("Indent", "task.indent"),
        ("Outdent", "task.outdent"),
        ("Link Tasks", "task.link"),
        ("Unlink Tasks", "task.unlink"),
        ("-", ""),
        ("Task Information...", "task.information"),
        ("Notes...", "task.notes"),
        ("Scroll to Task", "task.scrollTo"),
        ("-", ""),
        ("Mark 100% Complete", "task.markComplete.100"),
        ("Manually Schedule", "task.manualSchedule"),
        ("Auto Schedule", "task.autoSchedule"),
        ("Inactivate Task", "task.inactivate"),
        ("Activate Task", "task.activate"),
        ("Split Task", "task.split"),
    ];
    for (label, cmd) in items {
        if *label == "-" {
            ui.separator();
            continue;
        }
        if ui.button(*label).clicked() {
            ui.close_menu();
            app.run(cmd, json!({}));
        }
    }
}

fn summary_bar(p: &egui::Painter, x0: f32, x1: f32, y: f32, row_h: f32, col: Color32) {
    let mid = y + row_h / 2.0;
    let x1 = x1.max(x0 + 2.0);
    p.rect_filled(Rect::from_min_max(pos2(x0, mid - 4.0), pos2(x1, mid + 1.0)), 0.0, col);
    p.add(Shape::convex_polygon(vec![pos2(x0, mid - 4.0), pos2(x0 + 6.0, mid - 4.0), pos2(x0, mid + 5.0)], col, Stroke::NONE));
    p.add(Shape::convex_polygon(vec![pos2(x1 - 6.0, mid - 4.0), pos2(x1, mid - 4.0), pos2(x1, mid + 5.0)], col, Stroke::NONE));
}

fn diamond(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)], col, Stroke::NONE));
}

/// Shapes from Format > Drawing: drawn on the chart, dragged to move, right-click to delete.
#[allow(clippy::too_many_arguments)]
fn draw_drawings(app: &mut App, ui: &mut Ui, row_of_uid: &std::collections::HashMap<u32, usize>, rect_top: f32, range_start: usize, chart_left: f32, clip: Rect, runs: &mut Vec<(String, Value)>) {
    let row_h = app.s.view.row_height;
    let ppd = app.s.view.px_per_day;
    let drawings = app.s.view.drawings.clone();
    for d in drawings {
        let base_row = match d.task {
            Some(u) => match row_of_uid.get(&u) {
                Some(&k) => k as f32,
                None => continue,
            },
            None => 0.0,
        };
        let y = rect_top + (base_row + d.row - range_start as f32) * row_h;
        let mut x0 = chart_x(app, chart_left, d.date);
        let mut y = y;
        if let Some((id, off)) = app.drawing_drag {
            if id == d.id {
                x0 += off.x;
                y += off.y;
            }
        }
        let x1 = x0 + d.width_days * ppd;
        let h = (d.height_rows.max(0.0) * row_h).max(row_h * 0.8);
        let col = rgb(d.color);
        let p = ui.painter().with_clip_rect(clip);
        let area = match d.kind.as_str() {
            "line" | "arrow" => {
                let a = pos2(x0, y + row_h / 2.0);
                let b = pos2(x1, y + row_h / 2.0);
                p.line_segment([a, b], Stroke::new(2.0_f32, col));
                if d.kind == "arrow" {
                    p.add(Shape::convex_polygon(vec![b + vec2(2.0, 0.0), b + vec2(-7.0, -5.0), b + vec2(-7.0, 5.0)], col, Stroke::NONE));
                }
                if !d.text.is_empty() {
                    p.text(pos2((x0 + x1) / 2.0, y + row_h / 2.0 - 3.0), Align2::CENTER_BOTTOM, &d.text, FontId::proportional(app.s.view.font_size - 1.0), col);
                }
                Rect::from_min_max(pos2(x0, y), pos2(x1, y + row_h))
            }
            "box" => {
                let r = Rect::from_min_size(pos2(x0, y + 1.0), vec2((x1 - x0).max(10.0), h - 2.0));
                p.rect_stroke(r, 2.0, Stroke::new(2.0_f32, col), StrokeKind::Inside);
                if !d.text.is_empty() {
                    p.text(r.center(), Align2::CENTER_CENTER, &d.text, FontId::proportional(app.s.view.font_size - 1.0), col);
                }
                r
            }
            _ => {
                let galley = p.layout_no_wrap(d.text.clone(), FontId::proportional(app.s.view.font_size - 0.5), col);
                let r = Rect::from_min_size(pos2(x0, y + 1.0), vec2(galley.size().x + 10.0, h - 2.0));
                p.rect_filled(r, 2.0, Color32::from_rgb(255, 250, 215));
                p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(190, 170, 90)), StrokeKind::Inside);
                p.galley(pos2(r.left() + 5.0, r.center().y - galley.size().y / 2.0), galley, col);
                r
            }
        };
        let resp = ui.interact(area.expand(3.0), Id::new(("drawing", d.id)), Sense::click_and_drag()).on_hover_text("Drawing: drag to move, double-click to change its words, right-click to delete");
        if resp.dragged() {
            let off = app.drawing_drag.filter(|(id, _)| *id == d.id).map(|(_, o)| o).unwrap_or_default() + resp.drag_delta();
            app.drawing_drag = Some((d.id, off));
        }
        if resp.drag_stopped() {
            if let Some((id, off)) = app.drawing_drag.take() {
                runs.push(("format.drawing.move".into(), json!({"drawing": id, "days": off.x / ppd, "rows": (off.y / row_h).round()})));
            }
        }
        if resp.double_clicked() {
            app.open_palette_with("format.drawing.setText");
            app.palette.params = serde_json::to_string_pretty(&json!({"drawing": d.id, "value": d.text})).unwrap_or_default();
        }
        let id = d.id;
        resp.context_menu(|ui| {
            if ui.button("Change Text...").clicked() {
                ui.close_menu();
                app.open_palette_with("format.drawing.setText");
                app.palette.params = serde_json::to_string_pretty(&json!({"drawing": id, "value": ""})).unwrap_or_default();
            }
            if ui.button("Delete Drawing").clicked() {
                ui.close_menu();
                app.run("format.drawing.delete", json!({"drawing": id}));
            }
        });
    }
}

pub fn bar_height(app: &App) -> f32 {
    let f = match app.s.view.bar_size.as_str() {
        "small" => 0.30,
        "large" => 0.58,
        _ => 0.42,
    };
    (app.s.view.row_height * f).max(6.0)
}

/// Task Path: colour for each task related to the selected one (stronger colours for the
/// driving chain).
pub fn task_path_colors(app: &App) -> std::collections::HashMap<u32, Color32> {
    let mut out = std::collections::HashMap::new();
    let modes = &app.s.view.task_path;
    let Some(sel) = app.s.current_task() else { return out };
    if modes.is_empty() {
        return out;
    }
    let p = &app.s.project;
    let drives = |pred: &Task, t: &Task, kind: LinkKind| -> bool {
        let cal = p.task_cal(t);
        match kind {
            LinkKind::FS => cal.snap_forward(pred.finish) >= t.start || pred.finish >= t.start,
            LinkKind::SS => pred.start >= t.start,
            LinkKind::FF => pred.finish >= t.finish,
            LinkKind::SF => pred.start >= t.finish,
        }
    };
    let walk_back = |driving: bool| -> Vec<u32> {
        let mut seen = vec![];
        let mut stack = vec![p.tasks[sel].uid];
        while let Some(u) = stack.pop() {
            let Some(t) = p.task_by_uid(u) else { continue };
            for l in &t.predecessors {
                if let Some(pt) = p.task_by_uid(l.pred) {
                    if (!driving || drives(pt, t, l.kind)) && !seen.contains(&l.pred) {
                        seen.push(l.pred);
                        stack.push(l.pred);
                    }
                }
            }
        }
        seen
    };
    let walk_fwd = |driving: bool| -> Vec<u32> {
        let mut seen = vec![];
        let mut stack = vec![p.tasks[sel].uid];
        while let Some(u) = stack.pop() {
            let Some(src) = p.task_by_uid(u) else { continue };
            for t in &p.tasks {
                for l in t.predecessors.iter().filter(|l| l.pred == u) {
                    if (!driving || drives(src, t, l.kind)) && !seen.contains(&t.uid) {
                        seen.push(t.uid);
                        stack.push(t.uid);
                    }
                }
            }
        }
        seen
    };
    let has = |m: &str| modes.iter().any(|x| x == m);
    if has("predecessors") {
        for u in walk_back(false) {
            out.insert(u, Color32::from_rgb(250, 200, 120));
        }
    }
    if has("drivingPredecessors") {
        for u in walk_back(true) {
            out.insert(u, Color32::from_rgb(232, 120, 30));
        }
    }
    if has("successors") {
        for u in walk_fwd(false) {
            out.insert(u, Color32::from_rgb(205, 175, 235));
        }
    }
    if has("drivenSuccessors") {
        for u in walk_fwd(true) {
            out.insert(u, Color32::from_rgb(130, 70, 190));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(
    app: &mut App,
    ui: &mut Ui,
    p: &egui::Painter,
    i: usize,
    y: f32,
    left: f32,
    mode: Mode,
    th: &Theme,
    runs: &mut Vec<(String, Value)>,
    rows: &[DRow],
    row_of_uid: &std::collections::HashMap<u32, usize>,
    rect_top: f32,
    range_start: usize,
) {
    let row_h = app.s.view.row_height;
    let bc = app.s.view.bar_colors.clone();
    let tracking = mode == Mode::Tracking;
    let pr = &app.s.project;
    let t = pr.tasks[i].clone();
    // Task Information > Hide Bar
    if t.hide_bar {
        return;
    }
    let x0 = chart_x(app, left, t.start);
    let x1 = chart_x(app, left, t.finish).max(x0 + if t.is_milestone() { 0.0 } else { 2.0 });
    let mid = y + row_h / 2.0;
    let show_base = app.s.view.show_baseline || tracking;
    let bh = bar_height(app);
    let (top, bottom) = if show_base { (y + row_h * 0.16, y + row_h * 0.16 + bh) } else { (mid - bh / 2.0, mid + bh / 2.0) };
    let crit = (app.s.view.show_critical || tracking) && t.critical;
    let fs = app.s.view.font_size - 1.0;
    let text_col = th.text;
    let path_col = app.path_colors.get(&t.uid).copied();

    // slippage: a thin line from the baseline start to where the task starts now
    if let Some(n) = app.s.view.slippage {
        if let Some(bs) = t.baseline(n).and_then(|b| b.start) {
            let bx = chart_x(app, left, bs);
            if (bx - x0).abs() > 1.0 {
                let yy = (top + bottom) / 2.0;
                p.line_segment([pos2(bx, yy), pos2(x0, yy)], Stroke::new(1.2_f32, Color32::from_rgb(90, 90, 90)));
                p.line_segment([pos2(bx, yy - 3.0), pos2(bx, yy + 3.0)], Stroke::new(1.2_f32, Color32::from_rgb(90, 90, 90)));
            }
        }
    }
    // baseline
    if show_base {
        if let Some(b) = t.baseline(app.s.view.baseline_shown) {
            if let (Some(bs), Some(bf)) = (b.start, b.finish) {
                let bx0 = chart_x(app, left, bs);
                let bx1 = chart_x(app, left, bf).max(bx0 + 2.0);
                let col = rgb(bc.baseline);
                if t.is_milestone() && !t.summary {
                    diamond(p, pos2(bx0, bottom + 4.0), 4.0, col);
                } else {
                    p.rect_filled(Rect::from_min_max(pos2(bx0, bottom + 2.0), pos2(bx1, bottom + 6.0)), 0.0, col);
                }
            }
        }
    }
    let bar_rect;
    if t.summary {
        let col = if crit && tracking { rgb(bc.critical) } else { rgb(bc.summary) };
        if show_base {
            p.rect_filled(Rect::from_min_max(pos2(x0, top + 1.0), pos2(x1, top + 6.0)), 0.0, col);
            p.add(Shape::convex_polygon(vec![pos2(x0, top + 1.0), pos2(x0 + 5.0, top + 1.0), pos2(x0, top + 9.0)], col, Stroke::NONE));
            p.add(Shape::convex_polygon(vec![pos2(x1 - 5.0, top + 1.0), pos2(x1, top + 1.0), pos2(x1, top + 9.0)], col, Stroke::NONE));
        } else {
            summary_bar(p, x0, x1, y, row_h, col);
        }
        if tracking || (app.s.view.show_progress && t.percent_complete > 0.0 && !show_base) {
            let px = x0 + (x1 - x0) * (t.percent_complete as f32 / 100.0);
            if t.percent_complete > 0.0 {
                p.rect_filled(Rect::from_min_max(pos2(x0, mid - 1.0), pos2(px, mid + 0.5)), 0.0, rgb(bc.progress));
            }
        }
        if tracking {
            p.text(pos2(x1 + 6.0, mid), Align2::LEFT_CENTER, format!("{}%", fmt_num(t.percent_complete)), FontId::proportional(fs), text_col);
        }
        bar_rect = Rect::from_min_max(pos2(x0, top), pos2(x1, bottom));
    } else if t.is_milestone() {
        let c = pos2(x0, (top + bottom) / 2.0);
        let col = if crit { rgb(bc.critical) } else { rgb(bc.milestone) };
        let col = if app.s.view.dark && !crit { Color32::from_rgb(220, 220, 220) } else { col };
        let col = path_col.unwrap_or(col);
        diamond(p, c, bh * 0.62, col);
        if app.s.view.show_bar_text && app.s.view.bar_date_format != "none" {
            let label = if app.s.view.bar_date_format == "long" { fmt_date(t.start) } else { fmt_date_short(t.start) };
            p.text(pos2(x0 + bh * 0.7 + 4.0, c.y), Align2::LEFT_CENTER, label, FontId::proportional(fs), text_col);
        }
        bar_rect = Rect::from_center_size(c, vec2(bh * 1.4, bh * 1.4));
    } else {
        let base = path_col.or(t.bar_color.map(rgb)).unwrap_or(if crit {
            rgb(bc.critical)
        } else if t.manual {
            rgb(bc.manual)
        } else {
            rgb(bc.task)
        });
        let r = Rect::from_min_max(pos2(x0, top), pos2(x1, bottom));
        if t.active {
            p.rect_filled(r, 2.0, base);
            let border = Color32::from_rgb((base.r() as f32 * 0.6) as u8, (base.g() as f32 * 0.6) as u8, (base.b() as f32 * 0.6) as u8);
            p.rect_stroke(r, 2.0, Stroke::new(0.8_f32, border), StrokeKind::Inside);
        } else {
            p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, th.dim), StrokeKind::Inside);
        }
        if t.manual {
            let col = Color32::from_rgb(30, 120, 120);
            p.line_segment([pos2(x0, top - 1.0), pos2(x0, bottom + 1.0)], Stroke::new(2.0_f32, col));
            p.line_segment([pos2(x1, top - 1.0), pos2(x1, bottom + 1.0)], Stroke::new(2.0_f32, col));
        }
        if t.percent_complete > 0.0 && (app.s.view.show_progress || tracking) {
            let px = x0 + (x1 - x0) * (t.percent_complete as f32 / 100.0);
            let pc = rgb(bc.progress);
            if tracking {
                p.rect_filled(Rect::from_min_max(pos2(x0, top), pos2(px, bottom)), 2.0, if crit { Color32::from_rgb(170, 40, 40) } else { pc });
            } else {
                let h = (bottom - top) / 3.0;
                p.rect_filled(Rect::from_min_max(pos2(x0, top + h), pos2(px, bottom - h)), 0.0, pc);
            }
        }
        if app.s.view.show_late && t.late {
            p.rect_stroke(r.expand(1.0), 2.0, Stroke::new(1.6_f32, Color32::from_rgb(230, 130, 30)), StrokeKind::Outside);
        }
        if app.s.view.show_bar_text {
            let label = if tracking { format!("{}%", fmt_num(t.percent_complete)) } else { pr.resource_names(t.uid) };
            if !label.is_empty() {
                p.text(pos2(x1 + 5.0, (top + bottom) / 2.0), Align2::LEFT_CENTER, label, FontId::proportional(fs), text_col);
            }
        }
        bar_rect = r;
    }
    // slack
    if app.s.view.show_slack && !t.summary && t.late_finish > t.finish {
        let sx = chart_x(app, left, t.late_finish);
        let yy = bottom + 2.5;
        p.line_segment([pos2(x1, yy), pos2(sx, yy)], Stroke::new(2.0_f32, Color32::from_rgb(40, 150, 150)));
    }
    // deadline marker
    if let Some(dl) = t.deadline {
        let dx = chart_x(app, left, dl);
        let col = if t.missed_deadline { th.danger } else { Color32::from_rgb(40, 150, 70) };
        p.add(Shape::convex_polygon(vec![pos2(dx - 5.0, y + 2.0), pos2(dx + 5.0, y + 2.0), pos2(dx, y + 9.0)], col, Stroke::NONE));
        p.line_segment([pos2(dx, y + 9.0), pos2(dx, y + row_h - 3.0)], Stroke::new(1.0_f32, col));
    }

    // interaction
    let resp = ui.interact(bar_rect.expand2(vec2(3.0, 2.0)), Id::new(("bar", t.uid)), Sense::click_and_drag());
    let tip = format!(
        "{}\nStart: {}\nFinish: {}\nDuration: {}\n{}% complete{}",
        t.name,
        fmt_date(t.start),
        fmt_date(t.finish),
        t.duration.format(&pr.settings),
        fmt_num(t.percent_complete),
        if t.summary { "" } else { "\n\nDrag to move, drag the right end to change the duration, drag up or down onto another bar to link" }
    );
    let resp = resp.on_hover_text(tip);
    let near_right = resp.hover_pos().map(|p| p.x > bar_rect.right() - 6.0).unwrap_or(false);
    if near_right && !t.summary && !t.is_milestone() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    if resp.clicked() {
        select_row(app, rows, t.uid, ui.input(|i| i.modifiers));
    }
    if resp.double_clicked() {
        runs.push(("task.information".into(), json!({"id": i + 1})));
    }
    if resp.drag_started() && !t.summary {
        let nr = resp.interact_pointer_pos().map(|p| p.x > bar_rect.right() - 6.0).unwrap_or(false);
        app.drag = Some(BarDrag { uid: t.uid, mode: DragMode::Undecided, dx: 0.0, dy: 0.0, near_right: nr && !t.is_milestone() });
        app.s.view.selected = [t.uid].into_iter().collect();
    }
    if resp.dragged() {
        if let Some(d) = app.drag.as_mut().filter(|d| d.uid == t.uid) {
            let dd = resp.drag_delta();
            d.dx += dd.x;
            d.dy += dd.y;
            if d.mode == DragMode::Undecided && (d.dx.abs() > 4.0 || d.dy.abs() > 4.0) {
                d.mode = if d.dy.abs() > d.dx.abs() * 1.2 && d.dy.abs() > 6.0 {
                    DragMode::Link
                } else if d.near_right {
                    DragMode::Resize
                } else {
                    DragMode::Move
                };
            }
        }
    }
    if resp.drag_stopped() {
        if let Some(d) = app.drag.take().filter(|d| d.uid == t.uid) {
            let ppd = app.s.view.px_per_day as f64;
            let cal = app.s.project.task_cal(&t).clone();
            match d.mode {
                DragMode::Move => {
                    let nd = app.days_of(t.start) + d.dx as f64 / ppd;
                    let when = app.date_of_days(nd);
                    let snapped = if ppd >= 120.0 { cal.snap_forward(round_hour(when)) } else { cal.day_start(when.date()).unwrap_or_else(|| cal.snap_forward(when)) };
                    runs.push(("task.set.start".into(), json!({"id": i + 1, "value": fmt_datetime(snapped)})));
                }
                DragMode::Resize => {
                    let nf = app.date_of_days(app.days_of(t.finish) + d.dx as f64 / ppd);
                    let nf = if ppd >= 120.0 { round_hour(nf) } else { cal.day_end(nf.date()).unwrap_or(nf) };
                    let minutes = cal.work_between(t.start, nf).max(0);
                    let mpd = app.s.project.settings.minutes_per_day as f64;
                    let days = if ppd >= 120.0 { (minutes as f64 / 60.0).round() / (mpd / 60.0) } else { (minutes as f64 / mpd).round() };
                    runs.push(("task.set.duration".into(), json!({"id": i + 1, "value": format!("{}d", fmt_num(days.max(0.0)))})));
                }
                DragMode::Link => {
                    if let Some(pp) = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos())) {
                        let k = ((pp.y - rect_top) / row_h).floor() as i64 + range_start as i64;
                        if k >= 0 {
                            if let Some(DRow::Task(j)) = rows.get(k as usize) {
                                if *j != i {
                                    runs.push(("task.link".into(), json!({"from": i + 1, "to": j + 1})));
                                }
                            }
                        }
                    }
                }
                DragMode::Undecided => {}
            }
        }
    }
    let _ = row_of_uid;
}

fn round_hour(t: NaiveDateTime) -> NaiveDateTime {
    let base = t.date().and_hms_opt(t.hour(), 0, 0).unwrap();
    if t.minute() >= 30 {
        base + Duration::hours(1)
    } else {
        base
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_link(app: &App, p: &egui::Painter, pt: &Task, t: &Task, kind: LinkKind, yp: f32, ys: f32, left: f32, row_h: f32, col: Color32) {
    let st = Stroke::new(1.0_f32, col);
    let from_finish = matches!(kind, LinkKind::FS | LinkKind::FF);
    let to_start = matches!(kind, LinkKind::FS | LinkKind::SS);
    let sx = if from_finish { chart_x(app, left, pt.finish) } else { chart_x(app, left, pt.start) };
    let tx = if to_start { chart_x(app, left, t.start) } else { chart_x(app, left, t.finish) };
    let pm = yp + row_h / 2.0;
    let tm = ys + row_h / 2.0;
    let down = ys > yp;
    let bar_half = bar_height(app) / 2.0;
    let arrow = |p: &egui::Painter, tip: Pos2, dir: Pos2| {
        let d = (dir - tip).normalized();
        let n = vec2(-d.y, d.x);
        p.add(Shape::convex_polygon(vec![tip, tip + d * 5.0 + n * 3.5, tip + d * 5.0 - n * 3.5], col, Stroke::NONE));
    };
    if to_start {
        let ms = if t.is_milestone() { 6.0 } else { 0.0 };
        if tx - 2.0 >= sx + 4.0 {
            let ax = tx + 3.0 + ms * 0.0;
            let ay = if down { tm - bar_half - if ms > 0.0 { 4.0 } else { 0.0 } } else { tm + bar_half + if ms > 0.0 { 4.0 } else { 0.0 } };
            p.line_segment([pos2(sx, pm), pos2(ax, pm)], st);
            p.line_segment([pos2(ax, pm), pos2(ax, ay)], st);
            arrow(p, pos2(ax, ay), pos2(ax, if down { ay - 10.0 } else { ay + 10.0 }));
        } else {
            let ex = sx + 6.0;
            let gy = if down { ys } else { ys + row_h };
            let bx = tx - 8.0 - ms;
            p.line_segment([pos2(sx, pm), pos2(ex, pm)], st);
            p.line_segment([pos2(ex, pm), pos2(ex, gy)], st);
            p.line_segment([pos2(ex, gy), pos2(bx, gy)], st);
            p.line_segment([pos2(bx, gy), pos2(bx, tm)], st);
            p.line_segment([pos2(bx, tm), pos2(tx - ms, tm)], st);
            arrow(p, pos2(tx - ms, tm), pos2(tx - ms - 10.0, tm));
        }
    } else {
        let ex = sx.max(tx) + 8.0;
        p.line_segment([pos2(sx, pm), pos2(ex, pm)], st);
        p.line_segment([pos2(ex, pm), pos2(ex, tm)], st);
        p.line_segment([pos2(ex, tm), pos2(tx, tm)], st);
        arrow(p, pos2(tx, tm), pos2(tx + 10.0, tm));
    }
}

// ---------------------------------------------------------------------------------------------
// Timescale
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Tier {
    Hour(u32),
    Day,
    Week,
    Month,
    Quarter,
    Half,
    Year,
}

fn floor_tier(t: NaiveDateTime, tier: Tier) -> NaiveDateTime {
    let d = t.date();
    let midnight = |d: NaiveDate| d.and_hms_opt(0, 0, 0).unwrap();
    match tier {
        Tier::Hour(step) => d.and_hms_opt(t.hour() / step * step, 0, 0).unwrap(),
        Tier::Day => midnight(d),
        Tier::Week => midnight(d - Duration::days(d.weekday().num_days_from_sunday() as i64)),
        Tier::Month => midnight(NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()),
        Tier::Quarter => midnight(NaiveDate::from_ymd_opt(d.year(), (d.month() - 1) / 3 * 3 + 1, 1).unwrap()),
        Tier::Half => midnight(NaiveDate::from_ymd_opt(d.year(), if d.month() > 6 { 7 } else { 1 }, 1).unwrap()),
        Tier::Year => midnight(NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap()),
    }
}

fn add_months(d: NaiveDate, m: u32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + m as i32;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
}

fn next_tier(t: NaiveDateTime, tier: Tier) -> NaiveDateTime {
    match tier {
        Tier::Hour(step) => t + Duration::hours(step as i64),
        Tier::Day => t + Duration::days(1),
        Tier::Week => t + Duration::days(7),
        Tier::Month => add_months(t.date(), 1).and_hms_opt(0, 0, 0).unwrap(),
        Tier::Quarter => add_months(t.date(), 3).and_hms_opt(0, 0, 0).unwrap(),
        Tier::Half => add_months(t.date(), 6).and_hms_opt(0, 0, 0).unwrap(),
        Tier::Year => add_months(t.date(), 12).and_hms_opt(0, 0, 0).unwrap(),
    }
}

fn tier_label(t: NaiveDateTime, tier: Tier, top: bool, width: f32) -> String {
    let d = t.date();
    match tier {
        Tier::Hour(_) => {
            let h = t.hour();
            let (hh, ap) = if h == 0 { (12, "AM") } else if h < 12 { (h, "AM") } else if h == 12 { (12, "PM") } else { (h - 12, "PM") };
            if width > 34.0 { format!("{hh} {ap}") } else { format!("{hh}") }
        }
        Tier::Day => {
            if top {
                d.format("%a %b %-d, '%y").to_string()
            } else if width >= 24.0 {
                let day = d.format("%a").to_string()[..1].to_string();
                if width > 40.0 {
                    format!("{day} {}", d.day())
                } else {
                    day
                }
            } else {
                d.format("%a").to_string()[..1].to_string()
            }
        }
        Tier::Week => {
            if top || width > 70.0 {
                d.format("%b %-d, '%y").to_string()
            } else {
                d.format("%-m/%-d").to_string()
            }
        }
        Tier::Month => {
            if top || width > 90.0 {
                d.format("%B %Y").to_string()
            } else if width > 26.0 {
                d.format("%b").to_string()
            } else {
                d.format("%b").to_string()[..1].to_string()
            }
        }
        Tier::Quarter => {
            let q = (d.month() - 1) / 3 + 1;
            if top || width > 80.0 {
                format!("Qtr {q}, {}", d.year())
            } else {
                format!("Q{q}")
            }
        }
        Tier::Half => format!("H{}, {}", if d.month() > 6 { 2 } else { 1 }, d.year()),
        Tier::Year => d.year().to_string(),
    }
}

fn tiers(unit: TimeUnit, ppd: f64) -> (Tier, Tier) {
    match unit {
        TimeUnit::Hours => (Tier::Day, Tier::Hour(if ppd >= 480.0 { 1 } else if ppd >= 240.0 { 2 } else { 4 })),
        TimeUnit::Days => (Tier::Week, Tier::Day),
        TimeUnit::Weeks => (Tier::Month, Tier::Week),
        TimeUnit::Months => (Tier::Quarter, Tier::Month),
        TimeUnit::Quarters => (Tier::Year, Tier::Quarter),
        TimeUnit::Years => (Tier::Year, Tier::Half),
    }
}

pub fn timescale(app: &App, ui: &Ui, rect: Rect, th: &Theme) {
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, th.header);
    let ppd = app.s.view.px_per_day as f64;
    let (top, bottom) = tiers(app.s.view.timescale, ppd);
    let left_t = app.date_of_days(app.left_days);
    let right_t = app.date_of_days(app.left_days + rect.width() as f64 / ppd);
    let fs = app.s.view.font_size - 1.0;
    for (tier, y0, y1, is_top) in [(top, rect.top(), rect.top() + HEADER_H / 2.0, true), (bottom, rect.top() + HEADER_H / 2.0, rect.bottom(), false)] {
        let mut t = floor_tier(left_t, tier);
        let mut guard = 0;
        while t < right_t && guard < 2000 {
            let n = next_tier(t, tier);
            let xa = chart_x(app, rect.left(), t);
            let xb = chart_x(app, rect.left(), n);
            p.line_segment([pos2(xa, y0), pos2(xa, y1)], Stroke::new(1.0_f32, th.grid));
            let label = tier_label(t, tier, is_top, xb - xa);
            let lx = if is_top { xa.max(rect.left()) + 4.0 } else { (xa + xb) / 2.0 };
            let align = if is_top { Align2::LEFT_CENTER } else { Align2::CENTER_CENTER };
            if xb - xa > 8.0 {
                let cell = Rect::from_min_max(pos2(xa.max(rect.left()), y0), pos2(xb.min(rect.right()), y1));
                p.with_clip_rect(cell.intersect(rect)).text(pos2(lx, (y0 + y1) / 2.0), align, label, FontId::proportional(fs), th.text);
            }
            t = n;
            guard += 1;
        }
        p.line_segment([pos2(rect.left(), y1), pos2(rect.right(), y1)], Stroke::new(1.0_f32, th.grid));
    }
}

// ---------------------------------------------------------------------------------------------
// Usage grid (Task Usage)
// ---------------------------------------------------------------------------------------------

fn bucket_days(app: &App) -> i64 {
    if app.s.view.px_per_day >= 18.0 {
        1
    } else {
        7
    }
}

fn bucket_start(app: &App, d: NaiveDate) -> NaiveDate {
    if bucket_days(app) == 7 {
        d - Duration::days(d.weekday().num_days_from_sunday() as i64)
    } else {
        d
    }
}

fn usage_header(app: &App, ui: &Ui, rect: Rect, th: &Theme) {
    timescale(app, ui, rect, th);
}

pub fn for_buckets(app: &App, left: f32, right: f32, mut f: impl FnMut(NaiveDateTime, NaiveDateTime, f32, f32)) {
    let ppd = app.s.view.px_per_day as f64;
    let bd = bucket_days(app);
    let start = bucket_start(app, app.date_of_days(app.left_days).date());
    let mut d = start;
    let mut guard = 0;
    loop {
        let a = d.and_hms_opt(0, 0, 0).unwrap();
        let b = a + Duration::days(bd);
        let xa = chart_x(app, left, a);
        let xb = chart_x(app, left, b);
        if xa > right || guard > 3000 {
            break;
        }
        f(a, b, xa, xb);
        d += Duration::days(bd);
        guard += 1;
    }
    let _ = ppd;
}

fn usage_cells(app: &App, p: &egui::Painter, row: &DRow, y: f32, left: f32, right: f32, th: &Theme) {
    let pr = &app.s.project;
    let row_h = app.s.view.row_height;
    let fs = app.s.view.font_size - 1.5;
    let asgs: Vec<&Assignment> = match row {
        DRow::Task(i) => pr.assignments_of_task(pr.tasks[*i].uid).collect(),
        DRow::Asg(ai) => vec![&pr.assignments[*ai]],
        _ => return,
    };
    let summary_ids: Vec<u32> = match row {
        DRow::Task(i) if pr.tasks[*i].summary => pr.descendants(*i).map(|j| pr.tasks[j].uid).collect(),
        _ => vec![],
    };
    let sum_asg: Vec<&Assignment> = pr.assignments.iter().filter(|a| summary_ids.contains(&a.task)).collect();
    for_buckets(app, left, right, |a, b, xa, xb| {
        let mut w = 0i64;
        for asg in asgs.iter().chain(sum_asg.iter()) {
            if pr.resource_by_uid(asg.resource).map(|r| r.kind == ResourceKind::Work).unwrap_or(false) {
                w += schedule::assignment_work_in(pr, asg, a, b);
            }
        }
        p.line_segment([pos2(xa, y), pos2(xa, y + row_h)], Stroke::new(1.0_f32, th.grid));
        if w > 0 && xb - xa > 17.0 {
            let txt = format!("{}h", fmt_num(w as f64 / 60.0));
            p.text(pos2(xb - 3.0, y + row_h / 2.0), Align2::RIGHT_CENTER, txt, FontId::proportional(fs), if matches!(row, DRow::Asg(_)) { th.dim } else { th.text });
        }
    });
}
