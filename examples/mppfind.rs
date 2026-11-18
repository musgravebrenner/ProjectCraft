//! Developer tool: find where fields live in .mpp fixed-data blocks by matching the values the
//! scheduling program wrote to its own XML (the .ref.xml next to each .mpp).
//! cargo run --example mppfind -- DIR(TBkndTask|TBkndRsc|TBkndAssn) TAG(Task|Resource|Assignment) FILE.mpp...
use std::collections::BTreeMap;
use std::io::Read;

fn stream(c: &mut cfb::CompoundFile<std::fs::File>, p: &str) -> Vec<u8> {
    let mut b = vec![];
    if let Ok(mut s) = c.open_stream(p) {
        s.read_to_end(&mut b).unwrap();
    }
    b
}
fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Blocks of a FixedData stream, by meta item index.
fn blocks(meta: &[u8], data: &[u8]) -> Vec<Vec<u8>> {
    if meta.len() < 16 {
        return vec![];
    }
    let n = u32at(meta, 8) as usize;
    if n == 0 {
        return vec![];
    }
    let isz = (meta.len() - 16) / n;
    let offs: Vec<usize> = (0..n).map(|i| u32at(meta, 16 + i * isz + 4) as usize).collect();
    let mut sorted = offs.clone();
    sorted.sort();
    sorted.dedup();
    offs.iter()
        .map(|&o| {
            let end = sorted.iter().find(|&&x| x > o).copied().unwrap_or(data.len()).min(data.len());
            if o >= data.len() {
                vec![]
            } else {
                data[o..end].to_vec()
            }
        })
        .collect()
}

fn xml_items(xml: &str, tag: &str) -> Vec<BTreeMap<String, String>> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = vec![];
    let mut rest = xml;
    while let Some(a) = rest.find(&open) {
        let after = &rest[a + open.len()..];
        let Some(b) = after.find(&close) else { break };
        let body = &after[..b];
        let mut m = BTreeMap::new();
        let mut r = body;
        while let Some(s) = r.find('<') {
            let t = &r[s + 1..];
            let Some(e) = t.find('>') else { break };
            let name = &t[..e];
            if name.starts_with('/') || name.ends_with('/') {
                r = &t[e + 1..];
                continue;
            }
            let after_tag = &t[e + 1..];
            let close_t = format!("</{name}>");
            if let Some(ce) = after_tag.find(&close_t) {
                let v = &after_tag[..ce];
                if !v.contains('<') {
                    m.entry(name.to_string()).or_insert(v.to_string());
                }
                r = &after_tag[ce + close_t.len()..];
                // nested containers: keep scanning inside only if value had tags
                if v.contains('<') {
                    r = after_tag;
                }
            } else {
                r = after_tag;
            }
        }
        out.push(m);
        rest = &after[b + close.len()..];
    }
    out
}

fn epoch() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(1983, 12, 31).unwrap().and_hms_opt(0, 0, 0).unwrap()
}
fn ts(b: &[u8], o: usize) -> Option<chrono::NaiveDateTime> {
    let t = u16at(b, o) as i64;
    let d = u16at(b, o + 2) as i64;
    if d < 100 || d == 65535 {
        return None;
    }
    Some(epoch() + chrono::Duration::days(d) + chrono::Duration::seconds(if t == 65535 { 0 } else { t * 6 }))
}
fn pt_minutes(s: &str) -> Option<i64> {
    // PT8H30M0S
    let s = s.strip_prefix("PT")?;
    let h: f64 = s.split('H').next()?.parse().ok()?;
    let rest = s.split('H').nth(1)?;
    let m: f64 = rest.split('M').next()?.parse().ok()?;
    Some((h * 60.0 + m).round() as i64)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let tag = &args[2];
    // offset -> (field -> hits, misses)
    let mut score: BTreeMap<(String, String), (u32, u32, String)> = BTreeMap::new();
    let mut bits: BTreeMap<(String, usize, u8), (u32, u32, u32)> = BTreeMap::new();
    for file in &args[3..] {
        let mut c = cfb::open(file).unwrap();
        let base = format!("/   114/{dir}");
        let b1 = blocks(&stream(&mut c, &format!("{base}/FixedMeta")), &stream(&mut c, &format!("{base}/FixedData")));
        let b2 = blocks(&stream(&mut c, &format!("{base}/Fixed2Meta")), &stream(&mut c, &format!("{base}/Fixed2Data")));
        let m1 = meta_items(&stream(&mut c, &format!("{base}/FixedMeta")));
        let m2 = meta_items(&stream(&mut c, &format!("{base}/Fixed2Meta")));
        let xml = std::fs::read_to_string(file.replace(".mpp", ".ref.xml")).unwrap();
        let items = xml_items(&xml, tag);
        println!("{file}: {} fixed blocks, {} fixed2 blocks, {} xml items; sizes {:?}", b1.len(), b2.len(), items.len(), b1.iter().map(|b| b.len()).collect::<Vec<_>>());
        for (i, blk) in b1.iter().enumerate() {
            if blk.len() < 8 {
                continue;
            }
            let uid = u32at(blk, 0);
            let Some(it) = items.iter().find(|m| m.get("UID").map(|u| u == &uid.to_string()).unwrap_or(false)) else { continue };
            for (which, b) in [("F1", blk.clone()), ("F2", b2.get(i).cloned().unwrap_or_default()), ("M1", m1.get(i).cloned().unwrap_or_default()), ("M2", m2.get(i).cloned().unwrap_or_default())] {
                for (field, val) in it {
                    let fval = val.as_str();
                    for o in 0..b.len() {
                        let mut tests: Vec<(&str, bool)> = vec![];
                        if o + 4 <= b.len() {
                            if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(fval, "%Y-%m-%dT%H:%M:%S") {
                                tests.push(("ts", ts(&b, o) == Some(dt)));
                            }
                            if let Some(m) = pt_minutes(fval) {
                                if m > 0 {
                                    tests.push(("dur10", u32at(&b, o) as i64 == m * 10));
                                    tests.push(("dur1000", u32at(&b, o) as i64 == m * 1000));
                                }
                            }
                        }
                        if o + 2 <= b.len() {
                            if let Ok(n) = fval.parse::<i64>() {
                                if n > 1 && n < 65535 {
                                    tests.push(("u16", u16at(&b, o) as i64 == n));
                                }
                            }
                        }
                        if o + 8 <= b.len() {
                            if let Ok(f) = fval.parse::<f64>() {
                                if f.abs() > 1.0 {
                                    let d = f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
                                    tests.push(("f64", (d - f).abs() < 1e-6));
                                    tests.push(("f64x100", (d - f * 100.0).abs() < 1e-3));
                                }
                            }
                        }
                        for (enc, ok) in tests {
                            let key = (field.clone(), format!("{which}+{o}:{enc}"));
                            let e = score.entry(key).or_insert((0, 0, String::new()));
                            if ok {
                                e.0 += 1;
                            } else {
                                e.1 += 1;
                            }
                        }
                    }
                    if fval == "0" || fval == "1" {
                        let v = fval == "1";
                        for o in 0..b.len() {
                            for bit in 0..8u8 {
                                let set = b[o] & (1 << bit) != 0;
                                let e = bits.entry((format!("{field}@{which}"), o, bit)).or_insert((0, 0, 0)); if v { e.2 += 1; }
                                if set == v {
                                    e.0 += 1;
                                } else {
                                    e.1 += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let want: Vec<&str> = std::env::var("FIELDS").map(|f| f.leak().split(',').collect()).unwrap_or_default();
    for ((field, loc), (hit, miss, _)) in &score {
        if *hit >= 2 && *miss == 0 && (want.is_empty() || want.contains(&field.as_str())) {
            println!("{field:24} {loc:20} hits {hit}");
        }
    }
    let bitf: Vec<&str> = std::env::var("BITS").map(|f| f.leak().split(',').collect()).unwrap_or_default();
    for ((field, o, bit), (hit, miss, nt)) in &bits {
        let name = field.split('@').next().unwrap();
        if *miss == 0 && *hit >= 3 && *nt > 0 && *nt < *hit && bitf.contains(&name) {
            println!("BIT {field:24} +{o} bit {bit} hits {hit}");
        }
    }
}

/// Meta items (the per-record flag bytes) of a FixedMeta stream.
fn meta_items(meta: &[u8]) -> Vec<Vec<u8>> {
    if meta.len() < 16 {
        return vec![];
    }
    let n = u32at(meta, 8) as usize;
    if n == 0 {
        return vec![];
    }
    let isz = (meta.len() - 16) / n;
    (0..n).map(|i| meta[16 + i * isz..16 + (i + 1) * isz].to_vec()).collect()
}
