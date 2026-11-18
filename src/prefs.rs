//! App-wide preferences (File > Options): theme, start-up, quick access toolbar, ribbon tabs,
//! saving, date style. Kept in the user's settings folder, separate from plan files, and changed
//! only through the `app.options.*` commands so scripts and agents can set them too.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    /// "system", "light" or "dark".
    pub theme: String,
    /// Text size in points.
    pub text_size: f32,
    pub user_name: String,
    pub initials: String,
    /// View id new plans open in.
    pub default_view: String,
    /// Show the File screen's Home page when the window opens.
    pub show_start_screen: bool,
    /// Show command descriptions in button tooltips.
    pub screen_tips: bool,
    /// "us" (11/16/26), "uk" (16/11/26) or "iso" (2026-11-16).
    pub date_style: String,
    pub language: String,
    /// "plan.json" or "xml".
    pub save_format: String,
    /// Folder offered first by Save As (empty: Documents).
    pub default_folder: String,
    /// Save the open plan every this many minutes (0 = off).
    pub autosave_minutes: u32,
    pub undo_levels: usize,
    pub recent_count: usize,
    pub open_last_on_startup: bool,
    pub show_status_bar: bool,
    /// Ribbon tabs that are hidden ("resource", "report", ...).
    pub hidden_tabs: Vec<String>,
    /// Command ids shown in the title bar's quick access toolbar.
    pub quick_access: Vec<String>,
    pub show_quick_access: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            theme: "system".into(),
            text_size: 13.0,
            user_name: String::new(),
            initials: String::new(),
            default_view: "gantt".into(),
            show_start_screen: false,
            screen_tips: true,
            date_style: "us".into(),
            language: "en-US".into(),
            save_format: "plan.json".into(),
            default_folder: String::new(),
            autosave_minutes: 0,
            undo_levels: 100,
            recent_count: 30,
            open_last_on_startup: false,
            show_status_bar: true,
            hidden_tabs: vec![],
            quick_access: vec!["file.save".into(), "edit.undo".into(), "edit.redo".into()],
            show_quick_access: true,
        }
    }
}

pub const THEMES: &[(&str, &str)] = &[("system", "Use system setting"), ("light", "Light"), ("dark", "Dark")];
pub const DATE_STYLES: &[(&str, &str)] = &[("us", "Month/day/year (11/16/26)"), ("uk", "Day/month/year (16/11/26)"), ("iso", "Year-month-day (2026-11-16)")];
pub const LANGUAGES: &[(&str, &str)] = &[("en-US", "English (United States)")];
pub const SAVE_FORMATS: &[(&str, &str)] = &[("plan.json", "PlanCraft plan (*.plan.json)"), ("xml", "XML interchange (*.xml)")];
pub const RIBBON_TABS: &[(&str, &str)] = &[("task", "Task"), ("resource", "Resource"), ("report", "Report"), ("project", "Project"), ("view", "View"), ("help", "Help"), ("format", "Format")];

static CACHE: Mutex<Option<Prefs>> = Mutex::new(None);
/// Bumped on every change, so the window knows to apply it.
pub static GENERATION: AtomicU64 = AtomicU64::new(1);
/// When true (screenshots, tests), the settings file is neither read nor written.
pub static EPHEMERAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn store() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PLANCRAFT_SETTINGS") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::APP_NAME).join("settings.json"))
}

pub fn get() -> Prefs {
    let mut c = CACHE.lock().unwrap();
    if c.is_none() {
        let p = if EPHEMERAL.load(Ordering::Relaxed) {
            Prefs::default()
        } else {
            store().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
        };
        apply_globals(&p);
        *c = Some(p);
    }
    c.clone().unwrap()
}

fn apply_globals(p: &Prefs) {
    let style = match p.date_style.as_str() {
        "uk" => 1,
        "iso" => 2,
        _ => 0,
    };
    crate::model::DATE_STYLE.store(style, Ordering::Relaxed);
}

/// Change the preferences, save them and tell the window.
pub fn update(f: impl FnOnce(&mut Prefs)) -> Prefs {
    let mut p = get();
    f(&mut p);
    apply_globals(&p);
    if !EPHEMERAL.load(Ordering::Relaxed) {
        if let Some(path) = store() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, serde_json::to_string_pretty(&p).unwrap_or_default());
        }
    }
    *CACHE.lock().unwrap() = Some(p.clone());
    GENERATION.fetch_add(1, Ordering::Relaxed);
    p
}

fn one_of(v: &str, list: &[(&str, &str)], what: &str) -> Result<String, String> {
    list.iter()
        .find(|(k, label)| k.eq_ignore_ascii_case(v) || label.eq_ignore_ascii_case(v))
        .map(|(k, _)| k.to_string())
        .ok_or_else(|| format!("{what} must be one of: {}", list.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")))
}

fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::String(s) => match s.to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Ok(true),
            "false" | "no" | "off" | "0" => Ok(false),
            _ => Err(format!("'{s}' is not true or false")),
        },
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0) != 0.0),
        _ => Err("expected true or false".into()),
    }
}

fn as_num(v: &Value) -> Result<f64, String> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| "not a number".into()),
        Value::String(s) => s.trim().parse().map_err(|_| format!("'{s}' is not a number")),
        _ => Err("expected a number".into()),
    }
}

fn as_list(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Array(a) => Ok(a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()),
        Value::String(s) => Ok(s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()),
        _ => Err("expected a list".into()),
    }
}

/// Every key with its current value and what it accepts.
pub fn describe() -> Value {
    let p = get();
    json!({
        "settings": p,
        "accepts": {
            "theme": THEMES.iter().map(|x| x.0).collect::<Vec<_>>(),
            "dateStyle": DATE_STYLES.iter().map(|x| x.0).collect::<Vec<_>>(),
            "language": LANGUAGES.iter().map(|x| x.0).collect::<Vec<_>>(),
            "saveFormat": SAVE_FORMATS.iter().map(|x| x.0).collect::<Vec<_>>(),
            "hiddenTabs": RIBBON_TABS.iter().map(|x| x.0).collect::<Vec<_>>(),
            "textSize": "9 to 24",
            "autosaveMinutes": "0 (off) to 120",
            "undoLevels": "1 to 1000",
            "recentCount": "0 to 50",
            "quickAccess": "command ids, e.g. [\"file.save\",\"edit.undo\"]",
        }
    })
}

/// Set one preference by its key (camelCase, as `describe` lists them).
pub fn set(key: &str, v: &Value) -> Result<Prefs, String> {
    let s = || v.as_str().map(|x| x.to_string()).unwrap_or_else(|| v.to_string());
    // validate first, then write
    let mut next = get();
    match key {
        "theme" => next.theme = one_of(&s(), THEMES, "theme")?,
        "textSize" => next.text_size = (as_num(v)? as f32).clamp(9.0, 24.0),
        "userName" => next.user_name = s(),
        "initials" => next.initials = s(),
        "defaultView" => {
            let id = s();
            if !crate::session::ViewKind::menu_order().iter().any(|k| k.id() == id) {
                return Err(format!("no view '{id}'"));
            }
            next.default_view = id;
        }
        "showStartScreen" => next.show_start_screen = as_bool(v)?,
        "screenTips" => next.screen_tips = as_bool(v)?,
        "dateStyle" => next.date_style = one_of(&s(), DATE_STYLES, "dateStyle")?,
        "language" => next.language = one_of(&s(), LANGUAGES, "language")?,
        "saveFormat" => next.save_format = one_of(&s(), SAVE_FORMATS, "saveFormat")?,
        "defaultFolder" => next.default_folder = s(),
        "autosaveMinutes" => next.autosave_minutes = as_num(v)?.clamp(0.0, 120.0) as u32,
        "undoLevels" => next.undo_levels = as_num(v)?.clamp(1.0, 1000.0) as usize,
        "recentCount" => next.recent_count = as_num(v)?.clamp(0.0, 50.0) as usize,
        "openLastOnStartup" => next.open_last_on_startup = as_bool(v)?,
        "showStatusBar" => next.show_status_bar = as_bool(v)?,
        "hiddenTabs" => {
            let l = as_list(v)?;
            for t in &l {
                one_of(t, RIBBON_TABS, "hiddenTabs")?;
            }
            next.hidden_tabs = l;
        }
        "quickAccess" => {
            let l = as_list(v)?;
            let reg = crate::commands::Registry::get();
            for c in &l {
                if reg.find(c).is_none() {
                    return Err(format!("no command '{c}'"));
                }
            }
            next.quick_access = l;
        }
        "showQuickAccess" => next.show_quick_access = as_bool(v)?,
        _ => return Err(format!("no option '{key}' (see app.options.get)")),
    }
    Ok(update(|p| *p = next))
}

/// Whether the window should be dark, given the operating system's theme.
pub fn is_dark(p: &Prefs, system_dark: Option<bool>) -> bool {
    match p.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => system_dark.unwrap_or(false),
    }
}
