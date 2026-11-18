//! A Model Context Protocol server on stdin/stdout (JSON-RPC 2.0, one message per line).
//! Headless it owns its own session; with `--bridge` it drives the running window instead.

use crate::commands::Registry;
use crate::session::Session;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub struct McpServer {
    pub session: Session,
    pub bridge: Option<u16>,
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": description, "inputSchema": {"type": "object", "properties": props, "required": required}})
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "run_command",
            "Run one PlanCraft command (the same commands the menus, ribbon and dialogs use). Use list_commands to find ids and describe_command for parameters. Task IDs are row numbers.",
            json!({"command": {"type": "string", "description": "command id, e.g. task.insert"}, "params": {"type": "object", "description": "the command's parameters"}}),
            &["command"],
        ),
        tool(
            "run_commands",
            "Run several commands in order; stops at the first error.",
            json!({"steps": {"type": "array", "items": {"type": "object", "properties": {"cmd": {"type": "string"}, "params": {"type": "object"}}, "required": ["cmd"]}}}),
            &["steps"],
        ),
        tool("list_commands", "List command ids and titles, optionally filtered by text or category (file, edit, task, resource, project, calendar, view, format, report, help, app, assignment).", json!({"filter": {"type": "string"}}), &[]),
        tool("describe_command", "A command's help text and parameters.", json!({"command": {"type": "string"}}), &["command"]),
        tool("get_project_summary", "Dates, duration, work, cost, % complete and warnings of the open project.", json!({}), &[]),
        tool("list_tasks", "Every task with ID, name, outline level, duration, dates, links, resources and progress.", json!({}), &[]),
        tool("list_resources", "Every resource with rates, work, cost and overallocation.", json!({}), &[]),
        tool("open_project", "Open a project file (.plan.json or .xml).", json!({"path": {"type": "string"}}), &["path"]),
        tool("save_project", "Save the project (the extension picks the format: .plan.json, .xml, .csv, .svg, .html, .md).", json!({"path": {"type": "string"}}), &[]),
    ]
}

impl McpServer {
    fn exec(&mut self, cmd: &str, params: &Value) -> Result<Value, String> {
        match self.bridge {
            Some(port) => crate::control::send(port, cmd, params),
            None => {
                self.session.agent = true;
                Registry::get().run(&mut self.session, cmd, params)
            }
        }
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        let reg = Registry::get();
        match name {
            "run_command" => {
                let cmd = args.get("command").and_then(|c| c.as_str()).ok_or("missing 'command'")?;
                let params = args.get("params").cloned().unwrap_or(json!({}));
                self.exec(cmd, &params)
            }
            "run_commands" => {
                let steps = args.get("steps").cloned().ok_or("missing 'steps'")?;
                self.exec("app.batch", &json!({"steps": steps}))
            }
            "list_commands" => Ok(reg.list_json(args.get("filter").and_then(|f| f.as_str()))),
            "describe_command" => {
                let c = args.get("command").and_then(|c| c.as_str()).unwrap_or("");
                reg.describe(c).ok_or_else(|| format!("no command '{c}'"))
            }
            "get_project_summary" => self.exec("project.summary", &json!({})),
            "list_tasks" => self.exec("task.list", &json!({})),
            "list_resources" => self.exec("resource.list", &json!({})),
            "open_project" => self.exec("file.open", args),
            "save_project" => self.exec("file.save", args),
            other => Err(format!("unknown tool '{other}'")),
        }
    }

    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let result: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("2025-06-18"),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "plancraft", "version": env!("CARGO_PKG_VERSION")},
                "instructions": format!(
                    "PlanCraft schedules projects. Every action is one of {} commands. Typical flow: run_command task.insert {{name, duration:'5d', predecessors:'2'}}, resource.assign {{ids:[3], resource:'Ana'}}, then get_project_summary. {}",
                    Registry::get().len(),
                    if self.bridge.is_some() { "Commands go to the running PlanCraft window." } else { "This server holds its own project; save it with save_project." }
                )
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match self.call_tool(name, &args) {
                    Ok(v) => Ok(json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}], "isError": false})),
                    Err(e) => Ok(json!({"content": [{"type": "text", "text": e}], "isError": true})),
                }
            }
            "resources/list" => Ok(json!({"resources": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            m if m.starts_with("notifications/") => return None,
            m => Err((-32601, format!("method not found: {m}"))),
        };
        let id = id?;
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        })
    }

    pub fn run_stdio(&mut self) {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(Value::Array(batch)) => {
                    let out: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                    if out.is_empty() {
                        None
                    } else {
                        Some(Value::Array(out))
                    }
                }
                Ok(msg) => self.handle(&msg),
                Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
            };
            if let Some(r) = reply {
                let _ = writeln!(stdout, "{r}");
                let _ = stdout.flush();
            }
        }
    }
}
