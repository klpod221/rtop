//! MCP (Model Context Protocol) server — JSON-RPC 2.0 over stdio.
//!
//! Reads newline-delimited JSON from stdin, writes responses to stdout.
//! All diagnostic output goes to stderr to avoid corrupting the RPC stream.

use std::io::{BufRead, Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use app_collector::gpu::intel::IntelGpuCollector;
use app_config::Config;

// ─── JSON-RPC types ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

// ─── Server ───────────────────────────────────────────────────────────────────

/// Runs the MCP server until stdin closes. Blocks the current thread.
pub fn run(cfg: &Config) -> Result<()> {
    // Diagnostic logs go to stderr so stdout stays clean for JSON-RPC.
    // Caller (CLI) sets up the logger; we just ensure panic messages reach stderr.

    // Warm-up GPU collectors
    #[cfg(feature = "nvidia")]
    app_collector::gpu::nvidia::init();

    let mut intel_col: Option<IntelGpuCollector> = IntelGpuCollector::new().ok();
    if let Some(ref mut c) = intel_col { c.collect(); }
    app_collector::cpu::collect().ok();

    let stdin  = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }

        match serde_json::from_str::<Request>(&line) {
            Ok(req)  => handle(&req, cfg, intel_col.as_mut(), &mut out)?,
            Err(_)   => send_error(None, -32700, "Parse error", &mut out)?,
        }
    }
    Ok(())
}

fn handle(
    req: &Request,
    cfg: &Config,
    intel_col: Option<&mut IntelGpuCollector>,
    out: &mut impl Write,
) -> Result<()> {
    match req.method.as_str() {
        "initialize" => send_result(req.id.clone(), serde_json::json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "rtop", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": {} },
        }), out),

        "notifications/initialized" => Ok(()), // no response for notifications

        "tools/list" => send_result(req.id.clone(), serde_json::json!({
            "tools": [{
                "name": "get_metrics",
                "description": "Get real-time system metrics (CPU, Memory, Disk, Network, Processes, GPU)",
                "inputSchema": { "type": "object", "properties": {} }
            }]
        }), out),

        "tools/call" => {
            let tool_name = req.params.as_ref()
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");

            if tool_name == "get_metrics" {
                let payload = app_collector::collect_all(cfg, intel_col, None, None);
                // Trim process list to avoid LLM context overflow
                let mut payload = payload;
                payload.processes.truncate(20);
                let json = serde_json::to_value(&payload)?;
                send_result(req.id.clone(), serde_json::json!({
                    "content": [{ "type": "text", "text": json.to_string() }]
                }), out)
            } else {
                send_error(req.id.clone(), -32601, "Tool not found", out)
            }
        }

        _ => send_error(req.id.clone(), -32601, "Method not found", out),
    }
}

fn send_result(id: Option<Value>, result: Value, out: &mut impl Write) -> Result<()> {
    let Some(id) = id else { return Ok(()); }; // notification — no response
    write_response(Response { jsonrpc: "2.0", id: Some(id), result: Some(result), error: None }, out)
}

fn send_error(id: Option<Value>, code: i32, message: &str, out: &mut impl Write) -> Result<()> {
    let Some(id) = id else { return Ok(()); };
    write_response(Response {
        jsonrpc: "2.0",
        id: Some(id),
        result: None,
        error: Some(RpcError { code, message: message.to_string() }),
    }, out)
}

fn write_response(resp: Response, out: &mut impl Write) -> Result<()> {
    serde_json::to_writer(&mut *out, &resp)?;
    writeln!(out)?;
    out.flush()?;
    Ok(())
}
