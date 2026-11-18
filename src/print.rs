//! Printing: lays the Gantt chart out on paper pages (for the Print preview and for PDF), and a
//! small PDF writer so printing needs no other software. Sizes are in points (1/72 inch).

use crate::model::*;
use crate::session::{Row, Session};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PrintSettings {
    pub landscape: bool,
    /// letter, legal, a4, a3, tabloid
    pub paper: String,
    /// Only print this date range (chart), when set.
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    /// Print the task table beside the chart.
    pub table: bool,
    /// Print only the rows the view shows (filters, collapsed summaries) instead of every task.
    pub visible_only: bool,
    pub header: String,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings { landscape: true, paper: "letter".into(), from: None, to: None, table: true, visible_only: true, header: String::new() }
    }
}

pub fn paper_size(name: &str) -> (f32, f32) {
    match name {
        "a4" => (595.0, 842.0),
        "a3" => (842.0, 1191.0),
        "legal" => (612.0, 1008.0),
        "tabloid" => (792.0, 1224.0),
        _ => (612.0, 792.0),
    }
}

pub const PAPERS: &[(&str, &str)] = &[("letter", "Letter (8.5\" x 11\")"), ("legal", "Legal (8.5\" x 14\")"), ("tabloid", "Tabloid (11\" x 17\")"), ("a4", "A4 (210 x 297 mm)"), ("a3", "A3 (297 x 420 mm)")];

#[derive(Clone, Debug)]
pub enum Prim {
    Rect { x: f32, y: f32, w: f32, h: f32, fill: Option<[u8; 3]>, stroke: Option<[u8; 3]> },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 3], width: f32 },
    Text { x: f32, y: f32, size: f32, text: String, color: [u8; 3], bold: bool, right: bool },
    Diamond { cx: f32, cy: f32, r: f32, color: [u8; 3] },
}

#[derive(Clone, Debug)]
pub struct Page {
    pub w: f32,
    pub h: f32,
    pub prims: Vec<Prim>,
}

/// Approximate Helvetica text width (the PDF and the preview both use it to fit text).
pub fn text_width(s: &str, size: f32) -> f32 {
    s.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 0.25,
            'f' | 't' | 'r' | 'I' | ' ' | '(' | ')' | '[' | ']' | '-' | '/' => 0.32,
            'm' | 'w' | 'M' | 'W' => 0.85,
            c if c.is_ascii_uppercase() => 0.66,
            c if c.is_ascii_digit() => 0.556,
            _ => 0.52,
        })
        .sum::<f32>()
        * size
}

fn fit(s: &str, size: f32, width: f32) -> String {
    if text_width(s, size) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if text_width(&format!("{out}{c}..."), size) > width {
            break;
        }
        out.push(c);
    }
    format!("{out}...")
}

/// Lay the current task view out on pages: header, task table, timescale, bars, footer.
pub fn layout(s: &Session, ps: &PrintSettings) -> Vec<Page> {
    let p = &s.project;
    let (mut w, mut h) = paper_size(&ps.paper);
    if ps.landscape {
        std::mem::swap(&mut w, &mut h);
    }
    let margin = 36.0;
    let header_h = 34.0;
    let footer_h = 22.0;
    let ts_h = 24.0;
    let row_h = 13.0;
    let fs = 7.5;
    let content_w = w - 2.0 * margin;
    let rows: Vec<usize> = if ps.visible_only {
        s.task_rows().into_iter().filter_map(|r| if let Row::Task(i) = r { Some(i) } else { None }).collect()
    } else {
        (0..p.tasks.len()).collect()
    };
    let rows_per_page = (((h - 2.0 * margin - header_h - footer_h - ts_h) / row_h).floor() as usize).max(1);
    let pages_n = rows.len().div_ceil(rows_per_page).max(1);
    // columns of the table
    let cols: Vec<(&str, &str, f32)> = if ps.table {
        vec![("id", "ID", 22.0), ("name", "Task Name", 0.0), ("duration", "Duration", 46.0), ("start", "Start", 52.0), ("finish", "Finish", 52.0)]
    } else {
        vec![]
    };
    let table_w = if ps.table { (content_w * 0.42).clamp(200.0, 420.0) } else { 0.0 };
    let fixed: f32 = cols.iter().map(|c| c.2).sum();
    let name_w = (table_w - fixed).max(60.0);
    let chart_x0 = margin + table_w;
    let chart_w = content_w - table_w;
    // date range
    let cal = p.cal();
    let start = ps.from.unwrap_or_else(|| cal.snap_forward(p.start).date() - Duration::days(1));
    let end = ps.to.unwrap_or_else(|| p.finish.date() + Duration::days(2));
    let end = if end <= start { start + Duration::days(7) } else { end };
    let days = (end - start).num_days().max(1) as f32 + 1.0;
    let ppd = chart_w / days;
    let x_of = |t: NaiveDateTime| chart_x0 + (t - start.and_hms_opt(0, 0, 0).unwrap()).num_minutes() as f32 / 1440.0 * ppd;
    let bc = &s.view.bar_colors;
    let grey = [200, 200, 200];
    let dark = [40, 40, 40];
    let printed = chrono::Local::now().format("%a %-m/%-d/%y").to_string();
    let mut pages = vec![];
    for pg in 0..pages_n {
        let mut v = vec![];
        let top = margin;
        // header
        let title = if ps.header.is_empty() { p.title.clone() } else { ps.header.clone() };
        v.push(Prim::Text { x: margin, y: top + 14.0, size: 14.0, text: title, color: dark, bold: true, right: false });
        v.push(Prim::Text {
            x: w - margin,
            y: top + 14.0,
            size: 8.0,
            text: format!("{} - {}   |   printed {}", fmt_date(cal.snap_forward(p.start)), fmt_date(p.finish), printed),
            color: [90, 90, 90],
            bold: false,
            right: true,
        });
        let y0 = top + header_h;
        // table header
        let mut x = margin;
        v.push(Prim::Rect { x: margin, y: y0, w: content_w, h: ts_h, fill: Some([235, 235, 235]), stroke: None });
        for (id, title, cw) in &cols {
            let cw = if *id == "name" { name_w } else { *cw };
            v.push(Prim::Text { x: x + 3.0, y: y0 + ts_h - 6.0, size: fs, text: title.to_string(), color: dark, bold: true, right: false });
            v.push(Prim::Line { x1: x + cw, y1: y0, x2: x + cw, y2: y0 + ts_h, color: grey, width: 0.5 });
            x += cw;
        }
        // timescale: weeks on top, day letters (or day numbers) below
        let mut d = start;
        while d <= end {
            let xa = x_of(d.and_hms_opt(0, 0, 0).unwrap());
            if d.weekday().num_days_from_sunday() == 0 || d == start {
                v.push(Prim::Line { x1: xa, y1: y0, x2: xa, y2: y0 + ts_h, color: grey, width: 0.5 });
                if (end - d).num_days() >= 3 {
                    v.push(Prim::Text { x: xa + 2.0, y: y0 + 9.0, size: 6.5, text: d.format("%b %-d, '%y").to_string(), color: dark, bold: false, right: false });
                }
            }
            if ppd >= 7.0 {
                v.push(Prim::Text { x: xa + ppd / 2.0 - 2.0, y: y0 + ts_h - 5.0, size: 6.0, text: d.format("%a").to_string()[..1].to_string(), color: [90, 90, 90], bold: false, right: false });
            }
            d += Duration::days(1);
        }
        let body_y = y0 + ts_h;
        let page_rows = &rows[(pg * rows_per_page).min(rows.len())..((pg + 1) * rows_per_page).min(rows.len())];
        let body_h = page_rows.len() as f32 * row_h;
        // nonworking shading
        let mut d = start;
        while d <= end {
            if !cal.is_working_day(d) {
                let xa = x_of(d.and_hms_opt(0, 0, 0).unwrap());
                v.push(Prim::Rect { x: xa, y: body_y, w: ppd, h: body_h, fill: Some([244, 244, 244]), stroke: None });
            }
            d += Duration::days(1);
        }
        for (k, &i) in page_rows.iter().enumerate() {
            let t = &p.tasks[i];
            let y = body_y + k as f32 * row_h;
            let ty = y + row_h - 3.5;
            let mut x = margin;
            for (id, _, cw) in &cols {
                let cw = if *id == "name" { name_w } else { *cw };
                let (text, indent) = match *id {
                    "id" => ((i + 1).to_string(), 0.0),
                    "name" => (t.name.clone(), (t.outline_level as f32 - 1.0) * 8.0),
                    f => (crate::fields::task_text(p, i, f), 0.0),
                };
                let text = fit(&text, fs, cw - 6.0 - indent);
                v.push(Prim::Text { x: x + 3.0 + indent, y: ty, size: fs, text, color: dark, bold: t.summary, right: false });
                x += cw;
            }
            v.push(Prim::Line { x1: margin, y1: y + row_h, x2: margin + content_w, y2: y + row_h, color: [225, 225, 225], width: 0.4 });
            // bar
            let x0 = x_of(t.start).max(chart_x0);
            let x1 = x_of(t.finish).min(chart_x0 + chart_w);
            let crit = s.view.show_critical && t.critical;
            if t.summary {
                if x1 > x0 {
                    v.push(Prim::Rect { x: x0, y: y + 3.0, w: x1 - x0, h: 3.0, fill: Some(bc.summary), stroke: None });
                    v.push(Prim::Line { x1: x0, y1: y + 3.0, x2: x0, y2: y + 9.0, color: bc.summary, width: 1.5 });
                    v.push(Prim::Line { x1, y1: y + 3.0, x2: x1, y2: y + 9.0, color: bc.summary, width: 1.5 });
                }
            } else if t.is_milestone() {
                let cx = x_of(t.start);
                if cx >= chart_x0 && cx <= chart_x0 + chart_w {
                    v.push(Prim::Diamond { cx, cy: y + row_h / 2.0, r: 4.0, color: if crit { bc.critical } else { bc.milestone } });
                    v.push(Prim::Text { x: cx + 6.0, y: ty, size: 6.5, text: fmt_date_short(t.start), color: dark, bold: false, right: false });
                }
            } else if x1 > x0 {
                let fill = t.bar_color.unwrap_or(if crit { bc.critical } else if t.manual { bc.manual } else { bc.task });
                v.push(Prim::Rect { x: x0, y: y + 3.0, w: x1 - x0, h: row_h - 6.0, fill: Some(fill), stroke: Some([70, 100, 150]) });
                if t.percent_complete > 0.0 {
                    let px = x0 + (x1 - x0) * (t.percent_complete as f32 / 100.0);
                    v.push(Prim::Rect { x: x0, y: y + row_h / 2.0 - 1.2, w: px - x0, h: 2.4, fill: Some(bc.progress), stroke: None });
                }
                let names = p.resource_names(t.uid);
                if !names.is_empty() && x1 + 4.0 < chart_x0 + chart_w - 20.0 {
                    let room = chart_x0 + chart_w - x1 - 6.0;
                    v.push(Prim::Text { x: x1 + 3.0, y: ty, size: 6.5, text: fit(&names, 6.5, room), color: [60, 60, 60], bold: false, right: false });
                }
            }
        }
        // frame
        v.push(Prim::Rect { x: margin, y: y0, w: content_w, h: ts_h + body_h, fill: None, stroke: Some([160, 160, 160]) });
        if ps.table {
            v.push(Prim::Line { x1: chart_x0, y1: y0, x2: chart_x0, y2: y0 + ts_h + body_h, color: [160, 160, 160], width: 0.8 });
        }
        // footer
        let fy = h - margin + 4.0;
        v.push(Prim::Text { x: margin, y: fy, size: 7.0, text: format!("{}  -  {}", p.title, crate::APP_NAME), color: [120, 120, 120], bold: false, right: false });
        v.push(Prim::Text { x: w - margin, y: fy, size: 7.0, text: format!("Page {} of {}", pg + 1, pages_n), color: [120, 120, 120], bold: false, right: true });
        pages.push(Page { w, h, prims: v });
    }
    pages
}

fn pdf_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c)
            }
            c if (c as u32) < 128 => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

fn col(c: [u8; 3]) -> String {
    format!("{:.3} {:.3} {:.3}", c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0)
}

/// Write pages as a PDF (Helvetica text, vector shapes).
pub fn to_pdf(pages: &[Page]) -> Vec<u8> {
    let mut objs: Vec<String> = vec![];
    // 1 catalog, 2 pages, 3 font, 4 bold font, then (page, content) pairs
    let n = pages.len();
    let kids: Vec<String> = (0..n).map(|k| format!("{} 0 R", 5 + k * 2)).collect();
    objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), n));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into());
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".into());
    for (k, pg) in pages.iter().enumerate() {
        let hgt = pg.h;
        let mut c = String::new();
        for pr in &pg.prims {
            match pr {
                Prim::Rect { x, y, w, h, fill, stroke } => {
                    if let Some(f) = fill {
                        c.push_str(&format!("{} rg {:.2} {:.2} {:.2} {:.2} re f\n", col(*f), x, hgt - y - h, w, h));
                    }
                    if let Some(s) = stroke {
                        c.push_str(&format!("0.6 w {} RG {:.2} {:.2} {:.2} {:.2} re S\n", col(*s), x, hgt - y - h, w, h));
                    }
                }
                Prim::Line { x1, y1, x2, y2, color, width } => {
                    c.push_str(&format!("{:.2} w {} RG {:.2} {:.2} m {:.2} {:.2} l S\n", width, col(*color), x1, hgt - y1, x2, hgt - y2));
                }
                Prim::Diamond { cx, cy, r, color } => {
                    let y = hgt - cy;
                    c.push_str(&format!("{} rg {:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l f\n", col(*color), cx, y + r, cx + r, y, cx, y - r, cx - r, y));
                }
                Prim::Text { x, y, size, text, color, bold, right } => {
                    let x = if *right { x - text_width(text, *size) } else { *x };
                    c.push_str(&format!("BT /{} {:.1} Tf {} rg {:.2} {:.2} Td ({}) Tj ET\n", if *bold { "F2" } else { "F1" }, size, col(*color), x, hgt - y, pdf_text(text)));
                }
            }
        }
        objs.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.0} {:.0}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            pg.w,
            pg.h,
            6 + k * 2
        ));
        objs.push(format!("<< /Length {} >>\nstream\n{}endstream", c.len(), c));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![];
    for (k, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", k + 1, o).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref).as_bytes());
    out
}

/// Open a file with the system's default program (the PDF viewer prints it).
pub fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    let r = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    r.map(|_| ()).map_err(|e| format!("couldn't open {}: {e}", path.display()))
}
