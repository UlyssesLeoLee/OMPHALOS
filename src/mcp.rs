use crate::{engine, hooks, model::{Config, Result}, store};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

fn tool(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
    json!({"name":name,"description":description,
        "inputSchema":{"type":"object","properties":properties,"required":required}})
}

fn list_tools() -> Value {
    json!({"tools":[
        tool("requirements_get","Get approved requirement by ID with source and line",json!({"id":{"type":"string"}}),vec!["id"]),
        tool("requirements_check","Retrieve and compare new user requirement with approved requirements (heuristic)",json!({"text":{"type":"string"}}),vec!["text"]),
        tool("concept_discuss","Get pending Concept disagreement: clauses, locations, options",json!({}),vec![]),
        tool("concept_decide","Propose a decision for HUMAN terminal approval; this NEVER unlocks the gate",json!({"option":{"type":"string","enum":["A","B","C","D"]},"reason":{"type":"string"}}),vec!["option","reason"])
    ]})
}

fn run_tool(root: &Path, cfg: &Config, name: &str, args: &Value) -> Result<Value> {
    let b = store::load_baseline(root)?;
    match name {
        "requirements_get" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("missing id")?;
            Ok(json!(b.requirements.iter().find(|r| r.id == id)))
        }
        "requirements_check" => {
            let prompt = args.get("text").and_then(Value::as_str).ok_or("missing text")?;
            let config_digest = crate::parse::digest(&std::fs::read_to_string(crate::model::meta(root, "config.toml"))?);
            let key = crate::parse::digest(&format!("{}|{}|{}|mcp-v2",b.digest,config_digest,prompt));
            if let Some(a) = store::cache_get(root, &key) { return Ok(serde_json::to_value(a)?); }
            let a = engine::assess(prompt, &b, cfg, "user_prompt");
            store::cache_put(root, &key, &a)?;
            Ok(serde_json::to_value(a)?)
        }
        "concept_discuss" => {
            let p = hooks::pending_file(root);
            if !p.exists() { return Ok(json!({"status":"none"})); }
            Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?)
        }
        "concept_decide" => {
            let option = args.get("option").and_then(Value::as_str).ok_or("missing option")?;
            let reason = args.get("reason").and_then(Value::as_str).ok_or("missing reason")?;
            if !["A","B","C","D"].contains(&option) || reason.trim().len() < 3 { return Err("invalid option or reason".into()); }
            store::audit(root,"agent_decision_proposal", json!({"option":option,"reason":reason}))?;
            Ok(json!({"status":"proposal_only","option":option,"approval_command":"concept-guard decision A --reason <human reason>","warning":"MCP tools cannot approve or unlock requirements"}))
        }
        _ => Err(format!("Unknown tool: {name}").into()),
    }
}

pub fn serve(root: &Path, cfg: &Config) -> Result<()> {
    // Compatibility mode: MCP protocol 2025-06-18 newline-framed JSON-RPC over stdio.
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.len() > 1_000_000 { continue; }
        let v: Value = match serde_json::from_str(&line) { Ok(x) => x, Err(_) => continue };
        let id = v.get("id").cloned();
        let method = v.get("method").and_then(Value::as_str).unwrap_or("");
        if id.is_none() { continue; } // notification, no reply
        let result: Result<Value> = match method {
            "initialize" => Ok(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},
                  "serverInfo":{"name":"concept-guard","version":"0.1.0"}})),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(list_tools()),
            "tools/call" => {
                let p = v.get("params").cloned().unwrap_or(Value::Null);
                let name = p.get("name").and_then(Value::as_str).unwrap_or("");
                let args = p.get("arguments").cloned().unwrap_or(json!({}));
                match run_tool(root,cfg,name,&args) {
                    Ok(value) => Ok(json!({"content":[{"type":"text","text":serde_json::to_string(&value)?}],"isError":false})),
                    Err(e) => Ok(json!({"content":[{"type":"text","text":e.to_string()}],"isError":true})),
                }
            }
            _ => Err(format!("Unknown method {method}").into()),
        };
        let resp = match result {
            Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
            Err(err) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":err.to_string()}}),
        };
        writeln!(stdout,"{}",serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}
