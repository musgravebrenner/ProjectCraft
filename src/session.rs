//! A session: the open project plus everything about how it is being viewed, undo history and
//! requests for the window (dialogs, reports). Commands act on a session, with or without a window.

use crate::fields;
use crate::model::*;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum ViewKind {
    #[default]
    Gantt,
    TrackingGantt,
    TaskSheet,
    TaskUsage,
    NetworkDiagram,
    Calendar,
    ResourceSheet,
    ResourceUsage,
    Timeline,
    TaskForm,
    ResourceForm,
    ResourceGraph,
    TeamPlanner,
}

impl ViewKind {
    pub const ALL: [ViewKind; 13] = [
        ViewKind::Gantt,
        ViewKind::TrackingGantt,
        ViewKind::TaskSheet,
        ViewKind::TaskUsage,
        ViewKind::NetworkDiagram,
        ViewKind::Calendar,
        ViewKind::ResourceSheet,
        ViewKind::ResourceUsage,
        ViewKind::Timeline,
        ViewKind::TaskForm,
        ViewKind::ResourceForm,
        ViewKind::ResourceGraph,
        ViewKind::TeamPlanner,
    ];
    /// Built-in views in the order the view dropdown lists them (alphabetical, as the original).
    pub fn menu_order() -> Vec<ViewKind> {
        let mut v = ViewKind::ALL.to_vec();
        v.sort_by_key(|k| k.title());
        v
    }
    pub fn id(self) -> &'static str {
        match self {
            ViewKind::Gantt => "gantt",
            ViewKind::TrackingGantt => "trackingGantt",
            ViewKind::TaskSheet => "taskSheet",
            ViewKind::TaskUsage => "taskUsage",
            ViewKind::NetworkDiagram => "networkDiagram",
            ViewKind::Calendar => "calendar",
            ViewKind::ResourceSheet => "resourceSheet",
            ViewKind::ResourceUsage => "resourceUsage",
            ViewKind::Timeline => "timeline",
            ViewKind::TaskForm => "taskForm",
            ViewKind::ResourceForm => "resourceForm",
            ViewKind::ResourceGraph => "resourceGraph",
            ViewKind::TeamPlanner => "teamPlanner",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            ViewKind::Gantt => "Gantt Chart",
            ViewKind::TrackingGantt => "Tracking Gantt",
            ViewKind::TaskSheet => "Task Sheet",
            ViewKind::TaskUsage => "Task Usage",
            ViewKind::NetworkDiagram => "Network Diagram",
            ViewKind::Calendar => "Calendar",
            ViewKind::ResourceSheet => "Resource Sheet",
            ViewKind::ResourceUsage => "Resource Usage",
            ViewKind::Timeline => "Timeline",
            ViewKind::TaskForm => "Task Form",
            ViewKind::ResourceForm => "Resource Form",
            ViewKind::ResourceGraph => "Resource Graph",
            ViewKind::TeamPlanner => "Team Planner",
        }
    }
    pub fn is_resource_view(self) -> bool {
        matches!(self, ViewKind::ResourceSheet | ViewKind::ResourceUsage | ViewKind::ResourceForm | ViewKind::ResourceGraph | ViewKind::TeamPlanner)
    }
}

/// A view saved under a name with Save View (its table, filter, group, sort and zoom).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SavedView {
    pub name: String,
    pub view: ViewKind,
    pub columns: Vec<String>,
    pub filter: Filter,
    pub highlight: Option<Filter>,
    pub group: Group,
    pub sort: Option<(String, bool)>,
    pub timescale: TimeUnit,
    pub px_per_day: f32,
    pub outline_level: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum TimeUnit {
    Hours,
    #[default]
    Days,
    Weeks,
    Months,
    Quarters,
    Years,
}

impl TimeUnit {
    pub const ALL: [TimeUnit; 6] = [TimeUnit::Hours, TimeUnit::Days, TimeUnit::Weeks, TimeUnit::Months, TimeUnit::Quarters, TimeUnit::Years];
    pub fn id(self) -> &'static str {
        match self {
            TimeUnit::Hours => "hours",
            TimeUnit::Days => "days",
            TimeUnit::Weeks => "weeks",
            TimeUnit::Months => "months",
            TimeUnit::Quarters => "quarters",
            TimeUnit::Years => "years",
        }
    }
    /// Pixels per day that suit this timescale.
    pub fn px_per_day(self) -> f32 {
        match self {
            TimeUnit::Hours => 240.0,
            TimeUnit::Days => 28.0,
            TimeUnit::Weeks => 14.0,
            TimeUnit::Months => 4.0,
            TimeUnit::Quarters => 1.6,
            TimeUnit::Years => 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum Filter {
    #[default]
    All,
    Active,
    Inactive,
    Completed,
    Incomplete,
    InProgress,
    NotStarted,
    Critical,
    Milestones,
    Summary,
    Late,
    Manual,
    WithDeadline,
    Unassigned,
    NegativeSlack,
}

impl Filter {
    pub const ALL: [Filter; 15] = [
        Filter::All,
        Filter::Active,
        Filter::Inactive,
        Filter::Completed,
        Filter::Incomplete,
        Filter::InProgress,
        Filter::NotStarted,
        Filter::Critical,
        Filter::Milestones,
        Filter::Summary,
        Filter::Late,
        Filter::Manual,
        Filter::WithDeadline,
        Filter::Unassigned,
        Filter::NegativeSlack,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Active => "active",
            Filter::Inactive => "inactive",
            Filter::Completed => "completed",
            Filter::Incomplete => "incomplete",
            Filter::InProgress => "inProgress",
            Filter::NotStarted => "notStarted",
            Filter::Critical => "critical",
            Filter::Milestones => "milestones",
            Filter::Summary => "summary",
            Filter::Late => "late",
            Filter::Manual => "manual",
            Filter::WithDeadline => "withDeadline",
            Filter::Unassigned => "unassigned",
            Filter::NegativeSlack => "negativeSlack",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Filter::All => "All Tasks",
            Filter::Active => "Active Tasks",
            Filter::Inactive => "Inactive Tasks",
            Filter::Completed => "Completed Tasks",
            Filter::Incomplete => "Incomplete Tasks",
            Filter::InProgress => "Tasks In Progress",
            Filter::NotStarted => "Tasks Not Started",
            Filter::Critical => "Critical",
            Filter::Milestones => "Milestones",
            Filter::Summary => "Summary Tasks",
            Filter::Late => "Late Tasks",
            Filter::Manual => "Manually Scheduled Tasks",
            Filter::WithDeadline => "Tasks With Deadlines",
            Filter::Unassigned => "Tasks Without Resources",
            Filter::NegativeSlack => "Negative Slack",
        }
    }
    pub fn matches(self, p: &Project, i: usize) -> bool {
        let t = &p.tasks[i];
        match self {
            Filter::All => true,
            Filter::Active => t.active,
            Filter::Inactive => !t.active,
            Filter::Completed => t.percent_complete >= 100.0,
            Filter::Incomplete => t.percent_complete < 100.0,
            Filter::InProgress => t.percent_complete > 0.0 && t.percent_complete < 100.0,
            Filter::NotStarted => t.percent_complete <= 0.0,
            Filter::Critical => t.critical,
            Filter::Milestones => t.is_milestone(),
            Filter::Summary => t.summary,
            Filter::Late => t.late || t.missed_deadline,
            Filter::Manual => t.manual,
            Filter::WithDeadline => t.deadline.is_some(),
            Filter::Unassigned => !t.summary && p.assignments_of_task(t.uid).next().is_none(),
            Filter::NegativeSlack => t.total_slack < 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, Hash)]
pub enum Group {
    #[default]
    None,
    Critical,
    Milestones,
    Priority,
    Resource,
    Complete,
    ConstraintType,
    TaskType,
    Duration,
    Mode,
}

impl Group {
    pub const ALL: [Group; 10] = [
        Group::None,
        Group::Critical,
        Group::Milestones,
        Group::Priority,
        Group::Resource,
        Group::Complete,
        Group::ConstraintType,
        Group::TaskType,
        Group::Duration,
        Group::Mode,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Group::None => "none",
            Group::Critical => "critical",
            Group::Milestones => "milestones",
            Group::Priority => "priority",
            Group::Resource => "resource",
            Group::Complete => "complete",
            Group::ConstraintType => "constraintType",
            Group::TaskType => "taskType",
            Group::Duration => "duration",
            Group::Mode => "mode",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Group::None => "No Group",
            Group::Critical => "Critical",
            Group::Milestones => "Milestones",
            Group::Priority => "Priority",
            Group::Resource => "Resource",
            Group::Complete => "Complete and Incomplete Tasks",
            Group::ConstraintType => "Constraint Type",
            Group::TaskType => "Task Type",
            Group::Duration => "Duration",
            Group::Mode => "Auto Scheduled v. Manually Scheduled",
        }
    }
    pub fn key(self, p: &Project, i: usize) -> String {
        let t = &p.tasks[i];
        match self {
            Group::None => String::new(),
            Group::Critical => format!("Critical: {}", if t.critical { "Yes" } else { "No" }),
            Group::Milestones => format!("Milestone: {}", if t.is_milestone() { "Yes" } else { "No" }),
            Group::Priority => format!("Priority: {}", t.priority),
            Group::Resource => {
                let r = p.resource_names(t.uid);
                if r.is_empty() { "No Resource".into() } else { format!("Resource: {r}") }
            }
            Group::Complete => match t.percent_complete {
                x if x >= 100.0 => "% Complete: 100%".into(),
                x if x <= 0.0 => "% Complete: 0%".into(),
                _ => "% Complete: 1% - 99%".into(),
            },
            Group::ConstraintType => format!("Constraint Type: {}", t.constraint.title()),
            Group::TaskType => format!("Type: {}", t.task_type.title()),
            Group::Duration => format!("Duration: {}", t.duration.format(&p.settings)),
            Group::Mode => format!("Task Mode: {}", if t.manual { "Manually Scheduled" } else { "Auto Scheduled" }),
        }
    }
}

pub const SORT_FIELDS: &[&str] = &[
    "id", "name", "start", "finish", "duration", "priority", "cost", "work", "percentComplete", "totalSlack", "deadline", "constraintType",
    "resourceNames",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BarColors {
    pub task: [u8; 3],
    pub summary: [u8; 3],
    pub milestone: [u8; 3],
    pub critical: [u8; 3],
    pub progress: [u8; 3],
    pub baseline: [u8; 3],
    pub manual: [u8; 3],
}

impl Default for BarColors {
    fn default() -> Self {
        BarColors {
            task: [138, 180, 230],
            summary: [70, 70, 70],
            milestone: [40, 40, 40],
            critical: [232, 106, 106],
            progress: [40, 76, 130],
            baseline: [168, 168, 168],
            manual: [120, 196, 196],
        }
    }
}

pub const GANTT_STYLES: &[(&str, BarColors)] = &[];

/// The Gantt chart style gallery: (name, task bar, summary bar, milestone, progress).
const STYLE_TABLE: [(&str, [u8; 3], [u8; 3], [u8; 3], [u8; 3]); 16] = [
    ("Classic Blue", [138, 180, 230], [70, 70, 70], [40, 40, 40], [40, 76, 130]),
    ("Sky", [120, 196, 220], [60, 140, 170], [40, 150, 180], [30, 100, 130]),
    ("Raspberry", [210, 90, 150], [120, 40, 90], [190, 30, 110], [120, 30, 80]),
    ("Plum", [170, 110, 190], [70, 50, 100], [150, 70, 170], [90, 50, 120]),
    ("Slate", [150, 160, 176], [80, 90, 110], [90, 90, 100], [60, 70, 90]),
    ("Lagoon", [120, 200, 210], [70, 70, 70], [40, 160, 160], [30, 120, 130]),
    ("Teal", [70, 170, 170], [40, 110, 140], [40, 150, 140], [20, 100, 100]),
    ("Lime", [180, 210, 80], [120, 150, 50], [130, 160, 40], [90, 120, 30]),
    ("Leaf", [90, 180, 90], [40, 100, 160], [60, 140, 60], [40, 100, 40]),
    ("Ocean", [40, 140, 170], [30, 80, 120], [30, 140, 160], [20, 90, 110]),
    ("Forest", [60, 140, 70], [40, 90, 150], [110, 110, 110], [30, 90, 40]),
    ("Amber", [240, 180, 60], [200, 130, 30], [210, 150, 30], [160, 110, 20]),
    ("Olive", [130, 160, 70], [60, 100, 50], [90, 130, 40], [70, 90, 30]),
    ("Royal", [60, 110, 200], [30, 60, 140], [20, 20, 20], [30, 60, 140]),
    ("Violet", [120, 90, 190], [60, 50, 120], [20, 20, 20], [70, 50, 130]),
    ("Midnight", [90, 100, 150], [40, 40, 80], [20, 20, 20], [40, 40, 90]),
];

pub fn gantt_style(n: usize) -> (&'static str, BarColors) {
    let d = BarColors::default();
    let (name, task, summary, milestone, progress) = STYLE_TABLE[n.min(STYLE_TABLE.len() - 1)];
    (name, BarColors { task, summary, milestone, progress, ..d })
}
pub const GANTT_STYLE_COUNT: usize = 16;

/// A shape drawn on the Gantt chart (Format > Drawing): placed at a date, on a task's row or a
/// row number.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drawing {
    pub id: u32,
    /// text, box, line or arrow
    pub kind: String,
    pub date: NaiveDateTime,
    /// The task whose row it sits on (moves with the task's row), else `row`.
    pub task: Option<u32>,
    pub row: f32,
    pub width_days: f32,
    pub height_rows: f32,
    pub text: String,
    pub color: [u8; 3],
}

fn yes_default() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewState {
    pub view: ViewKind,
    pub selected: BTreeSet<u32>,
    pub selected_resources: BTreeSet<u32>,
    pub timescale: TimeUnit,
    pub px_per_day: f32,
    pub filter: Filter,
    pub highlight: Option<Filter>,
    pub group: Group,
    pub sort: Option<(String, bool)>,
    pub columns: Vec<String>,
    pub resource_columns: Vec<String>,
    pub outline_level: Option<u8>,
    pub show_timeline: bool,
    pub show_critical: bool,
    pub show_baseline: bool,
    pub show_slack: bool,
    pub show_late: bool,
    pub show_outline_numbers: bool,
    pub show_project_summary: bool,
    pub show_gridlines: bool,
    pub show_nonworking: bool,
    pub show_links: bool,
    pub show_bar_text: bool,
    pub show_progress: bool,
    pub show_details: bool,
    pub show_today: bool,
    pub dark: bool,
    pub font_size: f32,
    pub row_height: f32,
    pub table_width: f32,
    pub bar_colors: BarColors,
    pub gantt_style: usize,
    pub scroll_to: Option<NaiveDateTime>,
    pub search: String,
    #[serde(default)]
    pub saved_views: Vec<SavedView>,
    /// What the Resource Graph shows (peakUnits, work, cumulativeWork, overallocation, percentAllocation, remainingAvailability, cost, cumulativeCost).
    #[serde(default)]
    pub graph_mode: String,
    /// Column alignment by field id: left, center or right.
    #[serde(default)]
    pub col_align: std::collections::BTreeMap<String, String>,
    /// Column titles chosen in Field Settings.
    #[serde(default)]
    pub col_titles: std::collections::BTreeMap<String, String>,
    /// Column widths set by Best Fit.
    #[serde(default)]
    pub col_widths: std::collections::BTreeMap<String, f32>,
    /// The column last clicked (alignment and column settings act on it).
    #[serde(default)]
    pub active_column: String,
    #[serde(default = "yes_default")]
    pub show_summary_tasks: bool,
    /// Task Path highlighting: predecessors, drivingPredecessors, successors, drivenSuccessors.
    #[serde(default)]
    pub task_path: Vec<String>,
    /// Which baseline the chart draws (0 = Baseline, 1-10).
    #[serde(default)]
    pub baseline_shown: usize,
    /// Slippage lines against this baseline.
    #[serde(default)]
    pub slippage: Option<usize>,
    /// Link lines: elbow (default), straight or none.
    #[serde(default)]
    pub link_style: String,
    /// Bar height: small, medium (default) or large.
    #[serde(default)]
    pub bar_size: String,
    /// Dates beside milestones: short (default), long or none.
    #[serde(default)]
    pub bar_date_format: String,
    #[serde(default)]
    pub drawings: Vec<Drawing>,
    /// Text Styles dialog: formatting by kind of row ("all", "critical", "milestone", "summary",
    /// "inactive", "noncritical").
    #[serde(default)]
    pub text_styles: std::collections::BTreeMap<String, crate::model::TextStyle>,
    #[serde(default)]
    pub print: crate::print::PrintSettings,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            view: ViewKind::Gantt,
            selected: BTreeSet::new(),
            selected_resources: BTreeSet::new(),
            timescale: TimeUnit::Days,
            px_per_day: TimeUnit::Days.px_per_day(),
            filter: Filter::All,
            highlight: None,
            group: Group::None,
            sort: None,
            columns: default_columns(),
            resource_columns: ["id", "name", "type", "initials", "group", "maxUnits", "stdRate", "ovtRate", "costPerUse", "calendar"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            outline_level: None,
            show_timeline: true,
            show_critical: false,
            show_baseline: false,
            show_slack: false,
            show_late: false,
            show_outline_numbers: false,
            show_project_summary: false,
            show_gridlines: true,
            show_nonworking: true,
            show_links: true,
            show_bar_text: true,
            show_progress: true,
            show_details: false,
            show_today: true,
            dark: false,
            font_size: 13.0,
            row_height: 22.0,
            table_width: 690.0,
            bar_colors: BarColors::default(),
            gantt_style: 0,
            scroll_to: None,
            search: String::new(),
            saved_views: vec![],
            graph_mode: String::new(),
            col_align: Default::default(),
            col_titles: Default::default(),
            col_widths: Default::default(),
            active_column: "name".into(),
            show_summary_tasks: true,
            task_path: vec![],
            baseline_shown: 0,
            slippage: None,
            link_style: String::new(),
            bar_size: String::new(),
            bar_date_format: String::new(),
            drawings: vec![],
            text_styles: Default::default(),
            print: Default::default(),
        }
    }
}

pub fn default_columns() -> Vec<String> {
    ["id", "indicators", "mode", "name", "duration", "start", "finish", "predecessors", "resourceNames"].iter().map(|s| s.to_string()).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    TaskInfo(u32),
    ResourceInfo(u32),
    ProjectInfo,
    ChangeWorkingTime(String),
    Options,
    CommandPalette,
    FindReplace,
    Report(String, Value),
    About,
    AgentAccess,
    Open,
    SaveAs,
    Export(String),
    AssignResources,
    Statistics,
    ConfirmDiscard(String),
    ColumnPicker,
    SaveView,
    MoreViews,
    LevelingOptions,
    Layout,
    BarStyles,
    FieldSettings(String),
    CustomFields,
    Help,
    GettingStarted,
    Feedback,
    /// File > Options, opened at a tab.
    AppOptions(String),
    TextStyles,
    Inspector,
    UpdateTasks,
    SetBaseline,
    Message(String, String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiRequest {
    Dialog(Dialog),
    Quit,
    Screenshot(Option<PathBuf>),
    ScrollToTask(u32),
    Zoom(f32),
    RibbonTab(String),
    Backstage(String),
    OpenFile(PathBuf),
}

#[derive(Clone)]
pub struct Session {
    pub project: Project,
    pub path: Option<PathBuf>,
    pub view: ViewState,
    pub undo: Vec<(String, Project)>,
    pub redo: Vec<(String, Project)>,
    pub dirty: bool,
    pub clipboard: Vec<(Task, Vec<Assignment>)>,
    /// The format picked up by Format Painter.
    pub style_clipboard: Option<crate::model::TextStyle>,
    pub log: Vec<String>,
    pub requests: Vec<UiRequest>,
    pub headless: bool,
    /// True while a script or agent (CLI, JSON channel, MCP) is running a command.
    pub agent: bool,
}

pub enum Row {
    Task(usize),
    Group(String),
    ProjectSummary,
}

impl Session {
    pub fn new(project: Project) -> Session {
        Session {
            project,
            path: None,
            view: ViewState::default(),
            undo: vec![],
            redo: vec![],
            dirty: false,
            clipboard: vec![],
            style_clipboard: None,
            log: vec![],
            requests: vec![],
            headless: true,
            agent: false,
        }
    }
    pub fn snapshot(&mut self, label: &str) {
        self.undo.push((label.to_string(), self.project.clone()));
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn request(&mut self, r: UiRequest) {
        self.requests.push(r);
    }
    pub fn dialog(&mut self, d: Dialog) {
        self.requests.push(UiRequest::Dialog(d));
    }
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.project.title.clone());
        format!("{}{} - {}", name, if self.dirty { " *" } else { "" }, crate::APP_NAME)
    }
    /// Index of the first selected task (or the last task).
    pub fn current_task(&self) -> Option<usize> {
        self.view.selected.iter().next().and_then(|u| self.project.task_index(*u))
    }

    /// Rows shown in task views, after collapse, outline level, filter, sort and group.
    pub fn task_rows(&self) -> Vec<Row> {
        let p = &self.project;
        let n = p.tasks.len();
        // visibility by collapse / outline level
        let mut visible = vec![true; n];
        for i in 0..n {
            if let Some(l) = self.view.outline_level {
                if p.tasks[i].outline_level > l {
                    visible[i] = false;
                }
            }
            if p.tasks[i].collapsed && p.tasks[i].summary {
                for j in p.descendants(i) {
                    visible[j] = false;
                }
            }
        }
        // filter (keep summary rows of matching tasks)
        let mut keep = vec![false; n];
        let filt = self.view.filter;
        let search = self.view.search.trim().to_lowercase();
        for i in 0..n {
            let ok = filt.matches(p, i) && (search.is_empty() || p.tasks[i].name.to_lowercase().contains(&search));
            if ok {
                keep[i] = true;
                if filt != Filter::Summary {
                    let mut c = i;
                    while let Some(par) = p.parent(c) {
                        keep[par] = true;
                        c = par;
                    }
                }
            }
        }
        let hide_summaries = !self.view.show_summary_tasks;
        let mut idx: Vec<usize> = (0..n).filter(|&i| visible[i] && keep[i] && !(hide_summaries && p.tasks[i].summary)).collect();
        if let Some((field, asc)) = &self.view.sort {
            if field != "id" || !asc {
                idx.sort_by(|&a, &b| {
                    let o = compare_field(p, a, b, field);
                    if *asc { o } else { o.reverse() }
                });
            }
        }
        let mut rows = vec![];
        if self.view.show_project_summary {
            rows.push(Row::ProjectSummary);
        }
        if self.view.group == Group::None {
            rows.extend(idx.into_iter().map(Row::Task));
        } else {
            let mut groups: Vec<(String, Vec<usize>)> = vec![];
            for i in idx {
                if p.tasks[i].summary {
                    continue;
                }
                let k = self.view.group.key(p, i);
                match groups.iter_mut().find(|(g, _)| *g == k) {
                    Some((_, v)) => v.push(i),
                    None => groups.push((k, vec![i])),
                }
            }
            groups.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, v) in groups {
                rows.push(Row::Group(k));
                rows.extend(v.into_iter().map(Row::Task));
            }
        }
        rows
    }
}

pub fn compare_field(p: &Project, a: usize, b: usize, field: &str) -> std::cmp::Ordering {
    let va = fields::task_value(p, a, field);
    let vb = fields::task_value(p, b, field);
    let key = |v: &Value| -> (f64, String) {
        match v {
            Value::Number(n) => (n.as_f64().unwrap_or(0.0), String::new()),
            Value::Bool(b) => (if *b { 1.0 } else { 0.0 }, String::new()),
            Value::Object(o) => (o.get("minutes").and_then(|m| m.as_f64()).unwrap_or(0.0), String::new()),
            Value::String(s) => (0.0, s.to_lowercase()),
            _ => (0.0, String::new()),
        }
    };
    let (na, sa) = key(&va);
    let (nb, sb) = key(&vb);
    na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal).then(sa.cmp(&sb)).then(a.cmp(&b))
}
