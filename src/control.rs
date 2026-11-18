//! The JSON control channel: one JSON object per line in, one per line out.
//!
//! Request:  {"id": 1, "cmd": "task.insert", "params": {"name": "Pour concrete", "duration": "3d"}}
//! Reply:    {"id": 1, "ok": true, "result": {...}}  or  {"id": 1, "ok": false, "error": "..."}
//! Also:     {"cmd": "list"} (every command), {"cmd": "describe", "params": {"command": "task.insert"}}
//!
//! The running app listens on 127.0.0.1 (port 47811 by default); `plancraft-cli control` speaks
//! the same protocol on stdin/stdout without a window.

use crate::commands::Registry;
use crate::session::Session;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;

pub struct ControlRequest {
    pub line: String,
    pub reply: mpsc::Sender<String>,
}

/// Run one request line against a session and return the reply line.
pub fn handle_line(s: &mut Session, line: &str) -> String {
    let req: Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "error": format!("not JSON: {e}")}).to_string(),
    };
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let cmd = req.get("cmd").or_else(|| req.get("command")).or_else(|| req.get("method")).and_then(|c| c.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(json!({}));
    let reg = Registry::get();
    let res = match cmd {
        "" => Err("missing \"cmd\"".to_string()),
        "list" => Ok(reg.list_json(params.get("filter").and_then(|f| f.as_str()))),
        "describe" => {
            let c = params.get("command").and_then(|c| c.as_str()).unwrap_or("");
            reg.describe(c).ok_or_else(|| format!("no command '{c}'"))
        }
        "ping" => Ok(json!({"app": crate::APP_NAME, "commands": reg.len(), "title": s.project.title})),
        c => {
            let was = s.agent;
            s.agent = true;
            let r = reg.run(s, c, &params);
            s.agent = was;
            r
        }
    };
    match res {
        Ok(r) => json!({"id": id, "ok": true, "result": r}).to_string(),
        Err(e) => json!({"id": id, "ok": false, "error": e}).to_string(),
    }
}

/// Start the listener for the window. Requests are passed to the UI thread through `tx`;
/// `wake` asks the window to draw a frame so it handles them promptly.
pub fn start_server(port: u16, tx: mpsc::Sender<ControlRequest>, wake: impl Fn() + Send + Sync + Clone + 'static) -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("control channel: can't listen on 127.0.0.1:{port}: {e}"))?;
    let actual = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            let wake = wake.clone();
            std::thread::spawn(move || serve_conn(stream, tx, wake));
        }
    });
    Ok(actual)
}

fn serve_conn(stream: TcpStream, tx: mpsc::Sender<ControlRequest>, wake: impl Fn()) {
    let peer_ok = stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
    if !peer_ok {
        return;
    }
    let mut out = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let (rtx, rrx) = mpsc::channel();
        if tx.send(ControlRequest { line, reply: rtx }).is_err() {
            break;
        }
        wake();
        let reply = rrx.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_| json!({"ok": false, "error": "the app did not answer in time"}).to_string());
        if writeln!(out, "{reply}").is_err() {
            break;
        }
        let _ = out.flush();
    }
}

/// Send one command to a running app and return its reply.
pub fn send(port: u16, cmd: &str, params: &Value) -> Result<Value, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("no app is listening on 127.0.0.1:{port} ({e}). Start PlanCraft first."))?;
    stream.set_read_timeout(Some(Duration::from_secs(90))).ok();
    let req = json!({"id": 1, "cmd": cmd, "params": params});
    writeln!(stream, "{req}").map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(line.trim()).map_err(|e| format!("bad reply from the app: {e}"))?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(v["result"].clone())
    } else {
        Err(v["error"].as_str().unwrap_or("error").to_string())
    }
}

/// `plancraft-cli control`: JSON lines on stdin/stdout against a headless session.
pub fn run_stdio(s: &mut Session) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = handle_line(s, &line);
        let _ = writeln!(stdout, "{reply}");
        let _ = stdout.flush();
    }
}
