//! Reads project files in the binary .mpp format written by the desktop scheduling program
//! (its 2010 and later releases, which keep the plan in a `   114` folder of an OLE compound
//! file). The layout was worked out by comparing files with the program's own XML export of
//! the same plans; see MECHANICS.md. The records are turned into the XML interchange format
//! and handed to [`crate::io::import_xml`], whose field codes (constraints, link types,
//! duration units, resource types) are the same ones the .mpp file uses.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

type Cfb = cfb::CompoundFile<std::fs::File>;

fn read_stream(c: &mut Cfb, path: &str) -> Vec<u8> {
    let mut b = vec![];
    if let Ok(mut s) = c.open_stream(path) {
        let _ = s.read_to_end(&mut b);
    }
    b
}

fn u16at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}
fn u32at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}
fn f64at(b: &[u8], o: usize) -> Option<f64> {
    b.get(o..o + 8).map(|x| f64::from_le_bytes(x.try_into().unwrap())).filter(|v| v.is_finite())
}

/// Dates are two 16-bit values: tenths of a minute since midnight, then days since 1983-12-31.
fn timestamp(b: &[u8], o: usize) -> Option<chrono::NaiveDateTime> {
    let t = u16at(b, o)? as i64;
    let d = u16at(b, o + 2)? as i64;
    if d < 100 || d == 0xffff {
        return None;
    }
    let epoch = chrono::NaiveDate::from_ymd_opt(1983, 12, 31)?.and_hms_opt(0, 0, 0)?;
    Some(epoch + chrono::Duration::days(d) + chrono::Duration::seconds(if t == 0xffff { 0 } else { t * 6 }))
}

/// One table (tasks, resources, ...): fixed records, their flag bytes, and variable data.
struct Table {
    fixed: Vec<Vec<u8>>,
    meta: Vec<Vec<u8>>,
    meta2: Vec<Vec<u8>>,
    var: HashMap<(u32, u16), Vec<u8>>,
}

const MAGIC: u32 = 0xFADF_ADBA;

fn meta_items(meta: &[u8]) -> Vec<Vec<u8>> {
    if meta.len() < 16 || u32at(meta, 0) != Some(MAGIC) {
        return vec![];
    }
    let n = u32at(meta, 8).unwrap_or(0) as usize;
    if n == 0 || (meta.len() - 16) / n == 0 {
        return vec![];
    }
    let isz = (meta.len() - 16) / n;
    (0..n).map(|i| meta[16 + i * isz..16 + (i + 1) * isz].to_vec()).collect()
}

/// Fixed records: each meta item gives its record's offset; a record runs to the next offset.
fn fixed_blocks(items: &[Vec<u8>], data: &[u8]) -> Vec<Vec<u8>> {
    let offs: Vec<usize> = items.iter().map(|m| u32at(m, 4).unwrap_or(u32::MAX) as usize).collect();
    let mut sorted: Vec<usize> = offs.iter().copied().filter(|&o| o < data.len()).collect();
    sorted.sort_unstable();
    sorted.dedup();
    offs.iter()
        .zip(items)
        .map(|(&o, m)| {
            // deleted records keep their (stale) data; their flag byte has 0x02 set
            if o >= data.len() || m.first().map(|f| f & 0x02 != 0).unwrap_or(false) {
                return vec![];
            }
            let end = sorted.iter().copied().find(|&x| x > o).unwrap_or(data.len());
            data[o..end].to_vec()
        })
        .collect()
}

fn var_data(meta: &[u8], data: &[u8]) -> HashMap<(u32, u16), Vec<u8>> {
    let mut out = HashMap::new();
    if meta.len() < 24 || u32at(meta, 0) != Some(MAGIC) {
        return out;
    }
    let n = u32at(meta, 8).unwrap_or(0) as usize;
    for i in 0..n {
        let base = 24 + i * 12;
        let (Some(uid), Some(off), Some(ty)) = (u32at(meta, base), u32at(meta, base + 4), u16at(meta, base + 8)) else { break };
        let off = off as usize;
        let Some(size) = u32at(data, off) else { continue };
        if let Some(bytes) = data.get(off + 4..off + 4 + size as usize) {
            out.insert((uid, ty), bytes.to_vec());
        }
    }
    out
}

fn table(c: &mut Cfb, dir: &str) -> Table {
    let base = format!("/   114/{dir}");
    let meta = meta_items(&read_stream(c, &format!("{base}/FixedMeta")));
    let fixed = fixed_blocks(&meta, &read_stream(c, &format!("{base}/FixedData")));
    let meta2 = meta_items(&read_stream(c, &format!("{base}/Fixed2Meta")));
    let var = var_data(&read_stream(c, &format!("{base}/VarMeta")), &read_stream(c, &format!("{base}/Var2Data")));
    Table { fixed, meta, meta2, var }
}

impl Table {
    fn text(&self, uid: u32, ty: u16) -> String {
        match self.var.get(&(uid, ty)) {
            Some(b) => utf16(b),
            None => String::new(),
        }
    }
}

fn utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// Plain text from the RTF the program stores notes in.
pub fn rtf_to_text(rtf: &str) -> String {
    if !rtf.trim_start().starts_with("{\\rtf") {
        return rtf.to_string();
    }
    let mut out = String::new();
    let chars: Vec<char> = rtf.chars().collect();
    let mut i = 0;
    let mut depth = 0i32;
    let mut skip_depth: Option<i32> = None;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                depth += 1;
                // destinations that hold no visible text
                let rest: String = chars[i + 1..chars.len().min(i + 14)].iter().collect();
                if skip_depth.is_none() && (rest.starts_with("\\fonttbl") || rest.starts_with("\\colortbl") || rest.starts_with("\\stylesheet") || rest.starts_with("\\*") || rest.starts_with("\\info") || rest.starts_with("\\pict")) {
                    skip_depth = Some(depth);
                }
                i += 1;
            }
            '}' => {
                if skip_depth == Some(depth) {
                    skip_depth = None;
                }
                depth -= 1;
                i += 1;
            }
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                let n = chars[i];
                if n == '\\' || n == '{' || n == '}' {
                    if skip_depth.is_none() {
                        out.push(n);
                    }
                    i += 1;
                } else if n == '\'' {
                    let hex: String = chars[i + 1..chars.len().min(i + 3)].iter().collect();
                    if let (Ok(v), None) = (u8::from_str_radix(&hex, 16), skip_depth) {
                        out.push(v as char);
                    }
                    i += 3;
                } else if n.is_ascii_alphabetic() {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect();
                    let nstart = i;
                    if i < chars.len() && (chars[i] == '-' || chars[i].is_ascii_digit()) {
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let num: String = chars[nstart..i].iter().collect();
                    if i < chars.len() && chars[i] == ' ' {
                        i += 1;
                    }
                    if skip_depth.is_none() {
                        match word.as_str() {
                            "par" | "line" => out.push('\n'),
                            "tab" => out.push('\t'),
                            "u" => {
                                if let Ok(v) = num.parse::<i32>() {
                                    let v = if v < 0 { v + 65536 } else { v } as u32;
                                    if let Some(ch) = char::from_u32(v) {
                                        out.push(ch);
                                    }
                                    // skip the fallback character
                                    if i < chars.len() && chars[i] != '\\' && chars[i] != '{' && chars[i] != '}' {
                                        i += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    i += 1;
                }
            }
            '\r' | '\n' => i += 1,
            _ => {
                if skip_depth.is_none() {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    out.retain(|c| c == '\n' || c == '\t' || !c.is_control());
    out.trim().to_string()
}

/// String properties of an OLE property set stream (SummaryInformation and friends).
fn property_strings(b: &[u8]) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let Some(sets) = u32at(b, 24) else { return out };
    if sets == 0 {
        return out;
    }
    let Some(off) = u32at(b, 44).map(|o| o as usize) else { return out };
    let Some(count) = u32at(b, off + 4) else { return out };
    let codepage_utf16 = false;
    for k in 0..count.min(256) as usize {
        let (Some(id), Some(po)) = (u32at(b, off + 8 + k * 8), u32at(b, off + 12 + k * 8)) else { break };
        let p = off + po as usize;
        match u32at(b, p) {
            Some(30) => {
                // VT_LPSTR: length (with terminator) then bytes
                if let Some(len) = u32at(b, p + 4) {
                    if let Some(s) = b.get(p + 8..p + 8 + len as usize) {
                        let s: String = s.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
                        out.insert(id, s);
                    }
                }
            }
            Some(31) => {
                if let Some(len) = u32at(b, p + 4) {
                    if let Some(s) = b.get(p + 8..p + 8 + len as usize * 2) {
                        out.insert(id, utf16(s));
                    }
                }
            }
            _ => {}
        }
    }
    let _ = codepage_utf16;
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
fn xdate(d: chrono::NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%S").to_string()
}
fn xdur(minutes: i64) -> String {
    format!("PT{}H{}M0S", minutes / 60, minutes % 60)
}

/// What a quick look at an .mpp file finds, for messages and agents.
pub struct MppInfo {
    pub version: &'static str,
    pub tasks: usize,
    pub resources: usize,
    pub links: usize,
    pub assignments: usize,
}

/// Read an .mpp file into the XML interchange format.
pub fn to_xml(path: &Path) -> Result<(String, MppInfo), String> {
    let mut c = cfb::open(path).map_err(|e| format!("{} isn't a project file this app can read ({e})", path.display()))?;
    let has = |c: &Cfb, p: &str| c.is_storage(p);
    if !has(&c, "/   114") {
        let old = if has(&c, "/   112") {
            "the 2007 release"
        } else if has(&c, "/   19") || has(&c, "/   18") {
            "a release from 1998-2003"
        } else {
            ""
        };
        return Err(if old.is_empty() {
            format!("{} isn't an .mpp project file (no project data inside).", path.display())
        } else {
            format!("{} was saved by {old} of the desktop scheduling program, which uses an older .mpp layout this app can't read yet. Open it there and save it again (any 2010 or later release), or save it as XML.", path.display())
        });
    }
    let summary = property_strings(&read_stream(&mut c, "/\u{5}SummaryInformation"));
    let doc_summary = property_strings(&read_stream(&mut c, "/\u{5}DocumentSummaryInformation"));
    let tasks = table(&mut c, "TBkndTask");
    let res = table(&mut c, "TBkndRsc");
    let asg = table(&mut c, "TBkndAssn");
    let cons_meta = meta_items(&read_stream(&mut c, "/   114/TBkndCons/FixedMeta"));
    let cons = fixed_blocks(&cons_meta, &read_stream(&mut c, "/   114/TBkndCons/FixedData"));

    // --- tasks ---
    struct T {
        uid: u32,
        id: u32,
        xml: String,
    }
    let mut task_rows: Vec<T> = vec![];
    let mut project_start: Option<chrono::NaiveDateTime> = None;
    let mut summary_name = String::new();
    let mut task_uids: Vec<u32> = vec![];
    for (i, b) in tasks.fixed.iter().enumerate() {
        if b.len() < 186 {
            continue;
        }
        let uid = u32at(b, 4).unwrap_or(0);
        let id = u32at(b, 0).unwrap_or(0);
        let name = tasks.text(uid, 14);
        let start = timestamp(b, 104);
        if uid == 0 {
            summary_name = name;
            project_start = start;
            continue;
        }
        let level = u16at(b, 172).unwrap_or(1);
        if name.is_empty() && level == 0 {
            continue;
        }
        let m1 = tasks.meta.get(i).cloned().unwrap_or_default();
        let m2 = tasks.meta2.get(i).cloned().unwrap_or_default();
        let milestone = m1.get(10).map(|x| x & 0x02 != 0).unwrap_or(false);
        let manual = m2.get(8).map(|x| x & 0x80 != 0).unwrap_or(false);
        let active = m2.get(8).map(|x| x & 0x40 != 0).unwrap_or(true);
        let dur = u32at(b, 84).unwrap_or(0) as i64 / 10;
        let fmt = u16at(b, 164).unwrap_or(7);
        let mut x = String::from("  <Task>\n");
        let mut tag = |k: &str, v: String| x.push_str(&format!("   <{k}>{}</{k}>\n", esc(&v)));
        tag("UID", uid.to_string());
        tag("ID", id.to_string());
        tag("Name", name);
        tag("Active", if active { "1" } else { "0" }.into());
        tag("Manual", if manual { "1" } else { "0" }.into());
        tag("OutlineLevel", level.max(1).to_string());
        tag("Priority", u16at(b, 78).unwrap_or(500).min(1000).to_string());
        if let Some(s) = start {
            tag("Start", xdate(s));
        }
        if let Some(f) = timestamp(b, 108) {
            tag("Finish", xdate(f));
        }
        tag("Duration", xdur(dur));
        tag("DurationFormat", fmt.to_string());
        tag("Milestone", if milestone { "1" } else { "0" }.into());
        let pct = u16at(b, 92).unwrap_or(0).min(100);
        tag("PercentComplete", pct.to_string());
        if let Some(fc) = f64at(b, 48).filter(|v| *v != 0.0 && v.abs() < 1e12) {
            tag("FixedCost", format!("{fc:.2}"));
        }
        // actual dates only mean something once work has started
        if pct > 0 {
            if let Some(a) = timestamp(b, 96) {
                tag("ActualStart", xdate(a));
            }
            if pct == 100 {
                if let Some(f) = timestamp(b, 108) {
                    tag("ActualFinish", xdate(f));
                }
            }
        }
        let ct = u16at(b, 64).unwrap_or(0);
        tag("ConstraintType", if ct <= 7 { ct } else { 0 }.to_string());
        if let Some(cd) = timestamp(b, 66) {
            tag("ConstraintDate", xdate(cd));
        }
        if let Some(dl) = timestamp(b, 182) {
            tag("Deadline", xdate(dl));
        }
        let notes = tasks.var.get(&(uid, 15)).map(|n| rtf_to_text(&String::from_utf8_lossy(n))).unwrap_or_default();
        if !notes.is_empty() {
            tag("Notes", notes);
        }
        task_rows.push(T { uid, id, xml: x });
        task_uids.push(uid);
    }
    // --- links (predecessor, successor, type, lag in tenths of a minute, lag units) ---
    let mut links: HashMap<u32, Vec<String>> = HashMap::new();
    let mut nlinks = 0;
    for b in &cons {
        // one record per meta item: records past it are deleted links kept as free space
        for rec in b.chunks_exact(20).take(1) {
            let (Some(pred), Some(succ), Some(kind), Some(lag), Some(lag_fmt)) = (u32at(rec, 4), u32at(rec, 8), u16at(rec, 12), u32at(rec, 14), u16at(rec, 18)) else { continue };
            if pred == succ || !task_uids.contains(&pred) || !task_uids.contains(&succ) || kind > 3 {
                continue;
            }
            let lag = lag as i32;
            links.entry(succ).or_default().push(format!(
                "   <PredecessorLink>\n    <PredecessorUID>{pred}</PredecessorUID>\n    <Type>{kind}</Type>\n    <LinkLag>{lag}</LinkLag>\n    <LagFormat>{}</LagFormat>\n   </PredecessorLink>\n",
                if lag_fmt == 0 { 7 } else { lag_fmt }
            ));
            nlinks += 1;
        }
    }
    task_rows.sort_by_key(|t| t.id);

    // --- resources ---
    let mut res_xml = String::new();
    let mut res_kind: HashMap<u32, u8> = HashMap::new();
    for (i, b) in res.fixed.iter().enumerate() {
        if b.len() < 24 {
            continue;
        }
        let uid = u32at(b, 4).unwrap_or(0);
        let name = res.text(uid, 1);
        if uid == 0 || name.is_empty() {
            continue;
        }
        let m1 = res.meta.get(i).cloned().unwrap_or_default();
        let m2 = res.meta2.get(i).cloned().unwrap_or_default();
        let kind: u8 = if m2.get(8).map(|x| x & 0x10 != 0).unwrap_or(false) {
            2 // cost
        } else if m1.get(12).map(|x| x & 0x04 == 0).unwrap_or(false) {
            0 // material
        } else {
            1 // work
        };
        res_kind.insert(uid, kind);
        let mut x = String::from("  <Resource>\n");
        let mut tag = |k: &str, v: String| x.push_str(&format!("   <{k}>{}</{k}>\n", esc(&v)));
        tag("UID", uid.to_string());
        tag("ID", u32at(b, 0).unwrap_or(uid).to_string());
        tag("Name", name);
        tag("Type", kind.to_string());
        let initials = res.text(uid, 2);
        if !initials.is_empty() {
            tag("Initials", initials);
        }
        let group = res.text(uid, 3);
        if !group.is_empty() {
            tag("Group", group);
        }
        let label = res.text(uid, 299);
        if !label.is_empty() {
            tag("MaterialLabel", label);
        }
        if kind == 1 {
            tag("MaxUnits", format!("{}", f64at(b, 8).unwrap_or(10000.0) / 10000.0));
        }
        if kind != 2 {
            tag("StandardRate", format!("{}", f64at(b, 16).unwrap_or(0.0)));
        }
        x.push_str("  </Resource>\n");
        res_xml.push_str(&x);
    }
    // --- assignments ---
    let mut asg_xml = String::new();
    let mut nasg = 0;
    for b in &asg.fixed {
        if b.len() < 92 {
            continue;
        }
        let (Some(uid), Some(task), Some(resource)) = (u32at(b, 0), u32at(b, 4), u32at(b, 8)) else { continue };
        if !task_uids.contains(&task) || !res_kind.contains_key(&resource) {
            continue;
        }
        // cost resources have no units; the program shows 100%
        let units = if res_kind.get(&resource) == Some(&2) { 1.0 } else { f64at(b, 12).unwrap_or(10000.0) / 10000.0 };
        let work = (f64at(b, 20).unwrap_or(0.0) / 1000.0).round() as i64;
        let cost = f64at(b, 68).unwrap_or(0.0); // cents, as the XML format writes money
        asg_xml.push_str(&format!(
            "  <Assignment>\n   <UID>{uid}</UID>\n   <TaskUID>{task}</TaskUID>\n   <ResourceUID>{resource}</ResourceUID>\n   <Units>{units}</Units>\n   <Work>{}</Work>\n   <Cost>{cost:.2}</Cost>\n  </Assignment>\n",
            xdur(work)
        ));
        nasg += 1;
    }

    let title = summary.get(&2).cloned().filter(|t| !t.trim().is_empty()).or_else(|| Some(summary_name.clone()).filter(|t| !t.is_empty())).unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported Project".into()));
    let start = project_start.or_else(|| task_rows.iter().filter_map(|t| t.xml.split("<Start>").nth(1)).filter_map(|s| chrono::NaiveDateTime::parse_from_str(&s[..19], "%Y-%m-%dT%H:%M:%S").ok()).min());
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Project xmlns=\"http://schemas.microsoft.com/project\">\n");
    out.push_str(&format!(" <Title>{}</Title>\n", esc(&title)));
    if let Some(s) = summary.get(&3).filter(|s| !s.is_empty()) {
        out.push_str(&format!(" <Subject>{}</Subject>\n", esc(s)));
    }
    if let Some(s) = doc_summary.get(&0x0e).filter(|s| !s.is_empty()) {
        out.push_str(&format!(" <Manager>{}</Manager>\n", esc(s)));
    }
    if let Some(s) = doc_summary.get(&0x0f).filter(|s| !s.is_empty()) {
        out.push_str(&format!(" <Company>{}</Company>\n", esc(s)));
    }
    if let Some(s) = start {
        out.push_str(&format!(" <StartDate>{}</StartDate>\n", xdate(s)));
    }
    out.push_str(" <Tasks>\n");
    let ntasks = task_rows.len();
    for t in &task_rows {
        out.push_str(&t.xml);
        for l in links.get(&t.uid).into_iter().flatten() {
            out.push_str(l);
        }
        out.push_str("  </Task>\n");
    }
    out.push_str(" </Tasks>\n <Resources>\n");
    out.push_str(&res_xml);
    out.push_str(" </Resources>\n <Assignments>\n");
    out.push_str(&asg_xml);
    out.push_str(" </Assignments>\n</Project>\n");
    if ntasks == 0 && res_kind.is_empty() && !tasks.var.is_empty() && tasks.var.keys().any(|(u, t)| *u > 0 && *t == 14) {
        return Err(format!("{} has tasks this app couldn't read (it may be password-protected).", path.display()));
    }
    Ok((out, MppInfo { version: "2010 or later", tasks: ntasks, resources: res_kind.len(), links: nlinks, assignments: nasg }))
}

/// Whether a file is an OLE compound file (the container .mpp files use).
pub fn is_compound_file(path: &Path) -> bool {
    let mut head = [0u8; 8];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && head == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
}
