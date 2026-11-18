//! The window's own title bar (the system one is turned off), edge resizing, and the small
//! line glyphs used there and on the File screen.

use super::{theme, App};
use egui::{pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind};
use serde_json::json;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Home,
    New,
    Open,
    Info,
    Save,
    SaveAs,
    Print,
    Share,
    Export,
    Close,
    Back,
    Options,
    Feedback,
    Undo,
    Redo,
    Minimize,
    Maximize,
    Restore,
    X,
    Doc,
    Pin,
    Search,
    Folder,
    Agent,
    Copy,
    Pdf,
    Web,
    Table,
    Picture,
}

/// The title bar's colour: the app's green.
pub fn bar_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(28, 74, 48)
    } else {
        Color32::from_rgb(33, 102, 62)
    }
}

/// Draw a line glyph centred in `r` (about 16 px works best).
pub fn glyph(p: &Painter, r: Rect, g: Glyph, c: Color32) {
    let s = r.width().min(r.height());
    let k = s / 16.0;
    let o = r.center() - vec2(8.0 * k, 8.0 * k);
    let pt = |x: f32, y: f32| -> Pos2 { o + vec2(x * k, y * k) };
    let st = Stroke::new((1.3 * k).max(1.0), c);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| pt(x, y)).collect(), st));
    };
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(Rect::from_min_size(pt(x, y), vec2(w * k, h * k)), CornerRadius::same(1), st, StrokeKind::Middle);
    };
    match g {
        Glyph::Home => line(&[(2.0, 8.0), (8.0, 2.5), (14.0, 8.0), (12.5, 8.0), (12.5, 14.0), (3.5, 14.0), (3.5, 8.0), (2.0, 8.0)]),
        Glyph::New | Glyph::Doc => {
            line(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5), (3.5, 1.5)]);
            line(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)]);
            if g == Glyph::Doc {
                line(&[(5.5, 8.0), (10.5, 8.0)]);
                line(&[(5.5, 10.5), (10.5, 10.5)]);
            }
        }
        Glyph::Open | Glyph::Folder => {
            line(&[(1.5, 13.5), (1.5, 3.0), (6.0, 3.0), (7.5, 4.5), (13.0, 4.5), (13.0, 6.5)]);
            line(&[(1.5, 13.5), (3.5, 6.5), (15.0, 6.5), (13.0, 13.5), (1.5, 13.5)]);
        }
        Glyph::Info => {
            p.circle_stroke(pt(8.0, 8.0), 6.5 * k, st);
            line(&[(8.0, 7.0), (8.0, 11.5)]);
            p.circle_filled(pt(8.0, 4.8), 0.9 * k, c);
        }
        Glyph::Save | Glyph::SaveAs => {
            line(&[(2.0, 2.0), (11.5, 2.0), (14.0, 4.5), (14.0, 14.0), (2.0, 14.0), (2.0, 2.0)]);
            rect(4.5, 2.0, 6.0, 3.5);
            rect(4.5, 9.0, 7.0, 5.0);
            if g == Glyph::SaveAs {
                line(&[(9.0, 15.0), (15.0, 9.0)]);
            }
        }
        Glyph::Print => {
            rect(4.0, 1.5, 8.0, 4.0);
            line(&[(4.0, 11.5), (1.5, 11.5), (1.5, 5.5), (14.5, 5.5), (14.5, 11.5), (12.0, 11.5)]);
            rect(4.0, 9.0, 8.0, 5.5);
        }
        Glyph::Share => {
            p.circle_stroke(pt(12.0, 3.5), 2.0 * k, st);
            p.circle_stroke(pt(4.0, 8.0), 2.0 * k, st);
            p.circle_stroke(pt(12.0, 12.5), 2.0 * k, st);
            line(&[(5.8, 7.0), (10.2, 4.5)]);
            line(&[(5.8, 9.0), (10.2, 11.5)]);
        }
        Glyph::Export => {
            line(&[(9.0, 2.0), (2.0, 2.0), (2.0, 14.0), (12.0, 14.0), (12.0, 10.0)]);
            line(&[(6.0, 8.0), (15.0, 8.0)]);
            line(&[(12.0, 5.0), (15.0, 8.0), (12.0, 11.0)]);
        }
        Glyph::Close => {
            rect(2.0, 2.0, 12.0, 12.0);
            line(&[(5.5, 5.5), (10.5, 10.5)]);
            line(&[(10.5, 5.5), (5.5, 10.5)]);
        }
        Glyph::Back => {
            p.circle_stroke(pt(8.0, 8.0), 7.0 * k, st);
            line(&[(4.5, 8.0), (11.5, 8.0)]);
            line(&[(7.5, 5.0), (4.5, 8.0), (7.5, 11.0)]);
        }
        Glyph::Options => {
            p.circle_stroke(pt(8.0, 8.0), 2.3 * k, st);
            for a in 0..8 {
                let ang = a as f32 * std::f32::consts::FRAC_PI_4;
                let (sn, cs) = ang.sin_cos();
                line(&[(8.0 + cs * 4.5, 8.0 + sn * 4.5), (8.0 + cs * 6.8, 8.0 + sn * 6.8)]);
            }
        }
        Glyph::Feedback => {
            line(&[(2.0, 2.5), (14.0, 2.5), (14.0, 11.0), (7.0, 11.0), (4.0, 14.0), (4.0, 11.0), (2.0, 11.0), (2.0, 2.5)]);
        }
        Glyph::Undo | Glyph::Redo => {
            let f = |x: f32| if g == Glyph::Undo { x } else { 16.0 - x };
            // a hooked arrow: out to the right, round the bend and back
            let mut pts = vec![(f(3.0), 6.0)];
            for i in 0..=8 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 8.0;
                pts.push((f(10.0 + 3.5 * a.cos()), 9.5 + 3.5 * a.sin()));
            }
            pts.push((f(6.0), 13.0));
            line(&pts);
            line(&[(f(5.8), 3.2), (f(3.0), 6.0), (f(5.8), 8.8)]);
        }
        Glyph::Minimize => line(&[(3.0, 8.0), (13.0, 8.0)]),
        Glyph::Maximize => rect(3.0, 3.0, 10.0, 10.0),
        Glyph::Restore => {
            rect(3.0, 5.0, 8.0, 8.0);
            line(&[(5.0, 5.0), (5.0, 3.0), (13.0, 3.0), (13.0, 11.0), (11.0, 11.0)]);
        }
        Glyph::X => {
            line(&[(3.0, 3.0), (13.0, 13.0)]);
            line(&[(13.0, 3.0), (3.0, 13.0)]);
        }
        Glyph::Pin => {
            line(&[(6.0, 1.5), (10.0, 1.5), (9.5, 6.0), (12.0, 9.0), (4.0, 9.0), (6.5, 6.0), (6.0, 1.5)]);
            line(&[(8.0, 9.0), (8.0, 15.0)]);
        }
        Glyph::Search => {
            p.circle_stroke(pt(6.5, 6.5), 4.5 * k, st);
            line(&[(10.0, 10.0), (14.5, 14.5)]);
        }
        Glyph::Agent => {
            rect(3.0, 4.5, 10.0, 8.5);
            line(&[(8.0, 4.5), (8.0, 2.0)]);
            p.circle_filled(pt(6.0, 8.5), 1.0 * k, c);
            p.circle_filled(pt(10.0, 8.5), 1.0 * k, c);
            line(&[(6.0, 11.0), (10.0, 11.0)]);
        }
        Glyph::Copy => {
            rect(5.0, 5.0, 9.0, 9.5);
            line(&[(3.0, 11.0), (2.0, 11.0), (2.0, 1.5), (10.5, 1.5), (10.5, 3.0)]);
        }
        Glyph::Pdf => {
            line(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5), (3.5, 1.5)]);
            p.rect_filled(Rect::from_min_size(pt(1.5, 8.0), vec2(9.0 * k, 4.5 * k)), CornerRadius::same(1), Color32::from_rgb(196, 43, 43));
        }
        Glyph::Web => {
            p.circle_stroke(pt(8.0, 8.0), 6.5 * k, st);
            line(&[(1.5, 8.0), (14.5, 8.0)]);
            p.add(Shape::ellipse_stroke(pt(8.0, 8.0), vec2(2.8 * k, 6.5 * k), st));
        }
        Glyph::Table => {
            rect(1.5, 2.5, 13.0, 11.0);
            line(&[(1.5, 6.0), (14.5, 6.0)]);
            line(&[(1.5, 9.8), (14.5, 9.8)]);
            line(&[(6.0, 2.5), (6.0, 13.5)]);
        }
        Glyph::Picture => {
            rect(1.5, 2.5, 13.0, 11.0);
            line(&[(1.5, 12.0), (6.0, 7.5), (9.0, 10.5), (11.0, 8.5), (14.5, 12.0)]);
            p.circle_filled(pt(11.0, 5.5), 1.2 * k, c);
        }
    }
}

/// The line glyph drawn for a command in the title bar, when it has one.
pub fn command_glyph(id: &str) -> Option<Glyph> {
    Some(match id {
        "file.save" => Glyph::Save,
        "file.saveAs" => Glyph::SaveAs,
        "edit.undo" => Glyph::Undo,
        "edit.redo" => Glyph::Redo,
        "file.new" => Glyph::New,
        "file.open" => Glyph::Open,
        "file.print" | "file.backstage.print" => Glyph::Print,
        "file.export.pdf" => Glyph::Pdf,
        "edit.copy" => Glyph::Copy,
        "app.commandPalette" | "edit.find" => Glyph::Search,
        "app.options" | "project.options" => Glyph::Options,
        "file.backstage.home" => Glyph::Home,
        _ => return None,
    })
}

/// A flat button showing a glyph; returns its response.
pub fn glyph_button(ui: &mut egui::Ui, g: Glyph, size: egui::Vec2, color: Color32, hover_fill: Color32, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, hover_fill);
    }
    // white on the solid (red) close-button hover, the given colour otherwise
    let color = if resp.hovered() && hover_fill.a() == 255 { Color32::WHITE } else { color };
    glyph(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), g, color);
    resp.on_hover_text(tip)
}

pub const TITLE_H: f32 = 34.0;

/// The title bar: app mark, quick buttons, the plan's name, the command search box and the
/// window buttons. Dragging it moves the window; double-clicking maximizes.
pub fn title_bar(app: &mut App, ui: &mut egui::Ui) {
    let dark = app.s.view.dark;
    let bar = bar_color(dark);
    let white = theme(dark).text;
    let hover = if dark { Color32::from_white_alpha(30) } else { Color32::from_black_alpha(18) };
    let full = ui.max_rect();
    let drag = ui.interact(full, ui.id().with("title_drag"), Sense::click_and_drag());
    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
    if drag.double_clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    } else if drag.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    let _ = bar;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        // the app mark: a small page with three schedule bars
        let (mr, _) = ui.allocate_exact_size(vec2(40.0, TITLE_H), Sense::hover());
        let m = Rect::from_center_size(mr.center(), vec2(20.0, 18.0));
        ui.painter().rect_filled(m, CornerRadius::same(3), bar);
        for (k, (x, w)) in [(3.0, 8.0), (6.0, 9.0), (9.0, 7.0)].iter().enumerate() {
            ui.painter().rect_filled(Rect::from_min_size(m.min + vec2(*x, 3.5 + k as f32 * 4.3), vec2(*w, 2.6)), CornerRadius::same(1), Color32::WHITE);
        }
        // the Quick Access Toolbar (File > Options > Quick Access Toolbar)
        let bs = vec2(34.0, TITLE_H);
        let p = crate::prefs::get();
        let dim = theme(dark).dim.gamma_multiply(0.6);
        if p.show_quick_access {
            for id in &p.quick_access {
                let Some(c) = app.reg.find(id) else { continue };
                let enabled = match id.as_str() {
                    "edit.undo" => !app.s.undo.is_empty(),
                    "edit.redo" => !app.s.redo.is_empty(),
                    _ => true,
                };
                let tip = match &c.shortcut {
                    Some(k) => format!("{} ({k})", c.title.trim_end_matches("...")),
                    None => c.title.trim_end_matches("...").to_string(),
                };
                let resp = match command_glyph(id) {
                    Some(g) => glyph_button(ui, g, bs, if enabled { white } else { dim }, hover, &tip),
                    None => {
                        let (r, resp) = ui.allocate_exact_size(bs, Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::ZERO, hover);
                        }
                        if let Some(src) = super::icons::for_command(id) {
                            egui::Image::new(src).fit_to_exact_size(vec2(18.0, 18.0)).paint_at(ui, Rect::from_center_size(r.center(), vec2(18.0, 18.0)));
                        } else {
                            let short: String = c.title.chars().filter(|ch| ch.is_alphanumeric()).take(2).collect();
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, short, egui::FontId::proportional(12.0), white);
                        }
                        resp.on_hover_text(tip)
                    }
                };
                if resp.clicked() && enabled {
                    let c = c.clone();
                    app.invoke(&c);
                }
            }
        }
        // window buttons, right to left
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let wb = vec2(46.0, TITLE_H);
            let close = glyph_button(ui, Glyph::X, wb, white, Color32::from_rgb(196, 43, 28), "Close");
            if close.clicked() {
                app.run("file.exit", json!({}));
            }
            if glyph_button(ui, if maximized { Glyph::Restore } else { Glyph::Maximize }, wb, white, hover, if maximized { "Restore Down" } else { "Maximize" }).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            if glyph_button(ui, Glyph::Minimize, wb, white, hover, "Minimize").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            ui.add_space(10.0);
            // the person's initials (File > Options > General)
            if !p.initials.trim().is_empty() {
                let (ir, iresp) = ui.allocate_exact_size(vec2(34.0, TITLE_H), Sense::click());
                ui.painter().circle_filled(ir.center(), 13.0, Color32::from_rgb(196, 64, 160));
                ui.painter().text(ir.center(), egui::Align2::CENTER_CENTER, p.initials.trim().to_uppercase(), egui::FontId::proportional(12.0), Color32::WHITE);
                let who = if p.user_name.is_empty() { p.initials.clone() } else { p.user_name.clone() };
                if iresp.on_hover_text(format!("{who}: click for Options")).clicked() {
                    app.run("app.options", json!({"tab": "general"}));
                }
                ui.add_space(4.0);
            }
            // agents indicator
            let (ar, aresp) = ui.allocate_exact_size(vec2(30.0, TITLE_H), Sense::click());
            let on = app.control_port.is_some();
            ui.painter().circle_filled(ar.center(), 12.0, if aresp.hovered() { hover.gamma_multiply(2.0) } else { hover });
            glyph(ui.painter(), Rect::from_center_size(ar.center(), vec2(15.0, 15.0)), Glyph::Agent, white);
            if on {
                ui.painter().circle_filled(ar.center() + vec2(8.0, 8.0), 3.5, Color32::from_rgb(40, 170, 90));
            }
            let tip = match app.control_port {
                Some(p) => format!("Agents can drive this window (127.0.0.1:{p}). Click for details."),
                None => "Agent control is off".to_string(),
            };
            if aresp.on_hover_text(tip).clicked() {
                app.run("file.backstage.share", json!({}));
            }
            // centre: title and the command search box
            let left = ui.max_rect().left();
            let space = ui.available_width();
            let title = app.s.title();
            let search_w = (space * 0.42).clamp(160.0, 420.0);
            let right = left + space;
            let center_x = (full.center().x + 40.0).min(right - search_w / 2.0 - 8.0).max(left + search_w / 2.0 + 8.0);
            let srect = Rect::from_center_size(pos2(center_x, full.center().y), vec2(search_w, 24.0));
            let sresp = ui.interact(srect, ui.id().with("title_search"), Sense::click());
            let th = theme(dark);
            ui.painter().rect_filled(srect, CornerRadius::same(5), if dark { th.bg } else { Color32::WHITE });
            ui.painter().rect_stroke(srect, CornerRadius::same(5), Stroke::new(1.0_f32, if sresp.hovered() { th.accent } else { th.grid }), StrokeKind::Inside);
            glyph(ui.painter(), Rect::from_center_size(srect.left_center() + vec2(16.0, 0.0), vec2(13.0, 13.0)), Glyph::Search, th.dim);
            ui.painter().text(srect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, "Tell me what you want to do (Alt+Q)", egui::FontId::proportional(13.0), th.dim);
            if sresp.on_hover_text("Find and run any command").clicked() {
                app.run("app.commandPalette", json!({}));
            }
            // the title sits to the left of the search box
            let title_right = srect.left() - 16.0;
            if title_right - left > 60.0 {
                let galley = ui.painter().layout_no_wrap(title, egui::FontId::proportional(13.0), white);
                let w = galley.size().x;
                let x = (title_right - w).max(left + 4.0);
                let clip = Rect::from_min_max(pos2(left, full.top()), pos2(title_right, full.bottom()));
                ui.painter().with_clip_rect(clip).galley(pos2(x, full.center().y - galley.size().y / 2.0), galley, white);
            }
        });
    });
}

/// Edge and corner resizing for the undecorated window.
pub fn resize_edges(ctx: &egui::Context) {
    use egui::{CursorIcon, ResizeDirection as D};
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false));
    if maximized {
        return;
    }
    let r = ctx.screen_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let e = 5.0;
    let (l, rt, t, b) = (pos.x - r.left() < e, r.right() - pos.x < e, pos.y - r.top() < e, r.bottom() - pos.y < e);
    let dir = match (l, rt, t, b) {
        (true, _, true, _) => D::NorthWest,
        (_, true, true, _) => D::NorthEast,
        (true, _, _, true) => D::SouthWest,
        (_, true, _, true) => D::SouthEast,
        (true, _, _, _) => D::West,
        (_, true, _, _) => D::East,
        (_, _, true, _) => D::North,
        (_, _, _, true) => D::South,
        _ => return,
    };
    let icon = match dir {
        D::North | D::South => CursorIcon::ResizeVertical,
        D::East | D::West => CursorIcon::ResizeHorizontal,
        D::NorthWest | D::SouthEast => CursorIcon::ResizeNwSe,
        D::NorthEast | D::SouthWest => CursorIcon::ResizeNeSw,
    };
    ctx.set_cursor_icon(icon);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
    }
}

/// A thin border around the undecorated window.
pub fn window_border(ctx: &egui::Context, dark: bool) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }
    let r = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("window_border")));
    p.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0_f32, if dark { Color32::from_gray(70) } else { bar_color(false) }), StrokeKind::Inside);
    let _ = theme;
}
