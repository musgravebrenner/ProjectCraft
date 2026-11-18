#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! PlanCraft window. `plancraft [FILE] [--screenshot out.png] [--port N] [--no-control]`.
//! Command-line work (run, batch, mcp...) lives in `plancraft-cli`, which uses the same commands.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = plancraft::gui::Options::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                opts.screenshot = args.get(i + 1).map(Into::into);
                i += 1;
            }
            "--port" => {
                opts.port = args.get(i + 1).and_then(|p| p.parse().ok());
                i += 1;
            }
            "--cmd" => {
                if let Some(id) = args.get(i + 1) {
                    let mut params = serde_json::json!({});
                    if args.get(i + 2).map(|a| a == "--params").unwrap_or(false) {
                        params = args.get(i + 3).and_then(|p| serde_json::from_str(p).ok()).unwrap_or(params);
                        i += 2;
                    }
                    opts.startup.push((id.clone(), params));
                }
                i += 1;
            }
            "--no-control" => opts.no_control = true,
            "--sample" => opts.sample = true,
            "--help" | "-h" => {
                println!("plancraft [FILE] [--sample] [--cmd ID [--params JSON]]... [--screenshot out.png] [--port N] [--no-control]\nFor scripting use plancraft-cli (plancraft-cli help).");
                return;
            }
            a if !a.starts_with("--") => opts.open = Some(a.into()),
            _ => {}
        }
        i += 1;
    }
    if let Err(e) = plancraft::gui::run(opts) {
        eprintln!("PlanCraft could not start: {e}");
        std::process::exit(1);
    }
}
