//! The ribbon layout: which commands each tab shows, in the original's groups and order.
//! Every button runs a command from the registry.

use super::RibbonTab;
use crate::session::ViewKind;

pub enum RibbonItem {
    /// A large button: icon above its label (label may contain a line break).
    Big(&'static str, &'static str),
    /// A large button with a dropdown: label, icon name, commands (one entry ending in '.' = a prefix).
    BigMenu(&'static str, &'static str, &'static [&'static str]),
    /// The large "Recent" reports dropdown.
    BigRecent,
    /// The large view button with the dropdown of every view.
    ViewPicker,
    /// A small button: icon beside its label.
    Btn(&'static str, &'static str, &'static str),
    /// A row of icon-only buttons.
    IconRow(&'static [&'static str]),
    Toggle(&'static str, &'static str),
    /// A small dropdown showing the current choice ("Filter: Critical").
    Menu(&'static str, &'static str),
    /// The Status Date box.
    StatusDate,
    /// A small button with an icon that opens a list of commands.
    SmallMenu(&'static str, &'static str, &'static [&'static str]),
    /// The Gantt chart style gallery: a row of coloured previews.
    StyleGallery,
    /// The Font group: typeface and size, bold, italic, underline, background and font colour.
    FontGroup,
    /// A large button shown greyed out, as the original shows features that need a server.
    BigDisabled(&'static str, &'static str, &'static str),
}

use RibbonItem::*;

const ALL_REPORTS: &[&str] = &[
    "report.projectOverview",
    "report.costOverview",
    "report.workOverview",
    "report.cashFlow",
    "report.resourceOverview",
    "report.overallocated",
    "report.criticalTasks",
    "report.lateTasks",
    "report.milestones",
    "report.upcoming",
    "report.slipping",
    "report.deadlines",
    "report.notStarted",
    "report.completed",
];

pub fn ribbon_groups(tab: RibbonTab, view: ViewKind) -> Vec<(&'static str, Vec<RibbonItem>)> {
    match tab {
        RibbonTab::Task => vec![
            ("View", vec![ViewPicker]),
            ("Clipboard", vec![Big("Paste", "edit.paste"), Btn("Cut", "edit.cut", "{}"), Btn("Copy", "edit.copy", "{}"), Btn("Format Painter", "format.painter.copy", "{}")]),
            ("Font", vec![FontGroup]),
            (
                "Schedule",
                vec![
                    IconRow(&["task.markComplete.0", "task.markComplete.25", "task.markComplete.50", "task.markComplete.75", "task.markComplete.100"]),
                    IconRow(&["task.outdent", "task.indent", "task.link", "task.unlink", "task.split"]),
                    SmallMenu("Mark on Track", "on_track", &["task.markOnTrack", "task.update"]),
                    Btn("Respect Links", "task.respectLinks", "{}"),
                    Btn("Inactivate", "task.inactivate", "{}"),
                ],
            ),
            (
                "Tasks",
                vec![
                    Big("Manually\nSchedule", "task.manualSchedule"),
                    Big("Auto\nSchedule", "task.autoSchedule"),
                    Big("Inspect", "task.inspect"),
                    BigMenu("Move", "move", &["task.move.1d", "task.move.1w", "task.move.2w", "task.move.4w", "task.move.custom", "task.moveBack", "resource.levelSelection"]),
                    BigMenu("Mode", "mode", &["task.manualSchedule", "task.autoSchedule", "task.newTasksManual", "task.newTasksAuto"]),
                ],
            ),
            (
                "Insert",
                vec![
                    BigMenu("Task", "task", &["task.insert", "task.insertRecurring", "task.insertSubproject"]),
                    Big("Summary", "task.insertSummary"),
                    Big("Milestone", "task.insertMilestone"),
                    BigDisabled("Deliverable", "deliverable", "Deliverables are shared through a project server, which PlanCraft doesn't connect to."),
                ],
            ),
            ("Properties", vec![Big("Information", "task.information"), Btn("Notes", "task.notes", "{}"), Btn("Details", "view.toggle.details", "{}"), Btn("Add to Timeline", "task.addToTimeline", "{}")]),
            (
                "Editing",
                vec![
                    Big("Scroll\nto Task", "task.scrollTo"),
                    SmallMenu("Find", "find", &["edit.find", "edit.replace", "edit.goTo"]),
                    SmallMenu("Clear", "clear", &["format.font.clear", "edit.clear.notes", "edit.clear.hyperlinks", "edit.clear.constraints", "edit.clear.progress", "edit.clear.customFields", "edit.delete"]),
                    SmallMenu("Fill", "fill", &["edit.fillDown"]),
                ],
            ),
        ],
        RibbonTab::Resource => vec![
            ("View", vec![BigMenu("Team\nPlanner", "team_planner", &["view.show.teamPlanner", "view.show.resourceSheet", "view.show.resourceUsage", "view.show.resourceGraph", "view.show.resourceForm"])]),
            ("Assignments", vec![Big("Assign\nResources", "resource.assignDialog"), BigMenu("Resource\nPool", "resource_pool", &["view.show.resourceSheet", "resource.replace", "resource.list", "file.export.resourcesCsv"])]),
            ("Insert", vec![BigMenu("Add\nResources", "add_resource", &["resource.new.work", "resource.new.material", "resource.new.cost", "resource.add"])]),
            ("Properties", vec![Big("Information", "resource.information"), Big("Notes", "resource.notes"), Big("Details", "view.toggle.details")]),
            (
                "Level",
                vec![
                    Big("Level\nSelection", "resource.levelSelection"),
                    Big("Level\nResource", "resource.levelResource"),
                    Big("Level\nAll", "resource.level"),
                    Btn("Leveling Options", "resource.levelingOptions", "{}"),
                    Btn("Clear Leveling", "resource.clearLeveling", "{}"),
                    Btn("Next Overallocation", "resource.nextOverallocation", "{}"),
                ],
            ),
        ],
        RibbonTab::Report => vec![
            ("Project", vec![Big("Compare\nProjects", "report.compareProjects")]),
            (
                "View Reports",
                vec![
                    BigMenu("New\nReport", "new_report", ALL_REPORTS),
                    BigMenu("Dashboards", "dashboards", &["report.projectOverview", "report.costOverview", "report.workOverview", "report.cashFlow"]),
                    BigMenu("Resources", "report_resources", &["report.resourceOverview", "report.overallocated", "report.workOverview"]),
                    BigMenu("Costs", "report_costs", &["report.cashFlow", "report.costOverview"]),
                    BigMenu("In Progress", "in_progress", &["report.criticalTasks", "report.lateTasks", "report.milestones", "report.slipping", "report.upcoming"]),
                    BigMenu("Getting\nStarted", "getting_started", &["help.gettingStarted", "help.show", "help.agentAccess"]),
                    BigMenu("Custom", "custom_report", &["report.deadlines", "report.notStarted", "report.completed", "report.compareProjects"]),
                    BigRecent,
                ],
            ),
            ("Export", vec![BigMenu("Visual\nReports", "visual_reports", &["file.export.html", "file.export.svg", "file.export.csv", "file.export.resourcesCsv", "file.export.markdown", "file.exportPicture"])]),
        ],
        RibbonTab::Project => vec![
            ("Insert", vec![Big("Subproject", "task.insertSubproject")]),
            ("Agents", vec![Btn("Scripts and Agents", "help.agentAccess", "{}"), Btn("Command Palette", "app.commandPalette", "{}"), Btn("Command Log", "app.log", "{}")]),
            (
                "Properties",
                vec![
                    Big("Project\nInformation", "project.information"),
                    Big("Custom\nFields", "project.customFields"),
                    BigMenu("WBS", "wbs", &["view.toggle.outlineNumbers", "view.column.show.wbs", "view.column.show.outlineNumber"]),
                    Big("Change\nWorking Time", "calendar.changeWorkingTime"),
                ],
            ),
            ("Schedule", vec![Big("Calculate\nProject", "project.recalculate"), BigMenu("Set\nBaseline", "baseline", &["project.baseline.dialog", "project.baseline.set.0", "project.baseline.clear.0"]), Big("Move\nProject", "project.move")]),
            ("Status", vec![StatusDate, Btn("Update Project", "project.updateAsOf", "{}"), Btn("Statistics", "project.statistics", "{}")]),
            ("Options", vec![Big("Options", "project.options"), Menu("Calendar", "project.calendar.")]),
        ],
        RibbonTab::View => vec![
            (
                "Task Views",
                vec![
                    ViewPicker,
                    Big("Task\nUsage", "view.show.taskUsage"),
                    Btn("Network Diagram", "view.show.networkDiagram", "{}"),
                    Btn("Calendar", "view.show.calendar", "{}"),
                    Menu("Other Views", "view.show."),
                ],
            ),
            (
                "Resource Views",
                vec![
                    Big("Team\nPlanner", "view.show.teamPlanner"),
                    Btn("Resource Usage", "view.show.resourceUsage", "{}"),
                    Btn("Resource Sheet", "view.show.resourceSheet", "{}"),
                    Btn("Resource Graph", "view.show.resourceGraph", "{}"),
                ],
            ),
            (
                "Data",
                vec![
                    BigMenu("Sort", "sort", &["view.sort."]),
                    BigMenu("Outline", "outline", &["view.outline."]),
                    BigMenu("Tables", "tables", &["view.table."]),
                    Menu("Highlight", "view.highlight."),
                    Menu("Filter", "view.filter."),
                    Menu("Group by", "view.group."),
                ],
            ),
            (
                "Zoom",
                vec![
                    Menu("Timescale", "view.timescale."),
                    BigMenu("Zoom", "zoom", &["view.zoom.in", "view.zoom.out", "view.goToToday", "view.goToProjectStart"]),
                    Big("Entire\nProject", "view.zoom.entireProject"),
                    Big("Selected\nTasks", "view.zoom.selectedTasks"),
                ],
            ),
            ("Split View", vec![Toggle("Timeline", "timeline"), Toggle("Details", "details")]),
            ("Macros", vec![BigMenu("Macros", "macros", &["app.runScript", "app.saveLogAsScript", "app.batch", "app.log", "app.commandPalette"])]),
        ],
        RibbonTab::Help => vec![
            ("Help", vec![Big("Help", "help.show"), Big("Feedback", "help.feedback"), Big("Show\nTraining", "help.gettingStarted")]),
            ("Agents", vec![Big("Scripts and\nAgents", "help.agentAccess"), Btn("Command Palette", "app.commandPalette", "{}"), Btn("About", "help.about", "{}")]),
        ],
        RibbonTab::Format => format_groups(view),
    }
}

/// The contextual format tab: what it offers depends on the view, as in the original.
fn format_groups(view: ViewKind) -> Vec<(&'static str, Vec<RibbonItem>)> {
    let format = ("Format", vec![Big("Text\nStyles", "format.textStyles"), Big("Gridlines", "view.toggle.gridlines"), BigMenu("Bar\nStyles", "bar_styles", &["format.ganttStyle."])]);
    match view {
        ViewKind::ResourceGraph => vec![
            format,
            ("Navigate", vec![Big("Previous\nResource", "resource.previous"), Big("Next\nResource", "resource.next")]),
            ("Data", vec![Menu("Graph", "view.graph.")]),
        ],
        ViewKind::ResourceForm => vec![("Navigate", vec![Big("Previous\nResource", "resource.previous"), Big("Next\nResource", "resource.next")]), ("Properties", vec![Big("Information", "resource.information"), Big("Notes", "resource.notes")])],
        ViewKind::TaskForm => vec![("Navigate", vec![Big("Previous\nTask", "task.previous"), Big("Next\nTask", "task.next")]), ("Properties", vec![Big("Information", "task.information"), Big("Notes", "task.notes")])],
        ViewKind::ResourceSheet | ViewKind::ResourceUsage | ViewKind::TeamPlanner => vec![
            format,
            ("Columns", vec![Menu("Show Column", "view.resourceColumn.show."), Menu("Hide Column", "view.resourceColumn.hide.")]),
            ("Level", vec![Big("Level\nAll", "resource.level"), Btn("Clear Leveling", "resource.clearLeveling", "{}"), Btn("Next Overallocation", "resource.nextOverallocation", "{}")]),
        ],
        _ => vec![
            ("Format", vec![Big("Text\nStyles", "format.textStyles"), BigMenu("Gridlines", "gridlines", &["view.toggle.gridlines", "view.toggle.nonworkingTime", "view.toggle.todayLine", "view.rowHeight."]), Big("Layout", "view.layout")]),
            (
                "Columns",
                vec![
                    Big("Insert\nColumn", "view.columns.pick"),
                    IconRow(&["view.column.align.left", "view.column.align.center", "view.column.align.right"]),
                    SmallMenu("Column Settings", "column_settings", &["view.column.title", "view.column.bestFit", "view.column.resetWidth", "view.column.hideActive", "view.column.reset", "view.table."]),
                    Btn("Custom Fields", "project.customFields", "{}"),
                ],
            ),
            (
                "Bar Styles",
                vec![
                    BigMenu("Format", "bar_format", &["format.barStyles", "format.barColor.task", "format.barColor.critical", "format.barColor.summary", "format.barColor.milestone", "format.barColor.progress", "format.resetBarColors"]),
                    Toggle("Critical Tasks", "criticalPath"),
                    Toggle("Slack", "slack"),
                    Toggle("Late Tasks", "lateTasks"),
                    BigMenu("Task\nPath", "task_path", &["view.taskPath.predecessors", "view.taskPath.drivingPredecessors", "view.taskPath.successors", "view.taskPath.drivenSuccessors", "view.taskPath.clear"]),
                    BigMenu("Baseline", "baseline", &["view.toggle.baseline", "view.baseline.show."]),
                    BigMenu("Slippage", "slippage", &["view.slippage."]),
                ],
            ),
            ("Gantt Chart Style", vec![StyleGallery]),
            ("Show/Hide", vec![Toggle("Outline Number", "outlineNumbers"), Toggle("Project Summary Task", "projectSummary"), Toggle("Summary Tasks", "summaryTasks"), Menu("More", "view.toggle.")]),
            ("Drawings", vec![BigMenu("Drawing", "drawing", &["format.drawing.text", "format.drawing.box", "format.drawing.line", "format.drawing.arrow", "format.drawing.clear"])]),
        ],
    }
}
