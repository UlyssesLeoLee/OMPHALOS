use crate::model::{meta, Assessment, Baseline, Requirement, Result};
use crate::parse::digest;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> String { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs().to_string() }

fn database(root: &Path) -> Result<Connection> {
    fs::create_dir_all(meta(root, ""))?;
    let conn = Connection::open(meta(root, "cache.sqlite"))?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, assessment_json TEXT NOT NULL);\
                        CREATE TABLE IF NOT EXISTS req_index (id TEXT PRIMARY KEY, content TEXT NOT NULL, source TEXT NOT NULL, line INTEGER NOT NULL);")?;
    Ok(conn)
}

pub fn project_digest(requirements: &[Requirement]) -> Result<String> {
    let mut items: Vec<(String, String, String)> = requirements.iter().map(|r| (r.id.clone(), r.source.clone(), r.digest.clone())).collect();
    items.sort(); Ok(digest(&serde_json::to_string(&items)?))
}

pub fn draft(root: &Path, project: &str, rs: Vec<Requirement>) -> Result<Baseline> {
    let b = Baseline { schema_version: 1, project: project.into(), digest: project_digest(&rs)?, approved_at_utc: "NOT_APPROVED".into(), requirements: rs };
    fs::create_dir_all(meta(root, ""))?;
    fs::write(meta(root, "pending.json"), serde_json::to_vec_pretty(&b)?)?;
    Ok(b)
}

pub fn approve(root: &Path) -> Result<Baseline> {
    if !io::stdin().is_terminal() { return Err("Approval requires a human terminal (TTY); noninteractive approval is denied".into()); }
    let raw = fs::read_to_string(meta(root, "pending.json"))?;
    let mut b: Baseline = serde_json::from_str(&raw)?;
    if b.digest != project_digest(&b.requirements)? { return Err("Pending baseline digest invalid".into()); }
    eprintln!("About to approve {} requirement anchors. Digest {}", b.requirements.len(), b.digest);
    eprint!("Type APPROVE {} to continue: ", &b.digest[..12]); io::stderr().flush()?;
    let mut line = String::new(); io::stdin().read_line(&mut line)?;
    if line.trim() != format!("APPROVE {}", &b.digest[..12]) { return Err("Approval canceled".into()); }
    b.approved_at_utc = now();
    fs::write(meta(root, "approved.json"), serde_json::to_vec_pretty(&b)?)?;
    refresh_index(root, &b)?;
    // Approval of a new baseline supersedes pending reviews against an older baseline.
    let stale_discussion = meta(root, "pending_discussion.json");
    if stale_discussion.exists() { fs::remove_file(stale_discussion)?; }
    let old_permit = meta(root, "change_permit.json");
    if old_permit.exists() { fs::remove_file(old_permit)?; }
    audit(root, "baseline_approved", json!({ "digest": b.digest, "count": b.requirements.len() }))?;
    Ok(b)
}

pub fn load_baseline(root: &Path) -> Result<Baseline> {
    let b: Baseline = serde_json::from_str(&fs::read_to_string(meta(root, "approved.json"))?)?;
    if b.schema_version != 1 || b.digest != project_digest(&b.requirements)? { return Err("Approved baseline integrity check failed".into()); }
    if b.approved_at_utc == "NOT_APPROVED" { return Err("Not yet approved".into()); }
    Ok(b)
}

pub fn refresh_index(root: &Path, b: &Baseline) -> Result<()> {
    let mut conn = database(root)?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM req_index", [])?;
    for r in &b.requirements {
        tx.execute("INSERT INTO req_index(id,content,source,line) VALUES (?1,?2,?3,?4)", params![r.id, r.content, r.source, r.line])?;
    }
    tx.commit()?;
    // FTS5 is an acceleration layer; the core comparator works even when FTS5 is unavailable.
    let _ = conn.execute_batch("CREATE VIRTUAL TABLE IF NOT EXISTS req_fts USING fts5(id UNINDEXED, content, tokenize='trigram'); DELETE FROM req_fts;");
    for r in &b.requirements { let _ = conn.execute("INSERT INTO req_fts(id,content) VALUES (?1,?2)", params![r.id, r.content]); }
    Ok(())
}

pub fn cache_get(root: &Path, key: &str) -> Option<Assessment> {
    let conn = database(root).ok()?;
    let raw: String = conn.query_row("SELECT assessment_json FROM cache WHERE key=?1", [key], |row| row.get(0)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn cache_put(root: &Path, key: &str, a: &Assessment) -> Result<()> {
    let conn = database(root)?;
    conn.execute("INSERT OR REPLACE INTO cache(key,assessment_json) VALUES (?1,?2)", params![key, serde_json::to_string(a)?])?;
    Ok(())
}

pub fn audit(root: &Path, kind: &str, payload: Value) -> Result<()> {
    fs::create_dir_all(meta(root, ""))?;
    let path = meta(root, "audit.jsonl");
    let prev = fs::read_to_string(&path).unwrap_or_default();
    let prior = prev.lines().last().and_then(|l| serde_json::from_str::<Value>(l).ok())
        .and_then(|v| v.get("chain_hash").and_then(Value::as_str).map(str::to_owned)).unwrap_or_else(|| "GENESIS".into());
    let body = json!({"kind":kind,"epoch":now(),"payload":payload,"previous_hash":prior});
    let chain_hash = digest(&serde_json::to_string(&body)?);
    let entry = json!({"body":body,"chain_hash":chain_hash});
    let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    Ok(())
}
