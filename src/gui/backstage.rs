//! The File screen: a full-window page with a green list on the left (Home, New, Open, Info,
//! Save, Save As, Print, Share, Export, Close) like modern office suites. Every button runs a
//! registry command, so agents can do the same things.

use super::chrome::{bar_color, glyph, Glyph};
use super::{theme, App};
use crate::print::{self, Prim};
use crate::recent::RecentFile;
use crate::templates::{Template, TEMPLATES};
use egui::{pos2, vec2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind};
use serde_json::json;

pub struct State {
    pub page: String,
    pub search: String,
    pub pinned_tab: bool,
    pub recent: Vec<RecentFile>,
    pub save_name: String,
    pub save_folder: Option<String>,
    pub save_format: usize,
    pub props: [String; 4],
    pub preview_page: usize,
    pub from_text: String,
    pub to_text: String,
    pub template_search: String,
    pub note: String,
}

impl State {
    pub fn new(page: &str, app_title: &str) -> State {
        let mut st = State {
            page: page.to_string(),
            search: String::new(),
            pinned_tab: false,
            recent: vec![],
            save_name: app_title.to_string(),
            save_folder: None,
            save_format: 0,
            props: Default::default(),
            preview_page: 0,
            from_text: String::new(),
            to_text: String::new(),
            template_search: String::new(),
            note: String::new(),
        };
        st.reload();
        st
    }
    pub fn reload(&mut self) {
        self.recent = crate::recent::list();
    }
}

const SAVE_FORMATS: &[(&str, &str)] = &[
    ("plan.json", "Plan (*.plan.json)"),
    ("xml", "XML interchange (*.xml)"),
    ("pdf", "PDF (*.pdf)"),
    ("html", "Web page report (*.html)"),
    ("csv", "Task table, CSV (*.csv)"),
    ("svg", "Gantt chart picture (*.svg)"),
    ("md", "Markdown summary (*.md)"),
];

const NAV: &[(&str, &str, Glyph)] = &[
    ("home", "Home", Glyph::Home),
    ("new", "New", Glyph::New),
    ("open", "Open", Glyph::Open),
    ("-", "", Glyph::Home),
    ("info", "Info", Glyph::Info),
    ("save", "Save", Glyph::Save),
    ("saveAs", "Save As", Glyph::SaveAs),
    ("print", "Print", Glyph::Print),
    ("share", "Share", Glyph::Share),
    ("export", "Export", Glyph::Export),
    ("close", "Close", Glyph::Close),
];

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        0..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

/// Open the File screen at a page ("" closes it).
pub fn open(app: &mut App, page: &str) {
    if page.is_empty() {
        app.bs = None;
        return;
    }
    let page = match page {
        "saveas" => "saveAs",
        p => p,
    };
    match &mut app.bs {
        Some(st) => {
            st.page = page.to_string();
            st.reload();
        }
        None => {
            let name = app.s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| app.s.project.title.clone());
            let name = name.trim_end_matches(".plan.json").trim_end_matches(".json").trim_end_matches(".xml").to_string();
            app.bs = Some(State::new(page, &name));
        }
    }
    let ps = app.s.view.print.clone();
    if let Some(st) = &mut app.bs {
        st.props = [app.s.project.title.clone(), app.s.project.subject.clone(), app.s.project.manager.clone(), app.s.project.company.clone()];
        st.from_text = ps.from.map(|d| d.to_string()).unwrap_or_default();
        st.to_text = ps.to.map(|d| d.to_string()).unwrap_or_default();
        st.preview_page = 0;
    }
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let dark = app.s.view.dark;
    let th = theme(dark);
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.wants_keyboard_input() {
        app.bs = None;
        return;
    }
    let nav_fill = bar_color(dark);
    egui::SidePanel::left("bs_nav").exact_width(176.0).resizable(false).frame(egui::Frame::new().fill(nav_fill).inner_margin(egui::Margin::symmetric(0, 10))).show(ctx, |ui| nav(app, ui));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(th.bg).inner_margin(egui::Margin { left: 40, right: 32, top: 26, bottom: 16 })).show(ctx, |ui| {
        // outlined text boxes and dropdowns, as on office File screens
        let v = ui.visuals_mut();
        v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, th.grid);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, th.accent);
        v.selection.stroke = Stroke::new(1.5_f32, th.accent);
        let page = app.bs.as_ref().map(|b| b.page.clone()).unwrap_or_default();
        if page == "print" {
            print_page(app, ui);
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match page.as_str() {
            "new" => new_page(app, ui),
            "open" => open_page(app, ui),
            "info" => info_page(app, ui),
            "saveAs" => save_as_page(app, ui),
            "share" => share_page(app, ui),
            "export" => export_page(app, ui),
            _ => home_page(app, ui),
        });
    });
}

fn nav(app: &mut App, ui: &mut egui::Ui) {
    let white = Color32::WHITE;
    let page = app.bs.as_ref().map(|b| b.page.clone()).unwrap_or_default();
    ui.spacing_mut().item_spacing.y = 0.0;
    // back arrow
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_white_alpha(30));
    }
    glyph(ui.painter(), Rect::from_center_size(pos2(r.left() + 30.0, r.center().y), vec2(24.0, 24.0)), Glyph::Back, white);
    if resp.on_hover_text("Back to the plan (Esc)").clicked() {
        app.bs = None;
        return;
    }
    ui.add_space(8.0);
    let mut chosen: Option<&str> = None;
    for (id, label, g) in NAV {
        if *id == "-" {
            ui.add_space(6.0);
            let y = ui.cursor().top();
            ui.painter().line_segment([pos2(ui.max_rect().left() + 16.0, y), pos2(ui.max_rect().right() - 16.0, y)], Stroke::new(1.0_f32, Color32::from_white_alpha(70)));
            ui.add_space(7.0);
            continue;
        }
        if nav_item(ui, label, *g, page == *id) {
            chosen = Some(id);
        }
    }
    // bottom items
    let rest = ui.available_height();
    if rest > 100.0 {
        ui.add_space(rest - 92.0);
    }
    if nav_item(ui, "Feedback", Glyph::Feedback, false) {
        app.run("help.feedback", json!({}));
    }
    if nav_item(ui, "Options", Glyph::Options, false) {
        app.bs = None;
        app.run("app.options", json!({}));
        return;
    }
    if let Some(id) = chosen {
        match id {
            "save" => {
                if app.s.path.is_some() {
                    if app.run_value("file.save", json!({})).is_ok() {
                        app.bs = None;
                    }
                } else {
                    open(app, "saveAs");
                }
            }
            "close" => {
                app.bs = None;
                app.run("file.close", json!({}));
            }
            p => open(app, p),
        }
    }
}

fn nav_item(ui: &mut egui::Ui, label: &str, g: Glyph, selected: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
    if selected {
        ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_white_alpha(55));
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_white_alpha(28));
    }
    glyph(ui.painter(), Rect::from_center_size(pos2(r.left() + 30.0, r.center().y), vec2(17.0, 17.0)), g, Color32::WHITE);
    ui.painter().text(pos2(r.left() + 54.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(15.0), Color32::WHITE);
    resp.clicked()
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(28.0).color(theme(ui.visuals().dark_mode).text));
    ui.add_space(14.0);
}

fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).strong());
    ui.add_space(6.0);
}

fn link(ui: &mut egui::Ui, text: &str, accent: Color32) -> bool {
    ui.add(egui::Label::new(RichText::new(format!("{text}  >")).color(accent).size(14.0)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

// ---------------------------------------------------------------------------------------------
// Template cards
// ---------------------------------------------------------------------------------------------

enum Card<'a> {
    Blank,
    Existing,
    Template(&'a Template),
}

const CARD_W: f32 = 168.0;
const CARD_H: f32 = 118.0;

fn card(app: &mut App, ui: &mut egui::Ui, c: Card) {
    let th = theme(app.s.view.dark);
    let (title, desc) = match &c {
        Card::Blank => ("Blank Project", "An empty plan starting today."),
        Card::Existing => ("New from existing project", "Copy a plan you already have."),
        Card::Template(t) => (t.title, t.description),
    };
    ui.vertical(|ui| {
        ui.set_width(CARD_W);
        let (r, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
        let p = ui.painter();
        let fill = if app.s.view.dark { th.panel } else { Color32::WHITE };
        p.rect_filled(r, CornerRadius::same(4), fill);
        p.rect_stroke(r, CornerRadius::same(4), Stroke::new(if resp.hovered() { 2.0_f32 } else { 1.0 }, if resp.hovered() { th.accent } else { th.grid }), StrokeKind::Inside);
        let inner = r.shrink(10.0);
        match &c {
            Card::Blank => {
                // empty grid with a timescale
                p.rect_filled(Rect::from_min_size(inner.min, vec2(inner.width(), 10.0)), CornerRadius::ZERO, th.header);
                for k in 0..6 {
                    let y = inner.top() + 14.0 + k as f32 * 13.0;
                    p.line_segment([pos2(inner.left(), y), pos2(inner.right(), y)], Stroke::new(1.0_f32, th.grid));
                }
                p.line_segment([pos2(inner.left() + inner.width() * 0.38, inner.top()), pos2(inner.left() + inner.width() * 0.38, inner.bottom())], Stroke::new(1.0_f32, th.grid));
            }
            Card::Existing => {
                glyph(p, Rect::from_center_size(inner.center() - vec2(10.0, 4.0), vec2(40.0, 40.0)), Glyph::Doc, th.dim);
                glyph(p, Rect::from_center_size(inner.center() + vec2(14.0, 10.0), vec2(30.0, 30.0)), Glyph::New, th.accent);
            }
            Card::Template(t) => thumbnail(p, inner, t, &th, app.s.view.dark),
        }
        let r2 = ui.add(egui::Label::new(RichText::new(title).size(13.5)).wrap().sense(Sense::click()));
        let clicked = resp.clicked() || r2.clicked();
        resp.on_hover_text(desc);
        if clicked {
            match c {
                Card::Blank => {
                    app.bs = None;
                    app.run("file.new", json!({}));
                }
                Card::Existing => {
                    if app.run_value("file.newFromExisting", json!({})).map(|v| v.get("copiedFrom").is_some()).unwrap_or(false) {
                        app.bs = None;
                    }
                }
                Card::Template(t) => {
                    app.bs = None;
                    app.run("file.newFromTemplate", json!({"template": t.id}));
                }
            }
        }
    });
}

/// A small Gantt drawn from a template's task list.
fn thumbnail(p: &egui::Painter, r: Rect, t: &Template, th: &super::Theme, dark: bool) {
    let rows: Vec<(String, u8, f32, bool)> = if t.tasks.is_empty() {
        crate::sample::sample_session().project.tasks.iter().map(|x| (x.name.clone(), x.outline_level as u8, (x.duration.minutes as f32 / 480.0).max(0.0), x.summary)).collect()
    } else {
        t.tasks
            .iter()
            .enumerate()
            .map(|(k, (n, lvl, d, _, _))| {
                let days: f32 = d.trim_end_matches('d').parse().unwrap_or(1.0);
                let summary = t.tasks.get(k + 1).map(|nx| nx.1 > *lvl).unwrap_or(false);
                (n.to_string(), *lvl, days, summary)
            })
            .collect()
    };
    let n = rows.len().min(9);
    let row_h = (r.height() / n.max(1) as f32).min(11.0);
    let table_w = r.width() * 0.34;
    let chart = Rect::from_min_max(pos2(r.left() + table_w + 4.0, r.top()), r.max);
    let total: f32 = rows.iter().filter(|x| !x.3).map(|x| x.2).sum::<f32>().max(1.0);
    let scale = chart.width() / (total * 0.55).max(1.0);
    let mut x = chart.left();
    let bar = if dark { Color32::from_rgb(95, 191, 132) } else { Color32::from_rgb(93, 166, 120) };
    for (k, (_, lvl, days, summary)) in rows.iter().take(n).enumerate() {
        let y = r.top() + k as f32 * row_h + row_h / 2.0;
        // a line of "text" in the table
        let indent = (*lvl as f32 - 1.0) * 4.0;
        p.line_segment([pos2(r.left() + indent, y), pos2(r.left() + table_w - 6.0 - (k % 3) as f32 * 6.0, y)], Stroke::new(2.0_f32, if *summary { th.text } else { th.dim.gamma_multiply(0.6) }));
        if *summary {
            let w = (rows.iter().skip(k + 1).take_while(|x| x.1 > *lvl).filter(|x| !x.3).map(|x| x.2).sum::<f32>() * scale * 0.7).clamp(10.0, chart.right() - x);
            p.rect_filled(Rect::from_min_size(pos2(x, y - 1.5), vec2(w, 3.0)), CornerRadius::ZERO, th.text);
        } else if *days == 0.0 {
            let c = pos2(x, y);
            p.add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -3.5), c + vec2(3.5, 0.0), c + vec2(0.0, 3.5), c + vec2(-3.5, 0.0)], th.text, Stroke::NONE));
        } else {
            let w = (days * scale).clamp(4.0, (chart.right() - x).max(4.0));
            p.rect_filled(Rect::from_min_size(pos2(x, y - 3.0), vec2(w, 6.0)), CornerRadius::same(1), bar);
            x = (x + w * 0.8).min(chart.right() - 10.0);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------------------------

fn home_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, greeting());
    ui.horizontal(|ui| {
        section(ui, "New");
    });
    let room = ((ui.available_width() + 16.0) / (CARD_W + 16.0)).floor().max(2.0) as usize;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(16.0, 12.0);
        card(app, ui, Card::Blank);
        card(app, ui, Card::Existing);
        for t in TEMPLATES.iter().take((room - 2).min(4)) {
            card(app, ui, Card::Template(t));
        }
    });
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        if link(ui, "More templates", th.accent) {
            open(app, "new");
        }
    });
    ui.add_space(14.0);
    recent_block(app, ui, true);
}

fn recent_block(app: &mut App, ui: &mut egui::Ui, home: bool) {
    let th = theme(app.s.view.dark);
    let Some(st) = app.bs.as_mut() else { return };
    ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search").desired_width(ui.available_width().min(720.0)).margin(vec2(8.0, 6.0)));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        for (pinned, label) in [(false, "Recent"), (true, "Pinned")] {
            let sel = st.pinned_tab == pinned;
            let r = ui.add(egui::Label::new(RichText::new(label).size(14.5).color(if sel { th.text } else { th.dim }).strong()).sense(Sense::click()));
            if sel {
                ui.painter().line_segment([r.rect.left_bottom() + vec2(0.0, 3.0), r.rect.right_bottom() + vec2(0.0, 3.0)], Stroke::new(2.5_f32, th.accent));
            }
            if r.clicked() {
                st.pinned_tab = pinned;
            }
            ui.add_space(14.0);
        }
    });
    ui.add_space(10.0);
    let q = st.search.to_lowercase();
    let items: Vec<RecentFile> = st.recent.iter().filter(|r| (!st.pinned_tab || r.pinned) && (q.is_empty() || r.path.to_lowercase().contains(&q))).cloned().collect();
    // header
    let w = ui.available_width();
    let (hr, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    ui.painter().text(hr.left_center() + vec2(44.0, 0.0), egui::Align2::LEFT_CENTER, "Name", egui::FontId::proportional(13.0), th.dim);
    ui.painter().text(pos2(hr.right() - 190.0, hr.center().y), egui::Align2::LEFT_CENTER, "Date opened", egui::FontId::proportional(13.0), th.dim);
    ui.painter().line_segment([hr.left_bottom(), hr.right_bottom()], Stroke::new(1.0_f32, th.grid));
    if items.is_empty() {
        ui.add_space(14.0);
        let msg = if st.pinned_tab { "Pin plans you want to find easily: hover a recent plan and click the pin." } else if q.is_empty() { "Plans you open or save will show up here." } else { "No recent plans match your search." };
        ui.label(RichText::new(msg).color(th.dim));
    }
    let mut action: Option<(&str, String)> = None;
    let limit = if home { 12 } else { 30 };
    for r in items.iter().take(limit) {
        let (rr, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rr, CornerRadius::same(3), th.accent_light);
        }
        glyph(p, Rect::from_center_size(pos2(rr.left() + 20.0, rr.center().y), vec2(22.0, 22.0)), Glyph::Doc, th.accent);
        let path = std::path::Path::new(&r.path);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| r.path.clone());
        let folder = path.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let clip = Rect::from_min_max(rr.min, pos2(rr.right() - 200.0, rr.bottom()));
        p.with_clip_rect(clip).text(pos2(rr.left() + 44.0, rr.top() + 15.0), egui::Align2::LEFT_CENTER, &name, egui::FontId::proportional(14.0), th.text);
        p.with_clip_rect(clip).text(pos2(rr.left() + 44.0, rr.top() + 32.0), egui::Align2::LEFT_CENTER, &folder, egui::FontId::proportional(12.0), th.dim);
        p.text(pos2(rr.right() - 190.0, rr.center().y), egui::Align2::LEFT_CENTER, &r.opened, egui::FontId::proportional(13.0), th.dim);
        // pin button
        let pin_rect = Rect::from_center_size(pos2(rr.right() - 26.0, rr.center().y), vec2(26.0, 26.0));
        let pin_resp = ui.interact(pin_rect, ui.id().with(("pin", &r.path)), Sense::click());
        if r.pinned || resp.hovered() || pin_resp.hovered() {
            let c = if r.pinned { th.accent } else { th.dim };
            if pin_resp.hovered() {
                ui.painter().rect_filled(pin_rect, CornerRadius::same(3), th.select);
            }
            glyph(ui.painter(), pin_rect.shrink(5.0), Glyph::Pin, c);
        }
        if pin_resp.on_hover_text(if r.pinned { "Unpin" } else { "Pin to the list" }).clicked() {
            action = Some((if r.pinned { "unpin" } else { "pin" }, r.path.clone()));
        } else if resp.clicked() {
            action = Some(("open", r.path.clone()));
        }
        resp.on_hover_text(&r.path).context_menu(|ui| {
            if ui.button("Open").clicked() {
                action = Some(("open", r.path.clone()));
                ui.close_menu();
            }
            if ui.button("Open a copy").clicked() {
                action = Some(("copy", r.path.clone()));
                ui.close_menu();
            }
            if ui.button(if r.pinned { "Unpin from list" } else { "Pin to list" }).clicked() {
                action = Some((if r.pinned { "unpin" } else { "pin" }, r.path.clone()));
                ui.close_menu();
            }
            if ui.button("Copy path").clicked() {
                ui.ctx().copy_text(r.path.clone());
                ui.close_menu();
            }
            if ui.button("Remove from list").clicked() {
                action = Some(("remove", r.path.clone()));
                ui.close_menu();
            }
        });
    }
    if home && items.len() > 0 {
        ui.add_space(6.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if link(ui, "More plans", th.accent) {
                action = Some(("more", String::new()));
            }
        });
    }
    if let Some((what, path)) = action {
        match what {
            "open" => {
                if !std::path::Path::new(&path).exists() {
                    if let Some(st) = app.bs.as_mut() {
                        st.note = format!("{path} can't be found. It may have been moved or deleted.");
                    }
                    app.status = format!("Can't find {path}");
                    app.status_err = true;
                } else {
                    let res = app.run_value("file.open", json!({"path": path}));
                    if res.is_ok() {
                        app.bs = None;
                    }
                }
            }
            "copy" => {
                if app.run_value("file.newFromExisting", json!({"path": path})).is_ok() {
                    app.bs = None;
                }
            }
            "pin" => app.run("file.recent.pin", json!({"path": path, "pinned": true})),
            "unpin" => app.run("file.recent.pin", json!({"path": path, "pinned": false})),
            "remove" => app.run("file.recent.remove", json!({"path": path})),
            "more" => open(app, "open"),
            _ => {}
        }
        if let Some(st) = app.bs.as_mut() {
            st.reload();
        }
    }
    if let Some(st) = app.bs.as_ref() {
        if !st.note.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(&st.note).color(th.danger));
        }
    }
}

fn new_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "New");
    let q = {
        let Some(st) = app.bs.as_mut() else { return };
        ui.add(egui::TextEdit::singleline(&mut st.template_search).hint_text("Search templates").desired_width(ui.available_width().min(560.0)).margin(vec2(8.0, 6.0)));
        st.template_search.to_lowercase()
    };
    ui.add_space(16.0);
    let mut cards: Vec<Card> = vec![];
    if q.is_empty() || "blank project".contains(&q) {
        cards.push(Card::Blank);
    }
    if q.is_empty() || "new from existing project".contains(&q) {
        cards.push(Card::Existing);
    }
    for t in TEMPLATES.iter().filter(|t| q.is_empty() || t.title.to_lowercase().contains(&q) || t.description.to_lowercase().contains(&q)) {
        cards.push(Card::Template(t));
    }
    let per_row = ((ui.available_width() + 16.0) / (CARD_W + 16.0)).floor().max(1.0) as usize;
    let mut it = cards.into_iter().peekable();
    while it.peek().is_some() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(16.0, 16.0);
            for c in it.by_ref().take(per_row) {
                card(app, ui, c);
            }
        });
        ui.add_space(14.0);
    }
    ui.add_space(20.0);
    ui.label(RichText::new("Templates start on the next Monday; change it with Project > Project Information. Scripts can use file.newFromTemplate.").color(th.dim));
}

fn location_row(ui: &mut egui::Ui, g: Glyph, title: &str, sub: &str, selected: bool, th: &super::Theme) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(260.0, if sub.is_empty() { 40.0 } else { 50.0 }), Sense::click());
    if selected {
        ui.painter().rect_filled(r, CornerRadius::same(3), th.select);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(3), th.accent_light);
    }
    glyph(ui.painter(), Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(20.0, 20.0)), g, th.accent);
    if sub.is_empty() {
        ui.painter().text(pos2(r.left() + 46.0, r.center().y), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(14.0), th.text);
    } else {
        ui.painter().text(pos2(r.left() + 46.0, r.top() + 16.0), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(14.0), th.text);
        let clip = r;
        ui.painter().with_clip_rect(clip).text(pos2(r.left() + 46.0, r.top() + 34.0), egui::Align2::LEFT_CENTER, sub, egui::FontId::proportional(12.0), th.dim);
    }
    resp.on_hover_text(if sub.is_empty() { title } else { sub }).clicked()
}

fn open_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "Open");
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(270.0);
            location_row(ui, Glyph::Doc, "Recent", "", true, &th);
            if location_row(ui, Glyph::Folder, "This PC", "Browse your folders", false, &th) {
                if app.run_value("file.open", json!({})).is_ok() && app.s.path.is_some() {
                    app.bs = None;
                }
            }
            if location_row(ui, Glyph::New, "Sample plan", "The office move example", false, &th) {
                app.bs = None;
                app.run("file.newFromTemplate", json!({"template": "officeMove"}));
            }
        });
        ui.add_space(24.0);
        ui.separator();
        ui.add_space(16.0);
        ui.vertical(|ui| {
            recent_block(app, ui, false);
        });
    });
}

fn info_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "Info");
    let name = app.s.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| format!("{} (not saved yet)", app.s.project.title));
    ui.label(RichText::new(name).size(18.0));
    if let Some(p) = app.s.path.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(p.parent().map(|d| d.display().to_string()).unwrap_or_default()).color(th.dim));
            if ui.link("Open file location").clicked() {
                if let Some(dir) = p.parent() {
                    let _ = print::open_with_system(dir);
                }
            }
        });
    }
    ui.add_space(18.0);
    let ov = crate::reports::overview(&app.s.project);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.55).max(340.0));
            for (g, title, desc, cmd) in [
                (Glyph::Info, "Project Information", "Start date, calendar, status date and scheduling for the whole plan.", "project.information"),
                (Glyph::Table, "Statistics", "Current, baseline and actual start, finish, duration, work and cost.", "project.statistics"),
                (Glyph::Options, "Options", "Theme, start-up, scheduling defaults, saving, language and the Quick Access Toolbar.", "app.options"),
                (Glyph::Agent, "Agent access", "How scripts and AI agents can drive this window and the command line.", "help.agentAccess"),
            ] {
                if big_tile(ui, g, title, desc, &th) {
                    app.bs = None;
                    app.run(cmd, json!({}));
                    return;
                }
                ui.add_space(8.0);
            }
            ui.add_space(6.0);
            ui.label(RichText::new(format!("{} changes can be undone.  {}", app.s.undo.len(), if app.s.dirty { "There are unsaved changes." } else { "Everything is saved." })).color(th.dim));
        });
        ui.add_space(30.0);
        ui.vertical(|ui| {
            section(ui, "Properties");
            let Some(st) = app.bs.as_mut() else { return };
            let mut commit: Option<(usize, String)> = None;
            egui::Grid::new("bs_props").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
                for (k, label) in ["Title", "Subject", "Manager", "Company"].iter().enumerate() {
                    ui.label(RichText::new(*label).color(th.dim));
                    let r = ui.add(egui::TextEdit::singleline(&mut st.props[k]).hint_text(format!("Add a {}", label.to_lowercase())).desired_width(220.0));
                    if r.lost_focus() {
                        commit = Some((k, st.props[k].clone()));
                    }
                    ui.end_row();
                }
                let rows = [
                    ("Start", ov["start"].as_str().unwrap_or("").to_string()),
                    ("Finish", ov["finish"].as_str().unwrap_or("").to_string()),
                    ("Duration", ov["duration"].as_str().unwrap_or("").to_string()),
                    ("Tasks", ov["tasks"].to_string()),
                    ("Resources", ov["resources"].to_string()),
                    ("Complete", format!("{}%", ov["percentComplete"])),
                    ("Cost", format!("${:.2}", ov["cost"].as_f64().unwrap_or(0.0))),
                    ("Calendar", ov["calendar"].as_str().unwrap_or("").to_string()),
                    ("Status date", ov["statusDate"].as_str().unwrap_or("NA").to_string()),
                ];
                for (k, v) in rows {
                    ui.label(RichText::new(k).color(th.dim));
                    ui.label(v);
                    ui.end_row();
                }
            });
            if let Some((k, v)) = commit {
                let field = ["title", "subject", "manager", "company"][k];
                let cur = match k {
                    0 => app.s.project.title.clone(),
                    1 => app.s.project.subject.clone(),
                    2 => app.s.project.manager.clone(),
                    _ => app.s.project.company.clone(),
                };
                if cur != v && !(k == 0 && v.trim().is_empty()) {
                    app.run(&format!("project.set.{field}"), json!({"value": v}));
                }
            }
        });
    });
}

fn big_tile(ui: &mut egui::Ui, g: Glyph, title: &str, desc: &str, th: &super::Theme) -> bool {
    let w = ui.available_width().min(520.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 72.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(4), if resp.hovered() { th.accent_light } else { th.panel });
    let icon = Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(48.0, 48.0));
    p.rect_stroke(icon, CornerRadius::same(4), Stroke::new(1.0_f32, th.grid), StrokeKind::Inside);
    glyph(p, icon.shrink(12.0), g, th.accent);
    p.text(pos2(icon.right() + 16.0, r.top() + 22.0), egui::Align2::LEFT_CENTER, title, egui::FontId::proportional(15.0), th.text);
    let galley = p.layout(desc.to_string(), egui::FontId::proportional(12.5), th.dim, r.right() - icon.right() - 28.0);
    p.galley(pos2(icon.right() + 16.0, r.top() + 34.0), galley, th.dim);
    resp.clicked()
}

fn documents_dir() -> String {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(|h| std::path::Path::new(&h).join("Documents").to_string_lossy().to_string()).unwrap_or_else(|_| ".".into())
}

fn save_as_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "Save As");
    let current_dir = app.s.path.as_ref().and_then(|p| p.parent()).map(|d| d.to_string_lossy().to_string());
    let Some(st) = app.bs.as_mut() else { return };
    let mut folders: Vec<String> = vec![];
    if let Some(d) = &current_dir {
        folders.push(d.clone());
    }
    for r in &st.recent {
        if let Some(d) = std::path::Path::new(&r.path).parent() {
            let d = d.to_string_lossy().to_string();
            if !folders.contains(&d) {
                folders.push(d);
            }
        }
    }
    let docs = documents_dir();
    if !folders.contains(&docs) {
        folders.push(docs.clone());
    }
    folders.truncate(8);
    if st.save_folder.is_none() {
        st.save_folder = folders.first().cloned();
    }
    let mut browse = false;
    let mut do_save = false;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(270.0);
            ui.label(RichText::new("Folders").color(th.dim));
            ui.add_space(4.0);
            for f in &folders {
                let name = std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| f.clone());
                if location_row(ui, Glyph::Folder, &name, f, st.save_folder.as_deref() == Some(f.as_str()), &th) {
                    st.save_folder = Some(f.clone());
                }
            }
            ui.add_space(6.0);
            if location_row(ui, Glyph::Open, "Browse", "Pick any folder and name", false, &th) {
                browse = true;
            }
        });
        ui.add_space(24.0);
        ui.separator();
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(st.save_folder.clone().unwrap_or_default()).color(th.dim));
            ui.add_space(8.0);
            ui.add(egui::TextEdit::singleline(&mut st.save_name).hint_text("File name").desired_width(380.0).margin(vec2(8.0, 6.0)));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("bs_save_fmt").width(300.0).selected_text(SAVE_FORMATS[st.save_format].1).show_ui(ui, |ui| {
                    for (k, (_, label)) in SAVE_FORMATS.iter().enumerate() {
                        ui.selectable_value(&mut st.save_format, k, *label);
                    }
                });
                let b = egui::Button::new(RichText::new("  Save  ").color(Color32::WHITE)).fill(th.accent).min_size(vec2(72.0, 26.0));
                if ui.add(b).clicked() {
                    do_save = true;
                }
            });
            ui.add_space(10.0);
            ui.label(RichText::new("Plans (.plan.json) and XML files keep everything and become the open file; the other formats are copies for sharing.").color(th.dim));
            if !st.note.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&st.note).color(th.danger));
            }
        });
    });
    if browse {
        app.run("file.saveAs", json!({}));
        app.bs = None;
        return;
    }
    if do_save {
        let (name, folder, fmt) = (st.save_name.trim().to_string(), st.save_folder.clone().unwrap_or_else(documents_dir), SAVE_FORMATS[st.save_format].0);
        if name.is_empty() {
            st.note = "Give the file a name.".into();
            return;
        }
        let base = name.trim_end_matches(&format!(".{fmt}")).to_string();
        let path = std::path::Path::new(&folder).join(format!("{base}.{fmt}"));
        let ps = path.to_string_lossy().to_string();
        let res = if fmt == "pdf" { app.run_value("file.export.pdf", json!({"path": ps})) } else { app.run_value("file.saveAs", json!({"path": ps})) };
        match res {
            Ok(_) => {
                if fmt == "plan.json" || fmt == "xml" {
                    crate::recent::add(&path);
                }
                app.bs = None;
                app.status = format!("Saved {}", path.display());
            }
            Err(e) => {
                if let Some(st) = app.bs.as_mut() {
                    st.note = e;
                }
            }
        }
    }
}

fn action_row(ui: &mut egui::Ui, g: Glyph, title: &str, desc: &str, button: &str, th: &super::Theme) -> bool {
    let mut clicked = false;
    egui::Frame::new().fill(th.panel).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width().min(760.0));
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            glyph(ui.painter(), r.shrink(8.0), g, th.accent);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 130.0).max(200.0));
                ui.label(RichText::new(title).size(15.0));
                ui.label(RichText::new(desc).color(th.dim));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(button).min_size(vec2(110.0, 30.0))).clicked() {
                    clicked = true;
                }
            });
        });
    });
    ui.add_space(8.0);
    clicked
}

fn share_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "Share");
    section(ui, "With an AI agent or script");
    let port = app.control_port;
    let status = match port {
        Some(p) => format!("This window is listening on 127.0.0.1:{p}. Agents can call any of the {} commands; you'll see what they do in the status bar.", app.reg.len()),
        None => format!("Agent control is off for this window{}.", if app.control_note.is_empty() { String::new() } else { format!(" ({})", app.control_note) }),
    };
    ui.label(status);
    ui.add_space(6.0);
    for (label, text) in [
        ("MCP server for this window", "plancraft-cli mcp --bridge".to_string()),
        ("MCP server without a window", "plancraft-cli mcp".to_string()),
        ("Run one command", "plancraft-cli run plan.plan.json --cmd task.insert --params '{\"name\":\"Review\"}' --out plan.plan.json".to_string()),
    ] {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).color(th.dim));
            ui.add_space(8.0);
            ui.label(RichText::new(&text).monospace());
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(text.clone());
                app.status = "Copied".into();
            }
        });
    }
    ui.add_space(16.0);
    section(ui, "With people");
    if action_row(ui, Glyph::Copy, "Copy a summary", "Copy the plan's overview as text to paste into an email or chat.", "Copy", &th) {
        let tmp = std::env::temp_dir().join("plancraft-share.md");
        if app.run_value("file.export.markdown", json!({"path": tmp.to_string_lossy()})).is_ok() {
            if let Ok(t) = std::fs::read_to_string(&tmp) {
                ui.ctx().copy_text(t);
                app.status = "Copied the plan summary".into();
            }
        }
    }
    if action_row(ui, Glyph::Pdf, "Send as PDF", "Save the printed pages as a PDF you can attach.", "Save PDF", &th) {
        app.run("file.export.pdf", json!({}));
    }
    if action_row(ui, Glyph::Web, "Send as a web page", "A single HTML report anyone can open in a browser.", "Save page", &th) {
        app.run("file.export.html", json!({}));
    }
}

fn export_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    heading(ui, "Export");
    let rows: [(Glyph, &str, &str, &str, &str); 7] = [
        (Glyph::Pdf, "Create PDF", "The pages as they print, with layout and timescale kept.", "Create PDF", "file.export.pdf"),
        (Glyph::Web, "Web page report", "An HTML page with the overview and the task table.", "Save", "file.export.html"),
        (Glyph::Picture, "Gantt chart picture", "An SVG picture of the Gantt chart.", "Save", "file.export.svg"),
        (Glyph::Table, "Task table (CSV)", "Open the tasks in a spreadsheet.", "Save", "file.export.csv"),
        (Glyph::Table, "Resource table (CSV)", "Open the resources in a spreadsheet.", "Save", "file.export.resourcesCsv"),
        (Glyph::Export, "XML interchange file", "For other project scheduling programs.", "Save", "file.export.xml"),
        (Glyph::Doc, "Markdown summary", "Plain text with headings and tables.", "Save", "file.export.markdown"),
    ];
    for (g, title, desc, button, cmd) in rows {
        if action_row(ui, g, title, desc, button, &th) {
            app.run(cmd, json!({}));
        }
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Scripts can use the same commands with a path, e.g. file.export.pdf {\"path\":\"plan.pdf\"}.").color(th.dim));
}

// ---------------------------------------------------------------------------------------------
// Print
// ---------------------------------------------------------------------------------------------

fn print_page(app: &mut App, ui: &mut egui::Ui) {
    let th = theme(app.s.view.dark);
    let full = ui.available_rect_before_wrap();
    let left_w = 300.0;
    let mut ps = app.s.view.print.clone();
    let pages = print::layout(&app.s, &ps);
    let mut changed = false;
    let mut do_print = false;
    let mut do_pdf = false;
    let mut page_setup = false;
    // settings column
    let left = Rect::from_min_size(full.min, vec2(left_w, full.height()));
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(left), |ui| {
        egui::ScrollArea::vertical().id_salt("print_settings").show(ui, |ui| {
            heading(ui, "Print");
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(84.0, 84.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(r, CornerRadius::same(4), if resp.hovered() { th.accent_light } else { th.panel });
                p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, th.grid), StrokeKind::Inside);
                glyph(p, Rect::from_center_size(r.center() - vec2(0.0, 10.0), vec2(34.0, 34.0)), Glyph::Print, th.accent);
                p.text(pos2(r.center().x, r.bottom() - 16.0), egui::Align2::CENTER_CENTER, "Print", egui::FontId::proportional(14.0), th.text);
                if resp.on_hover_text("Open the pages in your PDF viewer to print (Ctrl+P)").clicked() {
                    do_print = true;
                }
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add_space(14.0);
                    ui.label(RichText::new(format!("{} page{}", pages.len(), if pages.len() == 1 { "" } else { "s" })).size(14.0));
                    ui.label(RichText::new(print::PAPERS.iter().find(|(k, _)| *k == ps.paper).map(|x| x.1).unwrap_or("")).color(th.dim));
                });
            });
            ui.add_space(14.0);
            section(ui, "Printer");
            egui::Frame::new().stroke(Stroke::new(1.0_f32, th.grid)).corner_radius(CornerRadius::same(3)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(270.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    glyph(ui.painter(), r.shrink(4.0), Glyph::Print, th.text);
                    ui.vertical(|ui| {
                        ui.label("System printer");
                        ui.label(RichText::new("Your PDF viewer prints the pages").small().color(th.dim));
                    });
                });
            });
            if ui.link("Save as PDF instead").clicked() {
                do_pdf = true;
            }
            ui.add_space(14.0);
            section(ui, "Settings");
            let w = 270.0;
            let whole = ps.from.is_none() && ps.to.is_none();
            egui::ComboBox::from_id_salt("print_range").width(w).selected_text(if whole { "Print Entire Project" } else { "Print Specific Dates" }).show_ui(ui, |ui| {
                if ui.selectable_label(whole, "Print Entire Project").clicked() {
                    ps.from = None;
                    ps.to = None;
                    changed = true;
                }
                if ui.selectable_label(!whole, "Print Specific Dates").clicked() && whole {
                    ps.from = Some(app.s.project.start.date());
                    ps.to = Some(app.s.project.finish.date());
                    changed = true;
                }
            });
            if let Some(st) = app.bs.as_mut() {
                if changed {
                    st.from_text = ps.from.map(|d| d.to_string()).unwrap_or_default();
                    st.to_text = ps.to.map(|d| d.to_string()).unwrap_or_default();
                }
                if !(ps.from.is_none() && ps.to.is_none()) {
                    ui.horizontal(|ui| {
                        ui.label("From");
                        let r1 = ui.add(egui::TextEdit::singleline(&mut st.from_text).desired_width(90.0));
                        ui.label("To");
                        let r2 = ui.add(egui::TextEdit::singleline(&mut st.to_text).desired_width(90.0));
                        if r1.lost_focus() || r2.lost_focus() {
                            ps.from = crate::model::parse_date(&st.from_text, 0).ok().map(|d| d.date());
                            ps.to = crate::model::parse_date(&st.to_text, 0).ok().map(|d| d.date());
                            changed = true;
                        }
                    });
                }
            }
            ui.add_space(4.0);
            egui::ComboBox::from_id_salt("print_table").width(w).selected_text(if ps.table { "Print table and chart" } else { "Print chart only" }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut ps.table, true, "Print table and chart").changed();
                changed |= ui.selectable_value(&mut ps.table, false, "Print chart only").changed();
            });
            ui.add_space(4.0);
            egui::ComboBox::from_id_salt("print_rows").width(w).selected_text(if ps.visible_only { "Rows shown in the view" } else { "All rows (expanded)" }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut ps.visible_only, true, "Rows shown in the view").changed();
                changed |= ui.selectable_value(&mut ps.visible_only, false, "All rows (expanded)").changed();
            });
            ui.add_space(4.0);
            egui::ComboBox::from_id_salt("print_orient").width(w).selected_text(if ps.landscape { "Landscape Orientation" } else { "Portrait Orientation" }).show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut ps.landscape, true, "Landscape Orientation").changed();
                changed |= ui.selectable_value(&mut ps.landscape, false, "Portrait Orientation").changed();
            });
            ui.add_space(4.0);
            let cur = print::PAPERS.iter().find(|(k, _)| *k == ps.paper).map(|x| x.1).unwrap_or("Letter");
            egui::ComboBox::from_id_salt("print_paper").width(w).selected_text(cur).show_ui(ui, |ui| {
                for (k, label) in print::PAPERS {
                    if ui.selectable_label(ps.paper == *k, *label).clicked() {
                        ps.paper = k.to_string();
                        changed = true;
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Header");
                let r = ui.add(egui::TextEdit::singleline(&mut ps.header).hint_text("the plan's title").desired_width(200.0));
                if r.lost_focus() {
                    changed = true;
                }
            });
            ui.add_space(8.0);
            if ui.link("Page Setup").on_hover_text("All the print settings as one command (file.printSetup)").clicked() {
                page_setup = true;
            }
        });
    });
    // preview
    let right = Rect::from_min_max(pos2(full.left() + left_w + 24.0, full.top()), full.max);
    let p = ui.painter_at(right);
    p.rect_filled(right, CornerRadius::ZERO, if app.s.view.dark { Color32::from_gray(50) } else { Color32::from_rgb(230, 230, 230) });
    let n = pages.len().max(1);
    let cur = app.bs.as_ref().map(|b| b.preview_page).unwrap_or(0).min(n - 1);
    let nav_h = 40.0;
    if let Some(pg) = pages.get(cur) {
        let avail = Rect::from_min_max(right.min + vec2(24.0, 24.0), right.max - vec2(24.0, 24.0 + nav_h));
        let scale = (avail.width() / pg.w).min(avail.height() / pg.h).max(0.05);
        let size = vec2(pg.w * scale, pg.h * scale);
        let page_rect = Rect::from_center_size(avail.center(), size);
        p.rect_filled(page_rect.translate(vec2(3.0, 4.0)), CornerRadius::ZERO, Color32::from_black_alpha(40));
        p.rect_filled(page_rect, CornerRadius::ZERO, Color32::WHITE);
        draw_page(&p, page_rect, scale, pg);
    }
    // page navigation
    let nav_rect = Rect::from_min_max(pos2(right.left(), right.bottom() - nav_h), right.max);
    let mut new_page = cur;
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(nav_rect), |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(nav_rect.width() / 2.0 - 70.0);
            if ui.add_enabled(cur > 0, egui::Button::new(" < ")).on_hover_text("Previous page").clicked() {
                new_page = cur - 1;
            }
            ui.label(format!("{} of {}", cur + 1, n));
            if ui.add_enabled(cur + 1 < n, egui::Button::new(" > ")).on_hover_text("Next page").clicked() {
                new_page = cur + 1;
            }
        });
    });
    if let Some(st) = app.bs.as_mut() {
        st.preview_page = new_page;
    }
    if changed {
        let params = json!({
            "landscape": ps.landscape, "paper": ps.paper, "table": ps.table, "visibleOnly": ps.visible_only, "header": ps.header,
            "from": ps.from.map(|d| d.to_string()).unwrap_or_default(), "to": ps.to.map(|d| d.to_string()).unwrap_or_default(),
        });
        app.run("file.printSetup", params);
        if let Some(st) = app.bs.as_mut() {
            st.preview_page = 0;
        }
    }
    if do_print {
        app.run("file.print", json!({}));
    }
    if do_pdf {
        app.run("file.export.pdf", json!({}));
    }
    if page_setup {
        app.open_palette_with("file.printSetup");
    }
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Draw one laid-out page (points, origin top-left) into a rectangle on screen.
fn draw_page(p: &egui::Painter, r: Rect, scale: f32, pg: &print::Page) {
    let at = |x: f32, y: f32| r.min + vec2(x * scale, y * scale);
    let p = p.with_clip_rect(r);
    for pr in &pg.prims {
        match pr {
            Prim::Rect { x, y, w, h, fill, stroke } => {
                let rr = Rect::from_min_size(at(*x, *y), vec2(w * scale, h * scale));
                if let Some(f) = fill {
                    p.rect_filled(rr, CornerRadius::ZERO, rgb(*f));
                }
                if let Some(s) = stroke {
                    p.rect_stroke(rr, CornerRadius::ZERO, Stroke::new((0.6 * scale).max(0.5), rgb(*s)), StrokeKind::Middle);
                }
            }
            Prim::Line { x1, y1, x2, y2, color, width } => {
                p.line_segment([at(*x1, *y1), at(*x2, *y2)], Stroke::new((width * scale).max(0.5), rgb(*color)));
            }
            Prim::Diamond { cx, cy, r: rad, color } => {
                let c = at(*cx, *cy);
                let d = rad * scale;
                p.add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)], rgb(*color), Stroke::NONE));
            }
            Prim::Text { x, y, size, text, color, bold, right } => {
                let fs = size * scale;
                if fs < 3.0 {
                    // too small to read: a grey line keeps the page's look
                    let w = print::text_width(text, *size) * scale;
                    let x0 = if *right { x * scale - w } else { x * scale };
                    let yy = y * scale - fs * 0.35;
                    p.line_segment([r.min + vec2(x0, yy), r.min + vec2(x0 + w, yy)], Stroke::new((fs * 0.5).max(0.6), rgb(*color).gamma_multiply(0.5)));
                    continue;
                }
                let font = egui::FontId::proportional(fs);
                let anchor = if *right { egui::Align2::RIGHT_BOTTOM } else { egui::Align2::LEFT_BOTTOM };
                // y is the baseline; egui places the text box, so lift by the descender
                let pos = at(*x, *y) + vec2(0.0, fs * 0.22);
                let galley_color = rgb(*color);
                p.text(pos, anchor, text, font.clone(), galley_color);
                if *bold {
                    p.text(pos + vec2(0.35, 0.0), anchor, text, font, galley_color);
                }
            }
        }
    }
}
