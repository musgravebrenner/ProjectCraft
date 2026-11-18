//! Reports drawn as documents: a title block, summary cards, and a formatted table with a
//! shaded header, striped rows and numbers aligned right. Project Overview is a dashboard.

use super::{theme, App, Theme};
use crate::model::fmt_date;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use serde_json::Value;

/// "baselineCost" -> "Baseline Cost".
pub fn friendly(key: &str) -> String {
    let mut out = String::new();
    for (i, ch) in key.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// A summary value as people read it: money with the plan's currency, percentages, dates.
pub fn friendly_value(app: &App, key: &str, v: &Value) -> String {
    let k = key.to_lowercase();
    match v {
        Value::Null => "None".into(),
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(0.0);
            let x = if x == 0.0 { 0.0 } else { x }; // no "-0"
            if k.contains("cost") || k.contains("amount") {
                app.s.project.fmt_cost(x)
            } else if k.contains("percent") {
                format!("{}%", crate::model::fmt_num(x.round()))
            } else if k.contains("hours") {
                format!("{} hrs", group_thousands(x))
            } else {
                group_thousands(x)
            }
        }
        Value::String(s) => match chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
            Ok(d) => fmt_date(d),
            Err(_) => s.clone(),
        },
        Value::Bool(b) => if *b { "Yes" } else { "No" }.into(),
        Value::Array(a) => format!("{} items", a.len()),
        Value::Object(_) => String::new(),
    }
}

fn group_thousands(x: f64) -> String {
    let neg = x < 0.0;
    let s = crate::model::fmt_num((x.abs() * 100.0).round() / 100.0);
    let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_string(), format!(".{b}"))).unwrap_or((s.clone(), String::new()));
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{out}{frac}", if neg { "-" } else { "" })
}

/// Whether a column holds numbers (to align it right).
fn numeric_column(cells: &[String]) -> bool {
    let filled: Vec<&String> = cells.iter().filter(|c| !c.trim().is_empty()).collect();
    !filled.is_empty()
        && filled.iter().all(|c| {
            let t = c.trim().trim_start_matches(['$', '-', '(', '€', '£']).trim_end_matches(['%', ')']).replace(',', "");
            let first = t.split_whitespace().next().unwrap_or("");
            first.parse::<f64>().is_ok() && (t.split_whitespace().count() <= 2)
        })
}

fn card(ui: &mut Ui, th: &Theme, label: &str, value: &str, accent: Color32, width: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(width, 64.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(6), th.panel);
    p.rect_filled(Rect::from_min_size(r.min, vec2(4.0, r.height())), CornerRadius { nw: 6, sw: 6, ne: 0, se: 0 }, accent);
    p.text(r.min + vec2(14.0, 12.0), Align2::LEFT_TOP, label, FontId::proportional(12.0), th.dim);
    let f = FontId::proportional(19.0);
    let v = super::gantt::fit_text(p, value, &f, width - 22.0);
    p.text(r.min + vec2(14.0, 30.0), Align2::LEFT_TOP, v, f, th.text);
}

fn header(app: &App, ui: &mut Ui, th: &Theme, title: &str, v: &Value) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 66.0), Sense::hover());
    let p = ui.painter_at(r);
    p.text(r.left_top(), Align2::LEFT_TOP, title, FontId::proportional(24.0), th.accent);
    let sub = format!("{}   ·   {}", app.s.project.title, chrono::Local::now().format("%B %-d, %Y"));
    let f = FontId::proportional(13.5);
    let sub = super::gantt::fit_text(&p, &sub, &f, w - 120.0);
    p.text(r.left_top() + vec2(0.0, 36.0), Align2::LEFT_TOP, sub, f, th.dim);
    p.line_segment([pos2(r.left(), r.top() + 60.0), pos2(r.right(), r.top() + 60.0)], Stroke::new(2.0_f32, th.accent));
    let br = Rect::from_min_size(pos2(r.right() - 88.0, r.top() + 8.0), vec2(88.0, 28.0));
    if ui.put(br, egui::Button::new("Copy").corner_radius(CornerRadius::same(4))).on_hover_text("Copy the report as Markdown text").clicked() {
        ui.ctx().copy_text(crate::reports::to_markdown(v));
    }
    ui.add_space(14.0);
}

pub fn show(app: &App, ui: &mut Ui, title: &str, v: &Value) {
    let th = theme(app.s.view.dark);
    header(app, ui, &th, title, v);
    if title == "Project Overview" {
        overview(app, ui, &th, &v["summary"]);
        return;
    }
    // summary cards
    if let Some(sum) = v["summary"].as_object().filter(|s| !s.is_empty()) {
        let n = sum.len().min(4) as f32;
        let w = ((ui.available_width() - (n - 1.0) * 10.0) / n).clamp(120.0, 220.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for (k, val) in sum.iter().take(4) {
                card(ui, &th, &friendly(k), &friendly_value(app, k, val), th.accent, w);
            }
        });
        ui.add_space(12.0);
    }
    table(app, ui, &th, title, v);
}

fn overview(app: &App, ui: &mut Ui, th: &Theme, o: &Value) {
    let get = |k: &str| friendly_value(app, k, &o[k]);
    let pct = o["percentComplete"].as_f64().unwrap_or(0.0);
    let cols = 4.0;
    let w = ((ui.available_width() - (cols - 1.0) * 10.0) / cols).max(120.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        card(ui, th, "Start", &get("start"), Color32::from_rgb(70, 130, 200), w);
        card(ui, th, "Finish", &get("finish"), Color32::from_rgb(70, 130, 200), w);
        card(ui, th, "Duration", &get("duration"), Color32::from_rgb(120, 110, 200), w);
        card(ui, th, "Cost", &get("cost"), Color32::from_rgb(220, 150, 60), w);
    });
    ui.add_space(14.0);
    // progress
    ui.label(RichText::new("Progress").strong().size(15.0));
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(11), th.panel);
    let done = Rect::from_min_size(r.min, vec2(r.width() * (pct / 100.0).clamp(0.0, 1.0) as f32, r.height()));
    if done.width() > 0.5 {
        p.rect_filled(done, CornerRadius::same(11), th.accent);
    }
    p.text(r.center(), Align2::CENTER_CENTER, format!("{}% complete", crate::model::fmt_num(pct.round())), FontId::proportional(12.5), if pct > 55.0 { Color32::WHITE } else { th.text });
    ui.add_space(16.0);
    // three groups of figures
    let groups: [(&str, &[(&str, &str)]); 3] = [
        ("Tasks", &[("tasks", "Tasks"), ("summaryTasks", "Summary tasks"), ("milestones", "Milestones"), ("criticalTasks", "Critical tasks"), ("lateTasks", "Late tasks")]),
        ("Resources", &[("resources", "Resources"), ("overallocatedResources", "Overallocated"), ("workHours", "Work"), ("calendar", "Calendar")]),
        ("Cost and dates", &[("cost", "Cost"), ("baselineCost", "Baseline cost"), ("statusDate", "Status date"), ("title", "Plan")]),
    ];
    let gw = ((ui.available_width() - 20.0) / 3.0).max(160.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for (g, rows) in groups {
            egui::Frame::new().fill(th.bg).stroke(Stroke::new(1.0_f32, th.grid)).corner_radius(CornerRadius::same(6)).inner_margin(egui::Margin::same(12)).show(ui, |ui| ui.vertical(|ui| {
                ui.set_width(gw - 26.0);
                ui.label(RichText::new(g).strong().size(14.5));
                ui.add_space(6.0);
                // fixed-width rows: label on the left, value on the right
                let row_w = gw - 26.0;
                let f = egui::TextStyle::Body.resolve(ui.style());
                for (k, label) in rows.iter() {
                    let val = get(k);
                    let warn = matches!(*k, "lateTasks" | "overallocatedResources") && o[*k].as_f64().unwrap_or(0.0) > 0.0;
                    let (r, resp) = ui.allocate_exact_size(vec2(row_w, 24.0), Sense::hover());
                    let p = ui.painter_at(r);
                    p.text(r.left_center(), Align2::LEFT_CENTER, *label, f.clone(), th.dim);
                    let lw = p.layout_no_wrap(label.to_string(), f.clone(), th.dim).size().x;
                    let shown = super::gantt::fit_text(&p, &val, &f, row_w - lw - 14.0);
                    p.text(r.right_center(), Align2::RIGHT_CENTER, shown, f.clone(), if warn { th.danger } else { th.text });
                    if val.len() > 20 {
                        resp.on_hover_text(val.clone());
                    }
                }
            }));
        }
    });
    if let Some(w) = o["warnings"].as_array().filter(|w| !w.is_empty()) {
        ui.add_space(14.0);
        ui.label(RichText::new(format!("Warnings ({})", w.len())).strong().color(th.danger));
        for x in w.iter().take(12) {
            ui.label(RichText::new(format!("•  {}", x.as_str().unwrap_or(""))).color(th.dim));
        }
    }
}

fn table(app: &App, ui: &mut Ui, th: &Theme, title: &str, v: &Value) {
    let cols: Vec<String> = v["columns"].as_array().map(|a| a.iter().map(crate::fields::value_as_text).collect()).unwrap_or_default();
    let rows: Vec<Vec<String>> = v["rows"].as_array().map(|rs| rs.iter().map(|r| r.as_array().map(|c| c.iter().map(crate::fields::value_as_text).collect()).unwrap_or_default()).collect()).unwrap_or_default();
    if rows.is_empty() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Nothing to show").size(17.0).color(th.dim));
            ui.label(RichText::new("No tasks or resources match this report right now.").color(th.dim));
        });
        return;
    }
    // column widths from the widest cell (bounded), names get the rest
    let font = egui::TextStyle::Body.resolve(ui.style());
    let measure = |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    let mut widths: Vec<f32> = cols.iter().map(|c| measure(c) + 22.0).collect();
    for r in &rows {
        for (k, c) in r.iter().enumerate() {
            if k < widths.len() {
                widths[k] = widths[k].max((measure(c) + 22.0).min(320.0));
            }
        }
    }
    let numeric: Vec<bool> = (0..cols.len()).map(|k| cols[k] != "ID" && numeric_column(&rows.iter().map(|r| r.get(k).cloned().unwrap_or_default()).collect::<Vec<_>>())).collect();
    let total: f32 = widths.iter().sum();
    let avail = ui.available_width() - 4.0;
    if total < avail {
        // give the spare room to the widest text column
        if let Some((k, _)) = widths.iter().enumerate().filter(|(k, _)| !numeric[*k]).max_by(|a, b| a.1.total_cmp(b.1)) {
            widths[k] += avail - total;
        }
    }
    let row_h = 26.0;
    let width: f32 = widths.iter().sum();
    egui::ScrollArea::both().id_salt(("rep_scroll", title)).auto_shrink([false, false]).show(ui, |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(width, row_h * (rows.len() + 1) as f32), Sense::hover());
        let p = ui.painter_at(r.expand(1.0));
        // header
        let hr = Rect::from_min_size(r.min, vec2(width, row_h));
        p.rect_filled(hr, CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, th.accent_light);
        let mut x = r.left();
        for (k, c) in cols.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(x, r.top()), vec2(widths[k], row_h));
            let cp = p.with_clip_rect(cell);
            if numeric[k] {
                cp.text(pos2(cell.right() - 10.0, cell.center().y), Align2::RIGHT_CENTER, c, font.clone(), th.text);
            } else {
                cp.text(pos2(cell.left() + 10.0, cell.center().y), Align2::LEFT_CENTER, c, font.clone(), th.text);
            }
            x += widths[k];
        }
        // rows
        for (ri, row) in rows.iter().enumerate() {
            let y = r.top() + row_h * (ri + 1) as f32;
            let rr = Rect::from_min_size(pos2(r.left(), y), vec2(width, row_h));
            if ri % 2 == 1 {
                p.rect_filled(rr, 0.0, th.panel);
            }
            let mut x = r.left();
            for (k, c) in row.iter().enumerate().take(cols.len()) {
                let cell = Rect::from_min_size(pos2(x, y), vec2(widths[k], row_h));
                let cp = p.with_clip_rect(cell);
                let missed = c == "Missed" || (c.starts_with('-') && cols[k].to_lowercase().contains("variance") && c.len() > 1);
                let col = if missed { th.danger } else if k == 0 && cols[0] == "ID" { th.dim } else { th.text };
                let shown = super::gantt::fit_text(&p, c, &font, widths[k] - 20.0);
                if numeric[k] {
                    cp.text(pos2(cell.right() - 10.0, cell.center().y), Align2::RIGHT_CENTER, shown, font.clone(), col);
                } else {
                    cp.text(pos2(cell.left() + 10.0, cell.center().y), Align2::LEFT_CENTER, shown, font.clone(), col);
                }
                x += widths[k];
            }
        }
        p.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0_f32, th.grid), StrokeKind::Inside);
    });
    ui.add_space(6.0);
    ui.label(RichText::new(format!("{} row{}", rows.len(), if rows.len() == 1 { "" } else { "s" })).small().color(th.dim));
    let _ = app;
}
