//! Developer tool: print the variable data (uid, type, size, preview) of one .mpp table.
//! cargo run --example mppvar -- FILE.mpp TBkndTask [UID]
use std::io::Read;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut c = cfb::open(&a[1]).unwrap();
    let rd = |c: &mut cfb::CompoundFile<std::fs::File>, p: &str| {
        let mut b = vec![];
        c.open_stream(p).unwrap().read_to_end(&mut b).unwrap();
        b
    };
    let meta = rd(&mut c, &format!("/   114/{}/VarMeta", a[2]));
    let data = rd(&mut c, &format!("/   114/{}/Var2Data", a[2]));
    let n = u32::from_le_bytes(meta[8..12].try_into().unwrap()) as usize;
    let want: Option<u32> = a.get(3).and_then(|s| s.parse().ok());
    for i in 0..n {
        let e = &meta[24 + i * 12..36 + i * 12];
        let uid = u32::from_le_bytes(e[0..4].try_into().unwrap());
        let off = u32::from_le_bytes(e[4..8].try_into().unwrap()) as usize;
        let ty = u16::from_le_bytes(e[8..10].try_into().unwrap());
        if want.map(|w| w != uid).unwrap_or(false) || off + 4 > data.len() {
            continue;
        }
        let sz = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        let body = &data[off + 4..(off + 4 + sz).min(data.len())];
        let u16s: String = body.chunks(2).take(40).map(|c| { let v = u16::from_le_bytes([c[0], *c.get(1).unwrap_or(&0)]); if (32..127).contains(&v) { v as u8 as char } else { '.' } }).collect();
        let hex: String = body.iter().take(16).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
        println!("uid {uid:4} type {ty:5} (0x{ty:04x}) size {sz:5}  {hex:48} {u16s}");
    }
}
