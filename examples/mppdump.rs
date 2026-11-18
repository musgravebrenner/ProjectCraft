//! Developer tool: list the streams of a compound file and hex-dump one.
//! cargo run --example mppdump -- FILE [STREAM [MAXBYTES]]
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut comp = cfb::open(&args[1]).expect("open");
    if args.len() < 3 {
        let entries: Vec<(String, u64, bool)> = comp.walk().map(|e| (e.path().display().to_string(), e.len(), e.is_stream())).collect();
        for (p, len, s) in entries {
            println!("{}{p}  {len}", if s { "  " } else { "D " });
        }
        return;
    }
    let mut buf = vec![];
    comp.open_stream(&args[2]).expect("stream").read_to_end(&mut buf).unwrap();
    let max: usize = args.get(3).and_then(|m| m.parse().ok()).unwrap_or(4096);
    println!("{} bytes", buf.len());
    for (k, chunk) in buf.chunks(16).enumerate().take(max / 16) {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let txt: String = chunk.iter().map(|&b| if (32..127).contains(&b) { b as char } else { '.' }).collect();
        println!("{:06x}  {:<48} {}", k * 16, hex.join(" "), txt);
    }
}
