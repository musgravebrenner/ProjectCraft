//! The window: menus and ribbon built from the command registry, the views, dialogs, the
//! control channel listener and the `--screenshot` mode.

mod backstage;
pub mod chrome;
mod dialogs;
mod gantt;
mod icons;
mod more_dialogs;
mod more_views;
mod options;
mod report_view;
mod ribbon_def;
pub use ribbon_def::{ribbon_groups, RibbonItem};
mod views;

use crate::commands::{Command, Registry};
use crate::control::{self, ControlRequest};
use crate::model::*;
use crate::session::*;
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Default)]
pub struct Options {
    pub screenshot: Option<PathBuf>,
    pub open: Option<PathBuf>,
    pub port: Option<u16>,
    pub no_control: bool,
    pub sample: bool,
    /// Commands run once the window has opened (`--cmd ID [--params JSON]`).
    pub startup: Vec<(String, Value)>,
}

pub fn run(opts: Options) -> Result<(), String> {
    let shot = opts.screenshot.is_some();
    if shot {
        // pictures use the default light look, whatever the person chose
        crate::prefs::EPHEMERAL.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::prefs::update(|p| *p = crate::prefs::Prefs { theme: "light".into(), ..Default::default() });
    }
    let mut vp = egui::ViewportBuilder::default()
        .with_title(crate::APP_NAME)
        .with_inner_size([1440.0, 880.0])
        .with_min_inner_size([760.0, 480.0])
        .with_decorations(false);
    if shot {
        vp = vp.with_active(false);
        #[cfg(windows)]
        {
            vp = vp.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions { viewport: vp, ..Default::default() };
    eframe::run_native(crate::APP_NAME, options, Box::new(move |cc| Ok(Box::new(App::new(cc, opts))))).map_err(|e| e.to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Task,
    Resource,
    Report,
    Project,
    View,
    Format,
    Help,
}

pub struct Theme {
    pub bg: Color32,
    pub panel: Color32,
    pub header: Color32,
    pub grid: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub accent_light: Color32,
    pub select: Color32,
    pub highlight: Color32,
    pub nonwork: Color32,
    pub summary_text: Color32,
    pub danger: Color32,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            bg: Color32::from_rgb(30, 31, 34),
            panel: Color32::from_rgb(42, 43, 46),
            header: Color32::from_rgb(47, 48, 52),
            grid: Color32::from_rgb(62, 63, 68),
            text: Color32::from_rgb(230, 230, 230),
            dim: Color32::from_rgb(150, 150, 150),
            accent: Color32::from_rgb(95, 191, 132),
            accent_light: Color32::from_rgb(46, 74, 58),
            select: Color32::from_rgb(44, 72, 57),
            highlight: Color32::from_rgb(90, 78, 30),
            nonwork: Color32::from_rgb(38, 39, 42),
            summary_text: Color32::WHITE,
            danger: Color32::from_rgb(240, 110, 110),
        }
    } else {
        Theme {
            bg: Color32::WHITE,
            panel: Color32::from_rgb(243, 243, 243),
            header: Color32::from_rgb(240, 240, 240),
            grid: Color32::from_rgb(218, 218, 218),
            text: Color32::from_rgb(32, 32, 32),
            dim: Color32::from_rgb(112, 112, 112),
            accent: Color32::from_rgb(46, 125, 79),
            accent_light: Color32::from_rgb(220, 239, 227),
            select: Color32::from_rgb(207, 230, 216),
            highlight: Color32::from_rgb(255, 242, 179),
            nonwork: Color32::from_rgb(238, 238, 238),
            summary_text: Color32::BLACK,
            danger: Color32::from_rgb(196, 43, 43),
        }
    }
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellEdit {
    /// None = the blank row for a new task/resource.
    pub uid: Option<u32>,
    pub field: String,
    pub buf: String,
    pub focused: bool,
    pub resource: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragMode {
    Undecided,
    Move,
    Resize,
    Link,
}

#[derive(Clone, Debug)]
pub struct BarDrag {
    pub uid: u32,
    pub mode: DragMode,
    pub dx: f32,
    pub dy: f32,
    pub near_right: bool,
}

pub struct PaletteState {
    pub query: String,
    pub selected: Option<String>,
    pub params: String,
    pub result: String,
    pub focus: bool,
}

pub struct App {
    pub s: Session,
    pub reg: &'static Registry,
    pub frame: u64,
    pub screenshot: Option<PathBuf>,
    pub shot_sent: bool,
    pub user_shot: Option<PathBuf>,
    pub tab: RibbonTab,
    pub dialogs: dialogs::Dialogs,
    pub edit: Option<CellEdit>,
    pub status: String,
    pub status_err: bool,
    pub left_days: f64,
    pub drag: Option<BarDrag>,
    pub control_rx: Option<mpsc::Receiver<ControlRequest>>,
    pub control_port: Option<u16>,
    pub control_note: String,
    pub palette: PaletteState,
    pub scroll_to_row: Option<u32>,
    pub last_title: String,
    pub applied_font: f32,
    pub applied_dark: Option<bool>,
    pub shortcuts: Vec<(KeyboardShortcut, String)>,
    pub cal_month: Option<chrono::NaiveDate>,
    pub anchor_row: Option<u32>,
    pub splitter_drag: bool,
    pub recent_reports: Vec<String>,
    pub path_colors: std::collections::HashMap<u32, Color32>,
    pub drawing_drag: Option<(u32, egui::Vec2)>,
    pub bs: Option<backstage::State>,
    pub app_options: Option<options::OptionsState>,
    /// Format Painter picked up a format; the next row clicked gets it.
    pub painter_armed: bool,
    pub prefs_gen: u64,
    pub last_autosave: std::time::Instant,
}

fn parse_shortcut(s: &str) -> Option<KeyboardShortcut> {
    let mut mods = Modifiers::NONE;
    let mut key = None;
    for part in s.split('+') {
        match part {
            "Ctrl" => mods = mods | Modifiers::COMMAND,
            "Shift" => mods = mods | Modifiers::SHIFT,
            "Alt" => mods = mods | Modifiers::ALT,
            "=" => key = Some(Key::Equals),
            "-" => key = Some(Key::Minus),
            "Up" => key = Some(Key::ArrowUp),
            "Down" => key = Some(Key::ArrowDown),
            "Left" => key = Some(Key::ArrowLeft),
            "Right" => key = Some(Key::ArrowRight),
            k => key = Key::from_name(k),
        }
    }
    key.map(|k| KeyboardShortcut::new(mods, k))
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, mut opts: Options) -> App {
        let startup = std::mem::take(&mut opts.startup);
        let had_file = opts.open.is_some() || opts.sample;
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let reg = Registry::get();
        let mut s = if opts.screenshot.is_some() || opts.sample || opts.open.is_none() { crate::sample::sample_session() } else { Session::new(Project::new("Project1", chrono::Local::now().date_naive())) };
        s.headless = false;
        let mut status = "Ready".to_string();
        let mut status_err = false;
        if let Some(path) = &opts.open {
            match reg.run(&mut s, "file.open", &json!({"path": path.to_string_lossy()})) {
                Ok(_) => {
                    status = format!("Opened {}", path.display());
                    crate::recent::add(path);
                }
                Err(e) => {
                    status = e;
                    status_err = true;
                }
            }
            s.headless = false;
        }
        let mut control_rx = None;
        let mut control_port = None;
        let mut control_note = String::new();
        if !opts.no_control && opts.screenshot.is_none() {
            let (tx, rx) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            match control::start_server(opts.port.unwrap_or(crate::DEFAULT_PORT), tx, move || ctx.request_repaint()) {
                Ok(port) => {
                    control_rx = Some(rx);
                    control_port = Some(port);
                }
                Err(e) => control_note = e,
            }
        }
        let shortcuts = reg.commands.iter().filter_map(|c| c.shortcut.as_deref().and_then(parse_shortcut).map(|k| (k, c.id.clone()))).collect();
        let left_days = 0.0;
        let mut app = App {
            s,
            reg,
            frame: 0,
            screenshot: opts.screenshot,
            shot_sent: false,
            user_shot: None,
            tab: RibbonTab::Task,
            dialogs: dialogs::Dialogs::default(),
            edit: None,
            status,
            status_err,
            left_days,
            drag: None,
            control_rx,
            control_port,
            control_note,
            palette: PaletteState { query: String::new(), selected: None, params: "{}".into(), result: String::new(), focus: false },
            scroll_to_row: None,
            last_title: String::new(),
            applied_font: 0.0,
            applied_dark: None,
            shortcuts,
            cal_month: None,
            anchor_row: None,
            splitter_drag: false,
            recent_reports: vec![],
            path_colors: Default::default(),
            drawing_drag: None,
            bs: None,
            app_options: None,
            painter_armed: false,
            prefs_gen: 0,
            last_autosave: std::time::Instant::now(),
        };
        app.left_days = app.days_of(app.s.project.start) - 3.0;
        // fit a short project in the chart on start
        let span = (app.s.project.finish - app.s.project.start).num_days() as f32 + 8.0;
        if span > 0.0 && app.s.view.px_per_day * span > 700.0 {
            let pp = (700.0 / span).clamp(18.0, 28.0);
            app.s.view.px_per_day = pp;
            app.s.view.timescale = if pp >= 18.0 { TimeUnit::Days } else { TimeUnit::Weeks };
        }
        // start-up options (File > Options > General); screenshots keep the defaults
        if app.screenshot.is_none() {
            let p = crate::prefs::get();
            if !had_file {
                let last = crate::recent::list().into_iter().find(|r| std::path::Path::new(&r.path).exists());
                match last {
                    Some(r) if p.open_last_on_startup => app.run("file.open", json!({"path": r.path})),
                    _ => {
                        if let Some(k) = ViewKind::menu_order().into_iter().find(|k| k.id() == p.default_view) {
                            app.s.view.view = k;
                        }
                    }
                }
                if p.show_start_screen {
                    backstage::open(&mut app, "home");
                }
            }
        }
        for (c, p) in startup {
            app.run(&c, p);
        }
        app
    }

    /// Apply app settings (theme, text size, tooltips) when they change, and follow the system
    /// theme when asked to.
    fn apply_prefs(&mut self, ctx: &egui::Context) {
        let p = crate::prefs::get();
        let gen = crate::prefs::GENERATION.load(std::sync::atomic::Ordering::Relaxed);
        let system_dark = ctx.input(|i| i.raw.system_theme).map(|t| t == egui::Theme::Dark);
        let dark = crate::prefs::is_dark(&p, system_dark);
        if gen != self.prefs_gen {
            self.prefs_gen = gen;
            self.s.view.font_size = p.text_size;
            ctx.style_mut(|st| st.interaction.tooltip_delay = if p.screen_tips { 0.5 } else { 1.0e6 });
        }
        if self.s.view.dark != dark {
            self.s.view.dark = dark;
        }
        // auto save the open plan
        if p.autosave_minutes > 0 && self.s.dirty && self.s.path.is_some() && self.last_autosave.elapsed().as_secs() >= p.autosave_minutes as u64 * 60 {
            self.last_autosave = std::time::Instant::now();
            if self.run_value("file.save", json!({})).is_ok() {
                self.status = format!("Auto saved {}", self.s.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
            }
        }
        if p.autosave_minutes > 0 {
            ctx.request_repaint_after(std::time::Duration::from_secs(30));
        }
    }

    /// Chart origin: a fixed day before the project start; x positions count days from here.
    pub fn origin(&self) -> chrono::NaiveDateTime {
        (self.s.project.start.date() - chrono::Duration::days(60)).and_hms_opt(0, 0, 0).unwrap()
    }
    pub fn days_of(&self, t: chrono::NaiveDateTime) -> f64 {
        (t - self.origin()).num_minutes() as f64 / 1440.0
    }
    pub fn date_of_days(&self, d: f64) -> chrono::NaiveDateTime {
        self.origin() + chrono::Duration::minutes((d * 1440.0).round() as i64)
    }

    /// Run a command from the window and show what happened in the status bar.
    pub fn run(&mut self, id: &str, params: Value) {
        let _ = self.run_value(id, params);
    }

    pub fn run_value(&mut self, id: &str, params: Value) -> Result<Value, String> {
        let guarded = matches!(id, "file.new" | "file.open" | "file.close" | "file.newFromTemplate" | "file.newFromExisting");
        if guarded && self.s.dirty && !params.get("force").and_then(|f| f.as_bool()).unwrap_or(false) {
            self.dialogs.open_confirm(id, params);
            return Ok(json!({"waiting": "confirm unsaved changes"}));
        }
        let res = self.reg.run(&mut self.s, id, &params);
        self.s.headless = false;
        match &res {
            Ok(v) => {
                let title = self.reg.find(id).map(|c| c.title.clone()).unwrap_or_else(|| id.to_string());
                self.status = match v.get("ok") {
                    Some(_) => title,
                    None => {
                        let t = v.to_string();
                        if t.len() > 140 {
                            title
                        } else {
                            format!("{title}: {t}")
                        }
                    }
                };
                self.status_err = false;
                if id.starts_with("report.") && id != "report.compareProjects" {
                    self.recent_reports.retain(|r| r != id);
                    self.recent_reports.insert(0, id.to_string());
                    self.recent_reports.truncate(8);
                }
                if id == "format.painter.copy" {
                    self.painter_armed = true;
                    self.status = "Format Painter: click a task to give it this formatting".into();
                }
                if matches!(id, "file.new" | "file.open" | "file.close" | "file.newFromTemplate" | "file.newFromExisting") {
                    self.left_days = self.days_of(self.s.project.start) - 3.0;
                    self.edit = None;
                }
                // the File screen's recent list
                if id == "file.open" && v.get("format").and_then(|f| f.as_str()) == Some("mpp") {
                    if let Some(p) = v.get("opened").and_then(|p| p.as_str()) {
                        crate::recent::add(std::path::Path::new(p));
                    }
                    self.status = format!("Opened {} (read from .mpp; Save keeps a .plan.json copy)", v["opened"].as_str().unwrap_or(""));
                } else if matches!(id, "file.open" | "file.save" | "file.saveAs") && v.get("waiting").is_none() {
                    if let Some(p) = &self.s.path {
                        if p.exists() {
                            crate::recent::add(p);
                        }
                    }
                }
            }
            Err(e) => {
                self.status = e.clone();
                self.status_err = true;
            }
        }
        res
    }

    /// A command chosen from a menu: run it, or open the palette when it needs parameters.
    pub fn invoke(&mut self, cmd: &Command) {
        if cmd.params.iter().any(|p| p.required && p.name != "ids") {
            self.open_palette_with(&cmd.id);
        } else {
            self.run(&cmd.id.clone(), json!({}));
        }
    }

    pub fn open_palette_with(&mut self, id: &str) {
        self.palette.selected = Some(id.to_string());
        self.palette.query = id.to_string();
        self.palette.params = dialogs::params_template(self, id);
        self.palette.result.clear();
        self.palette.focus = false;
        self.dialogs.palette = true;
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        let reqs = std::mem::take(&mut self.s.requests);
        for r in reqs {
            match r {
                UiRequest::Dialog(d) => dialogs::open(self, d),
                UiRequest::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                UiRequest::Screenshot(path) => {
                    let path = path.or_else(|| rfd::FileDialog::new().add_filter("PNG picture", &["png"]).set_file_name("plan.png").save_file());
                    if let Some(p) = path {
                        self.user_shot = Some(p);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                    }
                }
                UiRequest::Backstage(page) => backstage::open(self, &page),
                UiRequest::OpenFile(p) => self.run("file.open", json!({"path": p.to_string_lossy()})),
                UiRequest::ScrollToTask(uid) => self.scroll_to_row = Some(uid),
                UiRequest::Zoom(_) => {}
                UiRequest::RibbonTab(t) => {
                    self.tab = match t.as_str() {
                        "resource" => RibbonTab::Resource,
                        "report" => RibbonTab::Report,
                        "project" => RibbonTab::Project,
                        "view" => RibbonTab::View,
                        "help" => RibbonTab::Help,
                        "format" => RibbonTab::Format,
                        _ => RibbonTab::Task,
                    }
                }
            }
        }
        if let Some(t) = self.s.view.scroll_to.take() {
            self.left_days = self.days_of(t);
        }
    }

    fn handle_control(&mut self) {
        let Some(rx) = &self.control_rx else { return };
        let mut items = vec![];
        while let Ok(r) = rx.try_recv() {
            items.push(r);
        }
        for r in items {
            self.s.headless = false;
            let reply = control::handle_line(&mut self.s, &r.line);
            let req: Value = serde_json::from_str(&r.line).unwrap_or(Value::Null);
            let rep: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
            let cmd = req.get("cmd").or_else(|| req.get("command")).and_then(|c| c.as_str()).unwrap_or("?").to_string();
            let title = self.reg.find(&cmd).map(|c| c.title.clone()).unwrap_or(cmd.clone());
            if rep["ok"].as_bool().unwrap_or(false) {
                self.status = format!("Agent ran {title} ({cmd})");
                self.status_err = false;
            } else {
                self.status = format!("Agent: {title} failed: {}", rep["error"].as_str().unwrap_or(""));
                self.status_err = true;
            }
            let _ = r.reply.send(reply);
        }
    }

    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        let mut images = vec![];
        ctx.input(|i| {
            for e in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = e {
                    images.push(image.clone());
                }
            }
        });
        for image in images {
            let target = if let Some(p) = self.user_shot.take() { Some((p, false)) } else { self.screenshot.clone().map(|p| (p, true)) };
            if let Some((path, exit)) = target {
                let [w, h] = image.size;
                let mut bytes = Vec::with_capacity(w * h * 4);
                for px in &image.pixels {
                    bytes.extend_from_slice(&px.to_array());
                }
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                }
                let res = image::RgbaImage::from_raw(w as u32, h as u32, bytes).map(|img| img.save(&path));
                match res {
                    Some(Ok(())) => self.status = format!("Saved picture {}", path.display()),
                    _ => {
                        self.status = format!("Couldn't save {}", path.display());
                        self.status_err = true;
                    }
                }
                if exit {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() || self.edit.is_some() {
            return;
        }
        let mut fire = vec![];
        ctx.input_mut(|i| {
            for (sc, id) in &self.shortcuts {
                if i.consume_shortcut(sc) {
                    fire.push(id.clone());
                }
            }
        });
        for id in fire {
            if let Some(c) = self.reg.find(&id) {
                let c = c.clone();
                self.invoke(&c);
            }
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::ALT, Key::Q))) {
            self.run("app.commandPalette", json!({}));
        }
        // F2 edits the name of the selected task
        let f2 = ctx.input(|i| i.key_pressed(Key::F2) && i.modifiers.is_none());
        if f2 {
            if let Some(i) = self.s.current_task() {
                let uid = self.s.project.tasks[i].uid;
                self.edit = Some(CellEdit { uid: Some(uid), field: "name".into(), buf: self.s.project.tasks[i].name.clone(), focused: false, resource: false });
            }
        }
        // arrow keys move the selection
        let (up, down) = ctx.input(|i| (i.key_pressed(Key::ArrowUp) && i.modifiers.is_none(), i.key_pressed(Key::ArrowDown) && i.modifiers.is_none()));
        if (up || down) && !self.s.view.view.is_resource_view() {
            let rows: Vec<u32> = self.s.task_rows().into_iter().filter_map(|r| if let Row::Task(i) = r { Some(self.s.project.tasks[i].uid) } else { None }).collect();
            if !rows.is_empty() {
                let cur = self.s.view.selected.iter().next().and_then(|u| rows.iter().position(|r| r == u));
                let next = match cur {
                    Some(k) if up => k.saturating_sub(1),
                    Some(k) => (k + 1).min(rows.len() - 1),
                    None => 0,
                };
                self.s.view.selected = [rows[next]].into_iter().collect();
                self.scroll_to_row = Some(rows[next]);
            }
        }
    }

    fn apply_style(&mut self, ctx: &egui::Context) {
        let dark = self.s.view.dark;
        if self.applied_dark != Some(dark) || self.applied_font != self.s.view.font_size {
            let mut visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
            let th = theme(dark);
            visuals.panel_fill = th.panel;
            visuals.window_fill = if dark { th.panel } else { Color32::WHITE };
            visuals.extreme_bg_color = th.bg;
            // selected tabs, list rows and text: a light accent fill with accent-coloured text
            visuals.selection.bg_fill = if dark { th.accent_light } else { th.select };
            visuals.selection.stroke = egui::Stroke::new(1.0_f32, if dark { Color32::from_rgb(170, 230, 190) } else { th.accent });
            visuals.hyperlink_color = th.accent;
            visuals.widgets.hovered.weak_bg_fill = th.accent_light;
            visuals.widgets.hovered.bg_fill = th.accent_light;
            visuals.widgets.active.weak_bg_fill = th.select;
            // pop-ups, menus and drop-down lists: rounded, softly shadowed
            visuals.window_corner_radius = egui::CornerRadius::same(10);
            visuals.menu_corner_radius = egui::CornerRadius::same(6);
            visuals.window_shadow = egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(if dark { 120 } else { 55 }) };
            visuals.popup_shadow = egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(if dark { 100 } else { 40 }) };
            visuals.window_stroke = egui::Stroke::new(1.0_f32, if dark { Color32::from_gray(70) } else { Color32::from_gray(205) });
            ctx.set_visuals(visuals);
            let fs = self.s.view.font_size;
            ctx.style_mut(|st| {
                use egui::{FontFamily, FontId, TextStyle};
                st.text_styles.insert(TextStyle::Body, FontId::new(fs, FontFamily::Proportional));
                st.text_styles.insert(TextStyle::Button, FontId::new(fs, FontFamily::Proportional));
                st.text_styles.insert(TextStyle::Small, FontId::new(fs - 2.0, FontFamily::Proportional));
                st.text_styles.insert(TextStyle::Heading, FontId::new(fs + 5.0, FontFamily::Proportional));
                st.text_styles.insert(TextStyle::Monospace, FontId::new(fs - 1.0, FontFamily::Monospace));
                st.spacing.item_spacing = egui::vec2(6.0, 4.0);
                st.spacing.button_padding = egui::vec2(6.0, 3.0);
            });
            self.applied_dark = Some(dark);
            self.applied_font = self.s.view.font_size;
        }
    }

    // -----------------------------------------------------------------------------------------
    // Menu bar built from the registry
    // -----------------------------------------------------------------------------------------

    /// The classic menus, folded into one dropdown now that the window draws its own title bar.
    fn all_commands_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(180.0);
        if ui.button("Search commands...    Alt+Q").clicked() {
            ui.close_menu();
            self.run("app.commandPalette", json!({}));
        }
        ui.separator();
        {
            let groups: &[(&str, &[&str])] = &[
                ("File", &["file"]),
                ("Edit", &["edit"]),
                ("View", &["view"]),
                ("Task", &["task"]),
                ("Resource", &["resource", "assignment"]),
                ("Project", &["project", "calendar"]),
                ("Format", &["format"]),
                ("Report", &["report"]),
                ("Help", &["help", "app"]),
            ];
            for (title, cats) in groups {
                ui.menu_button(*title, |ui| {
                    ui.set_min_width(220.0);
                    for (k, cat) in cats.iter().enumerate() {
                        if k > 0 {
                            ui.separator();
                            ui.label(RichText::new(crate::commands::capitalize(cat)).small().color(theme(self.s.view.dark).dim));
                        }
                        let cmds: Vec<&'static Command> = self.reg.commands.iter().filter(|c| c.category == *cat).collect();
                        let tree = build_tree(&cmds, cat);
                        self.render_tree(ui, &tree);
                    }
                });
            }
        }
    }

    fn render_tree(&mut self, ui: &mut egui::Ui, nodes: &[MenuNode]) {
        for n in nodes {
            if n.children.is_empty() {
                if let Some(c) = n.cmd {
                    self.menu_item(ui, c);
                }
            } else {
                if let Some(c) = n.cmd {
                    self.menu_item(ui, c);
                }
                ui.menu_button(format!("{}  ", n.label), |ui| {
                    ui.set_min_width(200.0);
                    egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                        self.render_tree(ui, &n.children);
                    });
                });
            }
        }
    }

    fn menu_item(&mut self, ui: &mut egui::Ui, c: &'static Command) {
        let mut label = c.title.clone();
        if c.params.iter().any(|p| p.required && p.name != "ids") && !label.ends_with("...") {
            label.push_str("...");
        }
        let checked = self.is_checked(&c.id);
        let text = if checked { format!("✔ {label}") } else { format!("    {label}") };
        let mut b = match icons::for_command(&c.id) {
            Some(src) => egui::Button::image_and_text(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)), text),
            None => egui::Button::new(format!("      {text}")),
        }
        .wrap_mode(egui::TextWrapMode::Extend);
        if let Some(sc) = &c.shortcut {
            b = b.shortcut_text(sc.clone());
        }
        let r = ui.add(b).on_hover_text(format!("{}\n\n{}", c.help, c.id));
        if r.clicked() {
            ui.close_menu();
            self.invoke(c);
        }
    }

    /// Whether a toggle-like command reflects the current state (shown with a check mark).
    pub fn is_checked(&self, id: &str) -> bool {
        let v = &self.s.view;
        if let Some(f) = id.strip_prefix("view.toggle.") {
            return crate::commands::view_flag(v, f);
        }
        if let Some(f) = id.strip_prefix("view.show.") {
            return v.view.id() == f;
        }
        if let Some(f) = id.strip_prefix("view.filter.") {
            return v.filter.id() == f;
        }
        if let Some(f) = id.strip_prefix("view.highlight.") {
            return v.highlight.map(|h| h.id() == f).unwrap_or(f == "none");
        }
        if let Some(f) = id.strip_prefix("view.group.") {
            return v.group.id() == f;
        }
        if let Some(f) = id.strip_prefix("view.timescale.") {
            return v.timescale.id() == f;
        }
        if let Some(f) = id.strip_prefix("view.column.show.") {
            return v.columns.iter().any(|c| c == f);
        }
        if let Some(f) = id.strip_prefix("view.resourceColumn.show.") {
            return v.resource_columns.iter().any(|c| c == f);
        }
        if let Some(f) = id.strip_prefix("format.ganttStyle.") {
            return f.parse::<usize>().map(|n| n == v.gantt_style).unwrap_or(false);
        }
        if let Some(rest) = id.strip_prefix("view.sort.") {
            if rest == "none" {
                return v.sort.is_none();
            }
            if let Some((f, d)) = rest.rsplit_once('.') {
                return v.sort.as_ref().map(|(sf, asc)| sf == f && *asc == (d == "asc")).unwrap_or(false);
            }
        }
        if let Some(m) = id.strip_prefix("view.graph.") {
            let cur = if v.graph_mode.is_empty() { "peakUnits" } else { v.graph_mode.as_str() };
            return m == cur;
        }
        if let Some(rest) = id.strip_prefix("project.calendar.") {
            let name = match rest {
                "standard" => "Standard",
                "twentyFourHours" => "24 Hours",
                _ => "Night Shift",
            };
            return self.s.project.calendar == name;
        }
        false
    }

    // -----------------------------------------------------------------------------------------
    // Ribbon
    // -----------------------------------------------------------------------------------------

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        let th = theme(self.s.view.dark);
        ui.horizontal(|ui| {
            // the format tab is named after the view, as in the original ("Gantt Chart Format")
            let format_name = format!("{} Format", self.s.view.view.title());
            // File opens the full-window File screen, as in modern office suites
            let file_btn = egui::Button::new(RichText::new("File").color(th.text)).frame(false).min_size(egui::vec2(48.0, 22.0));
            if ui.add(file_btn).on_hover_text("New, open, save, print, share and export").clicked() {
                backstage::open(self, "home");
            }
            ui.add_space(4.0);
            let hidden_tabs = crate::prefs::get().hidden_tabs;
            for (t, name) in [
                (RibbonTab::Task, "Task"),
                (RibbonTab::Resource, "Resource"),
                (RibbonTab::Report, "Report"),
                (RibbonTab::Project, "Project"),
                (RibbonTab::View, "View"),
                (RibbonTab::Help, "Help"),
                (RibbonTab::Format, format_name.as_str()),
            ] {
                // tabs hidden in File > Options > Customize Ribbon
                let tab_id = match t {
                    RibbonTab::Task => "task",
                    RibbonTab::Resource => "resource",
                    RibbonTab::Report => "report",
                    RibbonTab::Project => "project",
                    RibbonTab::View => "view",
                    RibbonTab::Help => "help",
                    RibbonTab::Format => "format",
                };
                if hidden_tabs.iter().any(|h| h == tab_id) {
                    continue;
                }
                let sel = self.tab == t;
                let txt = RichText::new(name).color(if sel || t == RibbonTab::Format { th.accent } else { th.text });
                let r = ui.add(egui::Button::new(txt).frame(false).min_size(egui::vec2(64.0, 22.0)));
                if sel {
                    let rr = r.rect;
                    ui.painter().line_segment([rr.left_bottom() + egui::vec2(6.0, 0.0), rr.right_bottom() - egui::vec2(6.0, 0.0)], egui::Stroke::new(2.5_f32, th.accent));
                }
                if r.clicked() {
                    self.tab = t;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut q = self.s.view.search.clone();
                let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search tasks").desired_width(160.0));
                if r.changed() {
                    self.run("view.filter.search", json!({"text": q}));
                }
                ui.menu_button("All Commands", |ui| self.all_commands_menu(ui)).response.on_hover_text("Every command, grouped like a classic menu bar");
            });
        });
        ui.add_space(2.0);
        egui::Frame::new().fill(if self.s.view.dark { th.header } else { Color32::from_rgb(250, 250, 250) }).inner_margin(egui::Margin::symmetric(6, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row_h = ui.spacing().interact_size.y + 2.0;
            let items_h = row_h * 3.0 + 6.0;
            // ribbon labels never wrap: when the window is narrow the ribbon scrolls sideways
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::ScrollArea::horizontal().id_salt("ribbon_scroll").auto_shrink([false, true]).max_height(items_h + 26.0).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let groups = ribbon_groups(self.tab, self.s.view.view);
                    for (gi, (gtitle, items)) in groups.iter().enumerate() {
                        if gi > 0 {
                            let x = ui.cursor().left();
                            let top = ui.cursor().top();
                            ui.painter().line_segment([egui::pos2(x + 2.0, top), egui::pos2(x + 2.0, top + items_h + 16.0)], egui::Stroke::new(1.0_f32, th.grid));
                            ui.add_space(8.0);
                        }
                        ui.vertical(|ui| {
                            let r = ui
                                .horizontal_top(|ui| {
                                    // large buttons get a column each; small ones stack three high
                                    let mut small: Vec<&RibbonItem> = vec![];
                                    let flush = |app: &mut App, ui: &mut egui::Ui, small: &mut Vec<&RibbonItem>| {
                                        for chunk in small.chunks(3) {
                                            ui.vertical(|ui| {
                                                for it in chunk {
                                                    app.ribbon_item(ui, it);
                                                }
                                            });
                                        }
                                        small.clear();
                                    };
                                    for it in items {
                                        if matches!(it, RibbonItem::Big(..) | RibbonItem::BigMenu(..) | RibbonItem::BigRecent | RibbonItem::ViewPicker | RibbonItem::StyleGallery | RibbonItem::FontGroup | RibbonItem::BigDisabled(..)) {
                                            flush(self, ui, &mut small);
                                            self.ribbon_item(ui, it);
                                        } else {
                                            small.push(it);
                                        }
                                    }
                                    flush(self, ui, &mut small);
                                })
                                .response;
                            ui.add_space((items_h - r.rect.height()).max(0.0));
                            let w = r.rect.width();
                            ui.allocate_ui_with_layout(egui::vec2(w, 14.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(RichText::new(*gtitle).small().color(th.dim)).wrap_mode(egui::TextWrapMode::Extend));
                            });
                        });
                    }
                });
            });
        });
    }

    /// A large ribbon button: icon above a (possibly two-line) label, optional dropdown arrow.
    fn big_button(&self, ui: &mut egui::Ui, label: &str, icon: Option<egui::ImageSource<'static>>, highlighted: bool, dropdown: bool) -> egui::Response {
        let th = theme(self.s.view.dark);
        let font = egui::FontId::proportional(self.s.view.font_size - 0.5);
        let galley = ui.painter().layout(label.to_string(), font, th.text, 200.0);
        let w = (galley.size().x + if dropdown { 24.0 } else { 12.0 }).max(48.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 72.0), egui::Sense::click());
        if resp.hovered() || highlighted {
            ui.painter().rect_filled(rect, 3.0, if highlighted { th.select } else { th.accent_light });
        }
        let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(32.0, 32.0));
        if let Some(src) = icon {
            egui::Image::new(src).paint_at(ui, icon_rect);
        }
        let tp = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 40.0);
        let text_bottom = tp.y + galley.size().y;
        ui.painter().galley_with_override_text_color(tp, galley, th.text);
        if dropdown {
            let c = egui::pos2(rect.right() - 7.0, text_bottom - 8.0);
            ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 2.5)], th.text, egui::Stroke::NONE));
        }
        resp
    }

    /// Command ids for a dropdown; an entry ending in '.' means "every command starting with this".
    fn expand_cmds(&self, cmds: &[&str]) -> Vec<&'static Command> {
        let mut out = vec![];
        for c in cmds {
            if c.ends_with('.') {
                out.extend(self.reg.commands.iter().filter(|x| x.id.starts_with(c)));
            } else if let Some(x) = self.reg.find(c) {
                out.push(x);
            }
        }
        out
    }

    /// Task tab > Font: typeface, size, bold/italic/underline, background and font colour for the
    /// selected tasks, showing the first selected task's formatting.
    fn font_group(&mut self, ui: &mut egui::Ui) {
        let th = theme(self.s.view.dark);
        let st = self.s.current_task().and_then(|i| self.s.project.tasks[i].style.clone()).unwrap_or_default();
        let mut run: Option<(String, Value)> = None;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            ui.horizontal(|ui| {
                let mono = st.font.as_deref() == Some("mono");
                egui::ComboBox::from_id_salt("rb_font").width(118.0).selected_text(if mono { "Monospace" } else { "Sans" }).show_ui(ui, |ui| {
                    if ui.selectable_label(!mono, "Sans").clicked() {
                        run = Some(("format.font.family".into(), json!({"font": "sans"})));
                    }
                    if ui.selectable_label(mono, "Monospace").clicked() {
                        run = Some(("format.font.family".into(), json!({"font": "mono"})));
                    }
                });
                let size = st.size.unwrap_or(self.s.view.font_size);
                egui::ComboBox::from_id_salt("rb_size").width(48.0).selected_text(crate::model::fmt_num(size as f64)).show_ui(ui, |ui| {
                    for n in [8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0] {
                        if ui.selectable_label((size - n).abs() < 0.1, crate::model::fmt_num(n as f64)).clicked() {
                            run = Some(("format.font.size".into(), json!({"size": n})));
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                let toggle = |ui: &mut egui::Ui, txt: RichText, on: bool, tip: &str| {
                    let b = egui::Button::new(txt).min_size(egui::vec2(26.0, 24.0)).frame(on).fill(if on { th.select } else { Color32::TRANSPARENT });
                    ui.add(b).on_hover_text(tip).clicked()
                };
                if toggle(ui, RichText::new("B").strong(), st.bold.unwrap_or(false), "Bold (Ctrl+B)") {
                    run = Some(("format.font.bold".into(), json!({})));
                }
                if toggle(ui, RichText::new("I").italics(), st.italic.unwrap_or(false), "Italic (Ctrl+I)") {
                    run = Some(("format.font.italic".into(), json!({})));
                }
                if toggle(ui, RichText::new("U").underline(), st.underline.unwrap_or(false), "Underline (Ctrl+U)") {
                    run = Some(("format.font.underline".into(), json!({})));
                }
                ui.add_space(4.0);
                for (cmd, tip, cur, is_bg) in [("format.font.background", "Background Color", st.background, true), ("format.font.color", "Font Color", st.color, false)] {
                    let (r, resp) = ui.allocate_exact_size(egui::vec2(34.0, 24.0), egui::Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 3.0, th.accent_light);
                    }
                    let swatch = cur.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(if is_bg { Color32::from_rgb(255, 255, 0) } else { Color32::from_rgb(220, 30, 30) });
                    let icon = egui::Rect::from_min_size(r.min + egui::vec2(3.0, 2.0), egui::vec2(18.0, 20.0));
                    if is_bg {
                        // a paint bucket: tilted box over a colour bar
                        let p = ui.painter();
                        let c = icon.center() - egui::vec2(0.0, 3.0);
                        p.add(egui::Shape::convex_polygon(vec![c + egui::vec2(-6.0, 0.0), c + egui::vec2(0.0, -6.0), c + egui::vec2(6.0, 0.0), c + egui::vec2(0.0, 6.0)], Color32::WHITE, egui::Stroke::new(1.2_f32, th.text)));
                    } else {
                        ui.painter().text(icon.center() - egui::vec2(0.0, 3.0), egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(15.0), th.text);
                    }
                    ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(icon.left(), icon.bottom() - 4.0), egui::vec2(18.0, 4.0)), 0.0, swatch);
                    let c = egui::pos2(r.right() - 6.0, r.center().y);
                    ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-3.0, -1.5), c + egui::vec2(3.0, -1.5), c + egui::vec2(0.0, 2.0)], th.text, egui::Stroke::NONE));
                    let popup_id = ui.make_persistent_id(("rb_color", cmd));
                    if resp.on_hover_text(tip).clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    let resp2 = ui.interact(r, ui.id().with(("rb_color_r", cmd)), egui::Sense::hover());
                    egui::popup::popup_below_widget(ui, popup_id, &resp2, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(170.0);
                        if ui.button(if is_bg { "No Color" } else { "Automatic" }).clicked() {
                            run = Some((cmd.into(), json!({"color": "automatic"})));
                        }
                        egui::Grid::new(("rb_sw", cmd)).spacing([4.0, 4.0]).show(ui, |ui| {
                            for (k, (n, c)) in more_dialogs::SWATCHES.iter().enumerate() {
                                let (sr, sresp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                                ui.painter().rect_filled(sr, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
                                ui.painter().rect_stroke(sr, 2.0, egui::Stroke::new(1.0_f32, th.grid), egui::StrokeKind::Inside);
                                if sresp.on_hover_text(*n).clicked() {
                                    run = Some((cmd.into(), json!({"color": format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])})));
                                }
                                if k % 7 == 6 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                }
            });
        });
        if let Some((c, p)) = run {
            self.run(&c, p);
        }
    }

    fn ribbon_item(&mut self, ui: &mut egui::Ui, it: &RibbonItem) {
        match it {
            RibbonItem::FontGroup => self.font_group(ui),
            RibbonItem::BigDisabled(label, icon, why) => {
                let th = theme(self.s.view.dark);
                let font = egui::FontId::proportional(self.s.view.font_size - 0.5);
                let galley = ui.painter().layout(label.to_string(), font, th.dim.gamma_multiply(0.8), 200.0);
                let w = (galley.size().x + 12.0).max(48.0);
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 72.0), egui::Sense::hover());
                if let Some(src) = icons::icon(icon) {
                    egui::Image::new(src).tint(Color32::from_white_alpha(140)).paint_at(ui, egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 20.0), egui::vec2(32.0, 32.0)));
                }
                ui.painter().galley(egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 40.0), galley, th.dim);
                resp.on_hover_text(*why);
            }
            RibbonItem::SmallMenu(label, icon, cmds) => {
                let list = self.expand_cmds(cmds);
                let b = match icons::icon(icon) {
                    Some(src) => egui::Button::image_and_text(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)), format!("{label}    ")),
                    None => egui::Button::new(format!("{label}    ")),
                }
                .frame(false);
                let resp = ui.add(b);
                let r = resp.rect;
                let c = egui::pos2(r.right() - 8.0, r.center().y);
                ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 2.5)], theme(self.s.view.dark).text, egui::Stroke::NONE));
                let popup_id = ui.make_persistent_id(("smallmenu", *label));
                if resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(220.0);
                    if *label == "Column Settings" {
                        let col = self.s.view.active_column.clone();
                        let title = crate::fields::task_field(&col).map(|f| f.title).unwrap_or("Task Name");
                        ui.label(RichText::new(format!("Column: {title}")).weak());
                    }
                    for c in list {
                        self.menu_item(ui, c);
                    }
                });
            }
            RibbonItem::StyleGallery => {
                let th = theme(self.s.view.dark);
                let mut chosen = None;
                let current = self.s.view.gantt_style;
                // eight previews in the ribbon (the current style always among them), the rest in a popup
                let first = if current >= 6 { current + 1 - 6 } else { 0 };
                let shown: Vec<usize> = (first..first + 6).collect();
                let gallery = |ui: &mut egui::Ui, which: &[usize], chosen: &mut Option<usize>| {
                    for &n in which {
                        let (name, bc) = gantt_style(n);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(38.0, 46.0), egui::Sense::click());
                        let sel = self.s.view.gantt_style == n;
                        let p = ui.painter();
                        p.rect_filled(rect, 2.0, if sel { th.select } else if resp.hovered() { th.accent_light } else { Color32::WHITE });
                        p.rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, if sel { th.accent } else { th.grid }), egui::StrokeKind::Inside);
                        // a little preview: summary bracket, task bar, milestone
                        let l = rect.left() + 5.0;
                        let r = rect.right() - 5.0;
                        let t = rect.top() + 6.0;
                        p.rect_filled(egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, t + 3.0)), 0.0, rgb(bc.summary));
                        p.line_segment([egui::pos2(l, t), egui::pos2(l, t + 7.0)], egui::Stroke::new(1.5_f32, rgb(bc.summary)));
                        p.line_segment([egui::pos2(r, t), egui::pos2(r, t + 7.0)], egui::Stroke::new(1.5_f32, rgb(bc.summary)));
                        p.rect_filled(egui::Rect::from_min_max(egui::pos2(l + 2.0, t + 10.0), egui::pos2(r - 4.0, t + 20.0)), 1.0, rgb(bc.task));
                        p.rect_filled(egui::Rect::from_min_max(egui::pos2(l + 2.0, t + 13.5), egui::pos2(l + 14.0, t + 16.5)), 0.0, rgb(bc.progress));
                        let c = egui::pos2(r - 6.0, t + 28.0);
                        p.add(egui::Shape::convex_polygon(vec![c + egui::vec2(0.0, -5.0), c + egui::vec2(5.0, 0.0), c + egui::vec2(0.0, 5.0), c + egui::vec2(-5.0, 0.0)], rgb(bc.milestone), egui::Stroke::NONE));
                        if resp.on_hover_text(name).clicked() {
                            *chosen = Some(n);
                        }
                    }
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    gallery(ui, &shown, &mut chosen);
                    // "more styles" button, like the original's gallery arrow
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(18.0, 46.0), egui::Sense::click());
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0_f32, th.grid), egui::StrokeKind::Inside);
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 2.0, th.accent_light);
                    }
                    let c = rect.center();
                    ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 3.0)], th.text, egui::Stroke::NONE));
                    let resp = resp.on_hover_text("All 16 Gantt chart styles");
                    let popup_id = ui.make_persistent_id("style_gallery_more");
                    if resp.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(8.0 * 42.0);
                        for row in [(0..8).collect::<Vec<_>>(), (8..16).collect::<Vec<_>>()] {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                gallery(ui, &row, &mut chosen);
                            });
                        }
                    });
                });
                if let Some(n) = chosen {
                    self.run(&format!("format.ganttStyle.{n}"), json!({}));
                }
            }
            RibbonItem::Big(label, cmd) => {
                let tip = self.reg.find(cmd).map(|c| format!("{}\n{}", c.title, c.help)).unwrap_or_default();
                let checked = self.is_checked(cmd);
                if self.big_button(ui, label, icons::for_command(cmd), checked, false).on_hover_text(tip).clicked() {
                    if let Some(c) = self.reg.find(cmd) {
                        let c = c.clone();
                        self.invoke(&c);
                    }
                }
            }
            RibbonItem::BigMenu(label, icon, cmds) => {
                let list = self.expand_cmds(cmds);
                let resp = self.big_button(ui, label, icons::icon(icon), false, true).on_hover_text(label.replace('\n', " "));
                let popup_id = ui.make_persistent_id(("bigmenu", *label));
                if resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(220.0);
                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        for c in list {
                            self.menu_item(ui, c);
                        }
                    });
                });
            }
            RibbonItem::BigRecent => {
                let resp = self.big_button(ui, "Recent", icons::icon("recent"), false, true).on_hover_text("Reports opened recently");
                let popup_id = ui.make_persistent_id("recent_reports");
                if resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                let recent: Vec<&'static Command> = self.recent_reports.iter().filter_map(|id| self.reg.find(id)).collect();
                egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(200.0);
                    if recent.is_empty() {
                        ui.label(RichText::new("No reports opened yet").weak());
                    }
                    for c in recent {
                        self.menu_item(ui, c);
                    }
                });
            }
            RibbonItem::StatusDate => {
                ui.horizontal(|ui| {
                    if let Some(src) = icons::icon("status_date") {
                        ui.add(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)));
                    }
                    ui.label("Status Date:");
                    let id = ui.make_persistent_id("status_date_edit");
                    let current = self.s.project.status_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "NA".into());
                    let mut buf = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| current.clone());
                    let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(84.0)).on_hover_text("The date progress is measured against; type a date or NA");
                    if r.has_focus() {
                        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                    } else {
                        ui.data_mut(|d| d.remove::<String>(id));
                    }
                    if r.lost_focus() && buf.trim() != current {
                        let v = if buf.trim().eq_ignore_ascii_case("na") { String::new() } else { buf.trim().to_string() };
                        self.run("project.set.statusDate", json!({"value": v}));
                    }
                });
            }
            RibbonItem::ViewPicker => {
                // the current view's name, two lines like the original ("Gantt / Chart")
                let cur = self.s.view.view;
                let title = cur.title();
                let label = match title.split_once(' ') {
                    Some((a, b)) => format!("{a}\n{b}"),
                    None => title.to_string(),
                };
                let icon_cmd = format!("view.show.{}", cur.id());
                let resp = self.big_button(ui, &label, icons::for_command(&icon_cmd), false, true).on_hover_text("Choose a view");
                let popup_id = ui.make_persistent_id("view_picker_popup");
                if resp.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(popup_id));
                }
                let mut chosen: Option<(String, Value)> = None;
                egui::popup::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    ui.set_min_width(200.0);
                    ui.label(RichText::new("Built-In").strong());
                    for vk in ViewKind::menu_order() {
                        let mark = if vk == cur { "✔ " } else { "    " };
                        let id = format!("view.show.{}", vk.id());
                        let b = match icons::for_command(&id) {
                            Some(src) => egui::Button::image_and_text(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)), format!("{mark}{}", vk.title())),
                            None => egui::Button::new(format!("{mark}{}", vk.title())),
                        }
                        .frame(false)
                        .wrap_mode(egui::TextWrapMode::Extend);
                        if ui.add(b).clicked() {
                            chosen = Some((id, json!({})));
                        }
                    }
                    if !self.s.view.saved_views.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Custom").strong());
                        for sv in &self.s.view.saved_views {
                            if ui.add(egui::Button::new(format!("    {}", sv.name)).frame(false)).clicked() {
                                chosen = Some(("view.apply".into(), json!({"name": sv.name})));
                            }
                        }
                    }
                    ui.separator();
                    for (label, cmd) in [("Reset to Default", "view.reset"), ("Save View...", "view.save"), ("More Views...", "view.list")] {
                        let b = match icons::for_command(cmd) {
                            Some(src) => egui::Button::image_and_text(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)), label),
                            None => egui::Button::new(label),
                        }
                        .frame(false);
                        if ui.add(b).clicked() {
                            chosen = Some((cmd.to_string(), json!({})));
                        }
                    }
                });
                if let Some((c, p)) = chosen {
                    self.run(&c, p);
                }
            }
            RibbonItem::IconRow(cmds) => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for cmd in cmds.iter() {
                        let Some(src) = icons::for_command(cmd) else { continue };
                        let tip = self.reg.find(cmd).map(|c| format!("{}\n{}", c.title, c.help)).unwrap_or_default();
                        let b = egui::Button::image(egui::Image::new(src).fit_to_exact_size(egui::vec2(18.0, 18.0))).frame(false);
                        if ui.add(b).on_hover_text(tip).clicked() {
                            if let Some(c) = self.reg.find(cmd) {
                                let c = c.clone();
                                self.invoke(&c);
                            }
                        }
                    }
                });
            }
            RibbonItem::Btn(label, cmd, params) => {
                let tip = self.reg.find(cmd).map(|c| format!("{}\n{}", c.help, c.id)).unwrap_or_default();
                let checked = self.is_checked(cmd);
                let mut b = match icons::for_command(cmd) {
                    Some(src) => egui::Button::image_and_text(egui::Image::new(src).fit_to_exact_size(egui::vec2(16.0, 16.0)), *label),
                    None => egui::Button::new(*label),
                }
                .frame(false)
                .min_size(egui::vec2(0.0, 20.0));
                if checked {
                    b = b.fill(theme(self.s.view.dark).select).frame(true);
                }
                if ui.add(b).on_hover_text(tip).clicked() {
                    let p: Value = serde_json::from_str(params).unwrap_or(json!({}));
                    match self.reg.find(cmd) {
                        Some(c) if p.as_object().map(|o| o.is_empty()).unwrap_or(true) => {
                            let c = c.clone();
                            self.invoke(&c)
                        }
                        _ => self.run(cmd, p),
                    }
                }
            }
            RibbonItem::Toggle(label, flag) => {
                let mut on = crate::commands::view_flag(&self.s.view, flag);
                if ui.checkbox(&mut on, *label).clicked() {
                    self.run(&format!("view.toggle.{flag}"), json!({"on": on}));
                }
            }
            RibbonItem::Menu(label, prefix) => {
                let cmds: Vec<&'static Command> = self.reg.commands.iter().filter(|c| c.id.starts_with(prefix)).collect();
                let current = if prefix.starts_with("view.toggle.") { String::new() } else { cmds.iter().find(|c| self.is_checked(&c.id)).map(|c| short_title(&c.title)).unwrap_or_default() };
                let text = if current.is_empty() { format!("{label}    ") } else { format!("{label}: {current}    ") };
                let ir = ui.menu_button(text, |ui| {
                    egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        for c in cmds {
                            self.menu_item(ui, c);
                        }
                    });
                });
                let r = ir.response.rect;
                let c = egui::pos2(r.right() - 9.0, r.center().y);
                let col = theme(self.s.view.dark).text;
                ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(4.0, -2.0), c + egui::vec2(0.0, 2.5)], col, egui::Stroke::NONE));
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Frame
    // -----------------------------------------------------------------------------------------

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let th = theme(self.s.view.dark);
        ui.horizontal(|ui| {
            let col = if self.status_err { th.danger } else { th.text };
            // long messages are shortened so they never run into the rest of the bar (full text on hover)
            let max_w = (ui.available_width() - 1000.0).max(160.0);
            let shown = gantt::fit_text(ui.painter(), &self.status, &egui::TextStyle::Body.resolve(ui.style()), max_w);
            ui.label(RichText::new(shown).color(col)).on_hover_text(self.status.clone());
            ui.separator();
            ui.label(RichText::new(if self.s.project.settings.new_tasks_manual { "New Tasks: Manually Scheduled" } else { "New Tasks: Auto Scheduled" }).color(th.dim));
            if self.s.view.filter != Filter::All {
                ui.separator();
                ui.label(RichText::new(format!("Filter: {}", self.s.view.filter.title())).color(th.accent));
            }
            if self.s.project.circular {
                ui.separator();
                ui.label(RichText::new("Circular link").color(th.danger));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut z = (self.s.view.px_per_day as f64).ln();
                if ui.add(egui::Slider::new(&mut z, (0.3f64).ln()..=(400.0f64).ln()).show_value(false)).on_hover_text("Zoom the timescale").changed() {
                    let pp = z.exp();
                    self.run("view.zoom.set", json!({"pxPerDay": pp}));
                }
                for (label, id) in [("Report", "report.projectOverview"), ("Resource Sheet", "view.show.resourceSheet"), ("Task Usage", "view.show.taskUsage"), ("Gantt", "view.show.gantt")] {
                    let sel = self.is_checked(id);
                    if ui.selectable_label(sel, label).clicked() {
                        self.run(id, json!({}));
                    }
                }
                ui.separator();
                match self.control_port {
                    Some(p) => ui.label(RichText::new(format!("Agents: 127.0.0.1:{p}")).color(th.dim)).on_hover_text("Scripts and AI agents can drive this window over the JSON control channel or `plancraft-cli mcp --bridge`."),
                    None => ui.label(RichText::new("Agents: off").color(th.dim)).on_hover_text(self.control_note.clone()),
                };
                ui.label(RichText::new(format!("{} commands", self.reg.len())).color(th.dim));
            });
        });
    }

    fn view_bar(&self, ui: &mut egui::Ui) {
        let th = theme(self.s.view.dark);
        let rect = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(rect, 0.0, th.accent);
        let galley = p.layout_no_wrap(self.s.view.view.title().to_uppercase(), egui::FontId::proportional(12.0), Color32::WHITE);
        let pos = egui::pos2(rect.center().x - galley.size().y / 2.0, rect.top() + 14.0 + galley.size().x);
        p.add(egui::epaint::TextShape::new(pos, galley, Color32::WHITE).with_angle(-std::f32::consts::FRAC_PI_2));
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let t = self.s.title();
        if t != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
            self.last_title = t;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        self.apply_prefs(ctx);
        self.apply_style(ctx);
        self.handle_control();
        self.handle_screenshot_events(ctx);
        self.handle_keys(ctx);
        self.handle_requests(ctx);
        self.update_title(ctx);

        let th = theme(self.s.view.dark);
        chrome::resize_edges(ctx);
        egui::TopBottomPanel::top("titlebar").exact_height(chrome::TITLE_H).frame(egui::Frame::new().fill(th.panel)).show(ctx, |ui| chrome::title_bar(self, ui));
        if self.bs.is_some() {
            backstage::show(self, ctx);
            dialogs::show(self, ctx);
            options::show(self, ctx);
            if !self.s.requests.is_empty() {
                self.handle_requests(ctx);
            }
            chrome::window_border(ctx, self.s.view.dark);
            self.screenshot_tick(ctx);
            return;
        }
        egui::TopBottomPanel::top("ribbon").frame(egui::Frame::new().fill(th.panel).inner_margin(egui::Margin::symmetric(6, 2))).show(ctx, |ui| self.ribbon(ui));
        if crate::prefs::get().show_status_bar {
            egui::TopBottomPanel::bottom("status").frame(egui::Frame::new().fill(th.panel).inner_margin(egui::Margin::symmetric(8, 3))).show(ctx, |ui| self.status_bar(ui));
        }
        let task_view = !self.s.view.view.is_resource_view() && self.s.view.view != ViewKind::Timeline;
        if self.s.view.show_details && task_view {
            egui::TopBottomPanel::bottom("details").resizable(true).default_height(170.0).frame(egui::Frame::new().fill(th.bg).inner_margin(egui::Margin::same(6))).show(ctx, |ui| views::details_pane(self, ui));
        }
        egui::SidePanel::left("viewbar").exact_width(22.0).resizable(false).frame(egui::Frame::new()).show(ctx, |ui| self.view_bar(ui));
        let gantt_like = matches!(self.s.view.view, ViewKind::Gantt | ViewKind::TrackingGantt | ViewKind::TaskSheet | ViewKind::TaskUsage);
        if self.s.view.show_timeline && gantt_like {
            egui::TopBottomPanel::top("timeline").exact_height(70.0).frame(egui::Frame::new().fill(th.bg)).show(ctx, |ui| views::timeline_strip(self, ui, false));
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(th.bg)).show(ctx, |ui| match self.s.view.view {
            ViewKind::Gantt => gantt::task_view(self, ui, gantt::Mode::Gantt),
            ViewKind::TrackingGantt => gantt::task_view(self, ui, gantt::Mode::Tracking),
            ViewKind::TaskSheet => gantt::task_view(self, ui, gantt::Mode::Sheet),
            ViewKind::TaskUsage => gantt::task_view(self, ui, gantt::Mode::Usage),
            ViewKind::ResourceSheet => views::resource_view(self, ui, false),
            ViewKind::ResourceUsage => views::resource_view(self, ui, true),
            ViewKind::NetworkDiagram => views::network_view(self, ui),
            ViewKind::Calendar => views::calendar_view(self, ui),
            ViewKind::Timeline => views::timeline_strip(self, ui, true),
            ViewKind::TaskForm => more_views::task_form_view(self, ui),
            ViewKind::ResourceForm => more_views::resource_form_view(self, ui),
            ViewKind::ResourceGraph => more_views::resource_graph_view(self, ui),
            ViewKind::TeamPlanner => more_views::team_planner_view(self, ui),
        });
        dialogs::show(self, ctx);
        options::show(self, ctx);
        // dialogs may have queued more requests
        if !self.s.requests.is_empty() {
            self.handle_requests(ctx);
        }
        chrome::window_border(ctx, self.s.view.dark);
        self.screenshot_tick(ctx);
    }
}

impl App {
    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        if let Some(_) = &self.screenshot {
            ctx.request_repaint();
            if self.frame >= 120 && !self.shot_sent {
                self.shot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            if self.frame > 600 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Menu tree
// ---------------------------------------------------------------------------------------------

pub struct MenuNode {
    pub key: String,
    pub label: String,
    pub cmd: Option<&'static Command>,
    pub children: Vec<MenuNode>,
}

pub fn pretty(seg: &str) -> String {
    let mut out = String::new();
    for (i, ch) in seg.chars().enumerate() {
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

fn short_title(t: &str) -> String {
    t.split_once(": ").map(|(_, b)| b.to_string()).unwrap_or_else(|| t.to_string())
}

fn build_tree(cmds: &[&'static Command], cat: &str) -> Vec<MenuNode> {
    let mut roots: Vec<MenuNode> = vec![];
    for c in cmds {
        let rest = c.id.strip_prefix(cat).unwrap_or(&c.id).trim_start_matches('.');
        let segs: Vec<&str> = rest.split('.').collect();
        let mut level = &mut roots;
        for (k, seg) in segs.iter().enumerate() {
            let pos = match level.iter().position(|n| n.key == *seg) {
                Some(p) => p,
                None => {
                    level.push(MenuNode { key: seg.to_string(), label: pretty(seg), cmd: None, children: vec![] });
                    level.len() - 1
                }
            };
            if k == segs.len() - 1 {
                level[pos].cmd = Some(c);
            }
            level = &mut level[pos].children;
        }
    }
    fn collapse(nodes: Vec<MenuNode>) -> Vec<MenuNode> {
        nodes
            .into_iter()
            .map(|mut n| {
                n.children = collapse(n.children);
                if n.cmd.is_none() && n.children.len() == 1 && n.children[0].children.is_empty() {
                    let only = n.children.pop().unwrap();
                    return only;
                }
                n
            })
            .collect()
    }
    collapse(roots)
}

