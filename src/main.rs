mod engine;
mod hooks;
mod mcp;
mod model;
mod parse;
mod store;

use model::{meta, Result};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn args() -> (PathBuf, Vec<String>) {
    let mut root = env::var("CONCEPT_GUARD_PROJECT_ROOT").map(PathBuf::from).unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut rest = Vec::new();
    let mut iter = env::args().skip(1);
    while let Some(x) = iter.next() {
        if x == "--root" { if let Some(v) = iter.next() { root = PathBuf::from(v); } }
        else { rest.push(x); }
    }
    (root, rest)
}

fn help() {
    println!("Concept Guard 0.1.0 — requirement-only local Concept enforcement\n\
Usage: concept-guard [--root PROJECT] COMMAND\n\
  setup                          Create a sample config if missing\n\
  init                           Extract EXISTING requirements into pending.json (not approved)\n\
  approve                        Human TTY approval; create locked approved.json\n\
  status                         Baseline and pending review status\n\
  check TEXT                     Compare incoming user instruction to approved requirements\n\
  check-baseline                 Fail if approved requirements changed without approval\n\
  scan-staged                    Scan Git staged diff for candidate divergence\n\
  scan-branch BASE               Scan diff from BASE to HEAD for candidate divergence\n\
  decision A|B|C|D --reason TEXT  Human-only discussion decision (TTY)\n\
  hook prompt|pretool            Read JSON hook event from stdin; emit JSON decision\n\
  mcp                            Run MCP stdio server (JSON-RPC)\n");
}

fn setup(root: &Path) -> Result<()> {
    fs::create_dir_all(meta(root,""))?;
    let path = meta(root, "config.toml");
    if path.exists() { println!("Config already exists: {}", path.display()); return Ok(()); }
    fs::write(&path, "# Set the path to your EXISTING requirements documents\n[project]\nname = 'my-project'\n\n[requirements]\npaths = ['docs/requirements']\n\n[guard]\ntop_k = 3\nmin_score = 2\n")?;
    println!("Created {}. Point requirements.paths at existing files before init.", path.display());
    Ok(())
}

fn check_baseline(root: &Path) -> Result<()> {
    let cfg = parse::load_config(root)?;
    let approved = store::load_baseline(root)?;
    let current = parse::parse_project(root, &cfg)?;
    let hash = store::project_digest(&current)?;
    if hash != approved.digest {
        let old: std::collections::HashMap<String,String> = approved.requirements.iter().map(|r| (r.id.clone(), r.digest.clone())).collect();
        let new: std::collections::HashMap<String,String> = current.iter().map(|r| (r.id.clone(), r.digest.clone())).collect();
        let mut changes: Vec<String> = Vec::new();
        for (id, digest) in &new {
            match old.get(id) {
                None => changes.push(format!("ADDED {id}")),
                Some(x) if x != digest => changes.push(format!("CHANGED {id}")),
                _ => (),
            }
        }
        for id in old.keys() { if !new.contains_key(id) { changes.push(format!("REMOVED {id}")); } }
        changes.sort();
        return Err(format!("REQUIREMENTS BASELINE MISMATCH. Approval required.\n{}", changes.join("\n")).into());
    }
    println!("PASS: {} approved requirements unchanged; digest {}", current.len(), hash);
    Ok(())
}

fn check(root: &Path, prompt: &str) -> Result<()> {
    let cfg = parse::load_config(root)?;
    let b = store::load_baseline(root)?;
    let config_digest = parse::digest(&fs::read_to_string(meta(root, "config.toml"))?);
    let key = parse::digest(&format!("{}|{}|{}|cli-v2",b.digest,config_digest,prompt));
    let a = if let Some(cached) = store::cache_get(root, &key) { cached } else {
        let analyzed = engine::assess(prompt, &b, &cfg, "user_prompt");
        store::cache_put(root, &key, &analyzed)?;
        analyzed
    };
    println!("{}", serde_json::to_string_pretty(&a)?);
    Ok(())
}

fn scan_diff(root: &Path, base: Option<&str>) -> Result<()> {
    let cfg = parse::load_config(root)?;
    let b = store::load_baseline(root)?;
    let mut cmd = Command::new("git");
    cmd.current_dir(root).arg("diff").arg("--no-ext-diff").arg("--unified=0");
    if let Some(base) = base {
        if base.starts_with('-') { return Err("Invalid git revision".into()); }
        cmd.arg(format!("{base}...HEAD"));
    } else { cmd.arg("--cached"); }
    cmd.arg("--");
    let output = cmd.output()?;
    if !output.status.success() { return Err(format!("git diff failed: {}", String::from_utf8_lossy(&output.stderr)).into()); }
    let diff = String::from_utf8_lossy(&output.stdout);
    let mut path = String::new();
    let mut line = 0_usize;
    let mut issues = Vec::new();
    for raw in diff.lines() {
        if let Some(p) = raw.strip_prefix("+++ b/") { path = p.into(); }
        else if raw.starts_with("@@ ") {
            if let Some(tail) = raw.split('+').nth(1) {
                let number = tail.split(|c: char| c == ',' || c == ' ').next().unwrap_or("0");
                line = number.parse().unwrap_or(0);
            }
        } else if raw.starts_with('+') && !raw.starts_with("+++") && !path.is_empty() {
            let added = raw[1..].trim();
            if added.len() > 2 {
                let a = engine::assess(added, &b, &cfg, "code_diff");
                if a.outcome == "uncertain" && !a.hits.is_empty() || a.outcome == "conflict" {
                    issues.push(json!({"source":path,"line":line,"assessment":a}));
                }
            }
            line += 1;
        } else if !raw.starts_with('-') && !raw.starts_with('\\') && !raw.starts_with("diff ") { line += 1; }
    }
    println!("{}", serde_json::to_string_pretty(&json!({"count":issues.len(),"issues":issues}))?);
    if !issues.is_empty() { return Err(format!("{} candidate divergence(s) require review", issues.len()).into()); }
    Ok(())
}

fn decision(root: &Path, parts: &[String]) -> Result<()> {
    if !io::stdin().is_terminal() { return Err("Decision requires human terminal (TTY)".into()); }
    let opt = parts.first().map(String::as_str).ok_or("Specify A|B|C|D")?;
    if !["A","B","C","D"].contains(&opt) { return Err("Invalid decision option".into()); }
    let idx = parts.iter().position(|s| s == "--reason").ok_or("Required: --reason explanation")?;
    let reason = parts[idx+1..].join(" ");
    if reason.chars().count() < 8 { return Err("Reason must be >= 8 chars".into()); }
    let pending = hooks::pending_file(root);
    if !pending.exists() { return Err("No pending Concept discussion".into()); }
    let p: Value = serde_json::from_str(&fs::read_to_string(&pending)?)?;
    let digest = p["assessment"]["baseline_digest"].as_str().ok_or("Missing pending digest")?;
    if digest != store::load_baseline(root)?.digest { return Err("Stale Concept discussion; retry comparison".into()); }
    eprintln!("DECISION {opt}: {reason}\nPending request: {}",p["prompt"]);
    eprint!("Type CONFIRM {opt} to record this human decision: "); io::stderr().flush()?;
    let mut answer = String::new(); io::stdin().read_line(&mut answer)?;
    if answer.trim() != format!("CONFIRM {opt}") { return Err("Canceled".into()); }
    store::audit(root,"human_concept_decision", json!({"option":opt,"reason":reason,"baseline_digest":digest,"original_prompt":p["prompt"]}))?;
    // B issues a narrow, short-lived human-confirmed permit for the relevant requirements source.
    // It does NOT approve the new baseline; CI and owner review remain mandatory.
    if opt == "B" {
        let sources: Vec<String> = p["assessment"]["hits"].as_array().into_iter().flatten()
            .filter_map(|hit| hit["source"].as_str().map(str::to_owned)).collect();
        if sources.is_empty() { return Err("No relevant requirements source for scoped permit; manual approval procedure required".into()); }
        let expiry = store::now().parse::<u64>()? + 600;
        fs::write(meta(root,"change_permit.json"), serde_json::to_vec_pretty(&json!({
            "sources":sources, "baseline_digest":digest, "expires_epoch":expiry, "reason":reason
        }))?)?;
    }
    // Keep B/D pending: B may edit only specifically permitted requirement files and must publish a new baseline.
    if opt == "A" || opt == "C" {
        fs::remove_file(pending)?;
        let permit = meta(root, "change_permit.json");
        if permit.exists() { fs::remove_file(permit)?; }
    }
    println!("Recorded decision {opt}; B grants a 10-minute source-scoped edit permit, NOT baseline approval. B/D remain pending until formally resolved.");
    Ok(())
}

fn run() -> Result<()> {
    let (root, commands) = args();
    let cmd = commands.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "help" | "--help" => { help(); Ok(()) },
        "setup" => setup(&root),
        "init" => {
            let cfg = parse::load_config(&root)?;
            let b = store::draft(&root, &cfg.project.name, parse::parse_project(&root, &cfg)?)?;
            println!("Pending baseline written: {} anchors, digest {}. Run 'approve' in a human terminal.", b.requirements.len(), b.digest);
            Ok(())
        }
        "approve" => { let b = store::approve(&root)?; println!("Approved baseline: {}", b.digest); Ok(()) }
        "status" => {
            let b = store::load_baseline(&root)?;
            println!("Project: {} | {} approved anchors | digest {} | pending discussion: {}",b.project,b.requirements.len(),b.digest,hooks::pending_file(&root).exists());
            Ok(())
        }
        "check" => check(&root, &commands[1..].join(" ")),
        "check-baseline" => check_baseline(&root),
        "scan-staged" => scan_diff(&root,None),
        "scan-branch" => scan_diff(&root,Some(commands.get(1).map(String::as_str).ok_or("Missing base revision")?)),
        "decision" => decision(&root, &commands[1..]),
        "hook" => { let cfg = parse::load_config(&root)?; hooks::run(&root, &cfg, commands.get(1).map(String::as_str).ok_or("prompt|pretool required")?) },
        "mcp" => { let cfg = parse::load_config(&root)?; mcp::serve(&root, &cfg) },
        _ => Err(format!("Unknown command: {cmd}").into()),
    }
}

fn main() {
    if let Err(e) = run() { eprintln!("Concept Guard error: {e}"); std::process::exit(2); }
}
