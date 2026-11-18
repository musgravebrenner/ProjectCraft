//! `plancraft-cli`: the same command registry without a window.

use crate::commands::Registry;
use crate::control;
use crate::io;
use crate::mcp::McpServer;
use crate::model::Project;
use crate::session::Session;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAIN_HELP: &str = "plancraft-cli - plan and schedule projects from scripts and agents

Every menu item, ribbon button and dialog in PlanCraft runs a command from one registry.
This tool runs the same commands without a window.

USAGE:
  plancraft-cli <subcommand> [options]

SUBCOMMANDS:
  run        Open a plan (or start one), run commands, write the result
  batch      Apply one action list to every plan in a folder
  new        Write a new empty plan (or the sample plan)
  commands   List every command (filter with --filter)
  describe   Show one command's help and parameters
  info       Print a plan's summary
  report     Print a built-in report as Markdown or JSON
  control    Speak the JSON control protocol on stdin/stdout
  send       Send one command to the running PlanCraft window
  mcp        Run an MCP server on stdin/stdout (headless or --bridge to the window)
  help       This text, or help for a subcommand

Every subcommand explains itself:  plancraft-cli <subcommand> --help

EXAMPLES:
  plancraft-cli run office.plan.json \\
    --cmd task.insert        --params '{\"name\":\"Paint walls\",\"duration\":\"3d\",\"predecessors\":\"5\"}' \\
    --cmd resource.assign    --params '{\"ids\":[6],\"resource\":\"Painters\",\"units\":200}' \\
    --cmd project.baseline.set.0 \\
    --out office-final.plan.json
  plancraft-cli batch --actions weekly.json --in ./plans --out ./updated
  plancraft-cli mcp
";

const RUN_HELP: &str = "plancraft-cli run [FILE] [--sample] (--cmd ID [--params JSON])... [--out FILE] [--save] [--json]

Opens FILE (.plan.json or .xml), or a new empty plan, or --sample. Runs each --cmd in order; a
--params right after a --cmd gives its parameters as a JSON object (or @file.json).
Results are printed; the first failing command stops the run with exit code 1.

  --out FILE    write the result; the extension picks the format:
                .plan.json/.json (project), .xml (XML interchange), .csv (task table),
                .svg (Gantt chart picture), .html (report with chart), .md (summary)
  --save        write back to FILE
  --json        print results as JSON lines
  --quiet       print nothing but errors
";

const BATCH_HELP: &str = "plancraft-cli batch --actions ACTIONS.json --in DIR --out DIR [--format EXT]

Applies one action list to every .plan.json/.json/.xml plan in DIR and writes each result to the
output folder (same name, or with --format: plan.json, xml, csv, svg, html, md).

ACTIONS.json is a list of steps, or {\"steps\": [...]}:
  [ {\"cmd\": \"project.updateAsOf\", \"params\": {\"date\": \"2026-11-20\"}},
    {\"cmd\": \"project.baseline.set.1\"} ]
";

const MCP_HELP: &str = "plancraft-cli mcp [FILE] [--sample] [--bridge] [--port N]

Runs a Model Context Protocol server on stdin/stdout. Tools: run_command, run_commands,
list_commands, describe_command, get_project_summary, list_tasks, list_resources, open_project,
save_project.

Headless (default): the server holds its own plan (FILE, --sample, or empty).
--bridge: every command goes to the running PlanCraft window (control channel, port 47811 or --port).

Example client entry:  {\"command\": \"plancraft-cli\", \"args\": [\"mcp\", \"--bridge\"]}
";

const CONTROL_HELP: &str = "plancraft-cli control [FILE] [--sample]

Reads one JSON request per line on stdin and answers one JSON line on stdout:
  {\"id\":1, \"cmd\":\"task.insert\", \"params\":{\"name\":\"Survey\",\"duration\":\"2d\"}}
  -> {\"id\":1, \"ok\":true, \"result\":{\"id\":1,\"uid\":1}}
Special: {\"cmd\":\"list\"}, {\"cmd\":\"describe\",\"params\":{\"command\":\"task.insert\"}}, {\"cmd\":\"ping\"}.
The running window speaks the same protocol on tcp 127.0.0.1:47811.
";

const SEND_HELP: &str = "plancraft-cli send --cmd ID [--params JSON] [--port N]

Sends one command to the running PlanCraft window and prints its result.
";

const NEW_HELP: &str = "plancraft-cli new --out FILE [--title TITLE] [--start DATE] [--sample]

Writes a new empty plan, or the sample plan with --sample.
";

const OTHER_HELP: &str = "plancraft-cli commands [--filter TEXT] [--json]
plancraft-cli describe COMMAND_ID
plancraft-cli info FILE [--json]
plancraft-cli report FILE --report ID [--json]   (ids: projectOverview, criticalTasks, lateTasks,
    milestones, upcoming, costOverview, cashFlow, resourceOverview, workOverview, overallocated,
    slipping, deadlines, notStarted, completed)
";

fn arg_val(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}
fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}
fn first_positional(args: &[String], value_flags: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with("--") {
            if value_flags.contains(&a.as_str()) {
                i += 1;
            }
        } else {
            return Some(a.clone());
        }
        i += 1;
    }
    None
}

fn parse_json_arg(text: &str) -> Result<Value, String> {
    let t = if let Some(path) = text.strip_prefix('@') { std::fs::read_to_string(path).map_err(|e| format!("can't read {path}: {e}"))? } else { text.to_string() };
    serde_json::from_str(&t).map_err(|e| format!("--params is not valid JSON ({e}): {t}"))
}

fn open_session(file: Option<&str>, sample: bool) -> Result<Session, String> {
    let mut s = if sample {
        crate::sample::sample_session()
    } else if let Some(f) = file {
        let (p, view) = io::load(Path::new(f))?;
        let mut s = Session::new(p);
        if let Some(v) = view {
            s.view = v;
        }
        s.path = Some(PathBuf::from(f));
        s
    } else {
        Session::new(Project::new("Project1", chrono::Local::now().date_naive()))
    };
    s.headless = true;
    s.agent = true;
    Ok(s)
}

fn print_result(cmd: &str, v: &Value, json_out: bool) {
    if json_out {
        println!("{}", json!({"cmd": cmd, "ok": true, "result": v}));
    } else {
        let text = serde_json::to_string_pretty(v).unwrap_or_default();
        if text.len() > 4000 {
            println!("{cmd}: ok ({} bytes of output; use --json to see all)", text.len());
        } else {
            println!("{cmd}: {text}");
        }
    }
}

/// Entry point; returns the process exit code.
pub fn main_with(args: Vec<String>) -> i32 {
    match run_cli(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn run_cli(args: Vec<String>) -> Result<(), String> {
    let Some(sub) = args.first().cloned() else {
        print!("{MAIN_HELP}");
        return Ok(());
    };
    let rest: Vec<String> = args[1..].to_vec();
    let help = has(&rest, "--help") || has(&rest, "-h");
    let reg = Registry::get();
    match sub.as_str() {
        "help" | "--help" | "-h" => {
            match rest.first().map(|s| s.as_str()) {
                Some("run") => print!("{RUN_HELP}"),
                Some("batch") => print!("{BATCH_HELP}"),
                Some("mcp") => print!("{MCP_HELP}"),
                Some("control") => print!("{CONTROL_HELP}"),
                Some("send") => print!("{SEND_HELP}"),
                Some("new") => print!("{NEW_HELP}"),
                Some(c) if reg.find(c).is_some() => println!("{}", serde_json::to_string_pretty(&reg.describe(c).unwrap()).unwrap()),
                _ => print!("{MAIN_HELP}"),
            }
            Ok(())
        }
        "--version" | "version" => {
            println!("plancraft-cli {} ({} commands)", env!("CARGO_PKG_VERSION"), reg.len());
            Ok(())
        }
        "run" => {
            if help {
                print!("{RUN_HELP}");
                return Ok(());
            }
            let file = first_positional(&rest, &["--cmd", "--params", "--out"]);
            let mut s = open_session(file.as_deref(), has(&rest, "--sample"))?;
            let json_out = has(&rest, "--json");
            let quiet = has(&rest, "--quiet");
            let mut i = 0;
            let mut ran = 0;
            while i < rest.len() {
                if rest[i] == "--cmd" {
                    let cmd = rest.get(i + 1).ok_or("--cmd needs a command id")?.clone();
                    let mut params = json!({});
                    if rest.get(i + 2).map(|a| a == "--params").unwrap_or(false) {
                        params = parse_json_arg(rest.get(i + 3).ok_or("--params needs JSON")?)?;
                        i += 2;
                    }
                    let r = reg.run(&mut s, &cmd, &params).map_err(|e| format!("{cmd}: {e}"))?;
                    if !quiet {
                        print_result(&cmd, &r, json_out);
                    }
                    ran += 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if let Some(out) = arg_val(&rest, "--out") {
                io::write_any(&s, Path::new(&out))?;
                if !quiet {
                    eprintln!("wrote {out}");
                }
            } else if has(&rest, "--save") {
                let f = file.ok_or("--save needs an input FILE")?;
                io::write_any(&s, Path::new(&f))?;
                if !quiet {
                    eprintln!("saved {f}");
                }
            } else if ran > 0 && s.dirty && !quiet {
                eprintln!("note: changes were not written (add --out FILE or --save)");
            }
            Ok(())
        }
        "batch" => {
            if help {
                print!("{BATCH_HELP}");
                return Ok(());
            }
            let actions = arg_val(&rest, "--actions").ok_or("batch needs --actions FILE")?;
            let input = arg_val(&rest, "--in").ok_or("batch needs --in DIR")?;
            let output = arg_val(&rest, "--out").ok_or("batch needs --out DIR")?;
            let fmt = arg_val(&rest, "--format");
            let av = parse_json_arg(&format!("@{actions}"))?;
            let steps = match av {
                Value::Array(a) => a,
                Value::Object(ref o) if o.contains_key("steps") => o["steps"].as_array().cloned().unwrap_or_default(),
                _ => return Err("the actions file must be a list of {\"cmd\",\"params\"} steps".into()),
            };
            std::fs::create_dir_all(&output).map_err(|e| format!("can't create {output}: {e}"))?;
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&input)
                .map_err(|e| format!("can't read {input}: {e}"))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && io::is_project_format(p))
                .collect();
            entries.sort();
            if entries.is_empty() {
                return Err(format!("no .plan.json, .json or .xml plans in {input}"));
            }
            let mut failed = 0;
            for path in entries {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let result = (|| -> Result<PathBuf, String> {
                    let mut s = open_session(Some(&path.to_string_lossy()), false)?;
                    for (k, st) in steps.iter().enumerate() {
                        let cmd = st.get("cmd").or_else(|| st.get("command")).and_then(|c| c.as_str()).ok_or_else(|| format!("step {} has no cmd", k + 1))?;
                        reg.run(&mut s, cmd, &st.get("params").cloned().unwrap_or(json!({}))).map_err(|e| format!("step {} ({cmd}): {e}", k + 1))?;
                    }
                    let stem = name.trim_end_matches(".plan.json").trim_end_matches(".json").trim_end_matches(".xml").to_string();
                    let out_name = match &fmt {
                        Some(f) => format!("{stem}.{}", f.trim_start_matches('.')),
                        None => name.clone(),
                    };
                    let out = Path::new(&output).join(out_name);
                    io::write_any(&s, &out)?;
                    Ok(out)
                })();
                match result {
                    Ok(out) => println!("ok    {name} -> {}", out.display()),
                    Err(e) => {
                        failed += 1;
                        println!("FAIL  {name}: {e}");
                    }
                }
            }
            if failed > 0 {
                Err(format!("{failed} plan(s) failed"))
            } else {
                Ok(())
            }
        }
        "new" => {
            if help {
                print!("{NEW_HELP}");
                return Ok(());
            }
            let out = arg_val(&rest, "--out").ok_or("new needs --out FILE")?;
            let s = if has(&rest, "--sample") {
                crate::sample::sample_session()
            } else {
                let mut s = open_session(None, false)?;
                let mut params = json!({});
                if let Some(t) = arg_val(&rest, "--title") {
                    params["title"] = json!(t);
                }
                if let Some(d) = arg_val(&rest, "--start") {
                    params["start"] = json!(d);
                }
                reg.run(&mut s, "file.new", &params)?;
                s
            };
            io::write_any(&s, Path::new(&out))?;
            println!("wrote {out}");
            Ok(())
        }
        "commands" => {
            if help {
                print!("{OTHER_HELP}");
                return Ok(());
            }
            let list = reg.list_json(arg_val(&rest, "--filter").as_deref());
            if has(&rest, "--json") {
                println!("{}", serde_json::to_string_pretty(&list).unwrap());
            } else {
                let arr = list.as_array().unwrap();
                for c in arr {
                    println!("{:<44} {}", c["id"].as_str().unwrap_or(""), c["title"].as_str().unwrap_or(""));
                }
                println!("\n{} of {} commands. Details: plancraft-cli describe <id>", arr.len(), reg.len());
            }
            Ok(())
        }
        "describe" => {
            let id = rest.first().filter(|a| !a.starts_with("--")).ok_or("describe needs a command id")?;
            let d = reg.describe(id).ok_or_else(|| format!("no command '{id}'"))?;
            println!("{}", serde_json::to_string_pretty(&d).unwrap());
            Ok(())
        }
        "info" => {
            if help {
                print!("{OTHER_HELP}");
                return Ok(());
            }
            let file = first_positional(&rest, &[]).ok_or("info needs a FILE")?;
            let mut s = open_session(Some(&file), false)?;
            let v = reg.run(&mut s, "project.summary", &json!({}))?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            Ok(())
        }
        "report" => {
            if help {
                print!("{OTHER_HELP}");
                return Ok(());
            }
            let file = first_positional(&rest, &["--report"]);
            let mut s = open_session(file.as_deref(), file.is_none())?;
            let id = arg_val(&rest, "--report").unwrap_or_else(|| "projectOverview".into());
            let v = reg.run(&mut s, &format!("report.{id}"), &json!({}))?;
            if has(&rest, "--json") {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                print!("{}", crate::reports::to_markdown(&v));
            }
            Ok(())
        }
        "control" => {
            if help {
                print!("{CONTROL_HELP}");
                return Ok(());
            }
            let file = first_positional(&rest, &[]);
            let mut s = open_session(file.as_deref(), has(&rest, "--sample"))?;
            control::run_stdio(&mut s);
            Ok(())
        }
        "send" => {
            if help {
                print!("{SEND_HELP}");
                return Ok(());
            }
            let cmd = arg_val(&rest, "--cmd").ok_or("send needs --cmd ID")?;
            let params = match arg_val(&rest, "--params") {
                Some(p) => parse_json_arg(&p)?,
                None => json!({}),
            };
            let port = arg_val(&rest, "--port").and_then(|p| p.parse().ok()).unwrap_or(crate::DEFAULT_PORT);
            let v = control::send(port, &cmd, &params)?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            Ok(())
        }
        "mcp" => {
            if help {
                print!("{MCP_HELP}");
                return Ok(());
            }
            let file = first_positional(&rest, &["--port"]);
            let bridge = if has(&rest, "--bridge") { Some(arg_val(&rest, "--port").and_then(|p| p.parse().ok()).unwrap_or(crate::DEFAULT_PORT)) } else { None };
            let session = open_session(file.as_deref(), has(&rest, "--sample"))?;
            McpServer { session, bridge }.run_stdio();
            Ok(())
        }
        other => Err(format!("unknown subcommand '{other}'. Try: plancraft-cli help")),
    }
}
