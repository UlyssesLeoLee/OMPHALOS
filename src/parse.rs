use crate::model::{Config, Constraint, Requirement, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn digest(input: &str) -> String {
    let bytes = Sha256::digest(input.as_bytes());
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn load_config(root: &Path) -> Result<Config> {
    let file = root.join(".concept-guard/config.toml");
    let text = fs::read_to_string(&file)
        .map_err(|e| format!("Cannot read {}: {e}. Run setup or supply a config.", file.display()))?;
    Ok(toml::from_str(&text)?)
}

fn collect(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let canonical = path.canonicalize()?;
    let parent = root.canonicalize()?;
    if !canonical.starts_with(parent) { return Err(format!("Path outside workspace: {}", path.display()).into()); }
    if canonical.is_dir() {
        for child in fs::read_dir(&canonical)? { collect(root, &child?.path(), files)?; }
    } else if let Some(ext) = canonical.extension().and_then(|s| s.to_str()) {
        if ["md", "markdown", "txt"].contains(&ext.to_ascii_lowercase().as_str()) { files.push(canonical); }
    }
    Ok(())
}

pub fn parse_project(root: &Path, cfg: &Config) -> Result<Vec<Requirement>> {
    let mut files = Vec::new();
    if cfg.requirements.paths.is_empty() { return Err("requirements.paths cannot be empty".into()); }
    for relative in &cfg.requirements.paths {
        if relative.contains('*') { return Err("Glob paths unsupported: use directory or file paths".into()); }
        let full = root.join(relative);
        if !full.exists() { return Err(format!("Missing requirements source: {}", full.display()).into()); }
        collect(root, &full, &mut files)?;
    }
    files.sort(); files.dedup();
    if files.is_empty() { return Err("No .md/.txt requirements files found".into()); }
    let mut all = Vec::new();
    let mut ids = HashSet::new();
    for file in files {
        let rel = file.strip_prefix(root.canonicalize()?)?.to_string_lossy().replace('\\', "/");
        let text = fs::read_to_string(&file)?;
        if text.trim().is_empty() { continue; }
        for req in parse_document(&rel, &text) {
            if !ids.insert(req.id.clone()) { return Err(format!("Duplicate Requirement ID: {}", req.id).into()); }
            all.push(req);
        }
    }
    if all.is_empty() { return Err("No requirement clauses extracted".into()); }
    all.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(all)
}

fn extract_id(heading: &str) -> Option<String> {
    let s = heading.trim();
    if s.starts_with('[') {
        if let Some(end) = s.find(']') {
            let inside = &s[1..end];
            if inside.starts_with("REQ-") && inside.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return Some(inside.to_owned()); }
        }
    }
    let first = s.split_whitespace().next().unwrap_or("");
    if first.starts_with("REQ-") && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return Some(first.to_owned()); }
    None
}

pub fn parse_document(relative: &str, text: &str) -> Vec<Requirement> {
    let mut chunks: Vec<(String, usize, String)> = Vec::new();
    let mut current_heading = "Document requirements".to_string();
    let mut start = 1_usize;
    let mut buffer = String::new();
    let mut in_fence = false;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") { in_fence = !in_fence; }
        let new_heading = !in_fence && (trimmed.starts_with("## ") || trimmed.starts_with("### ") || trimmed.starts_with("#### "));
        if new_heading {
            if !buffer.trim().is_empty() {
                chunks.push((current_heading.clone(), start, buffer.trim().to_string()));
            }
            current_heading = trimmed.trim_start_matches('#').trim().to_string();
            start = i + 1;
            buffer.clear();
        } else if !trimmed.starts_with("```") && !in_fence {
            if !trimmed.is_empty() { buffer.push_str(raw); buffer.push('\n'); }
        }
    }
    if !buffer.trim().is_empty() { chunks.push((current_heading, start, buffer.trim().to_string())); }
    let mut result = Vec::new();
    let mut used_auto = HashSet::new();
    for (section, line, content) in chunks {
        let id = if let Some(id) = extract_id(&section) { id } else {
            let mut candidate = format!("AUTO-{}", &digest(&format!("{relative}|{section}"))[..12]);
            if !used_auto.insert(candidate.clone()) {
                candidate = format!("AUTO-{}", &digest(&format!("{relative}|{section}|{line}"))[..12]);
            }
            candidate
        };
        let mut constraints = Vec::new();
        for raw in content.lines() {
            let cleaned = raw.trim().trim_start_matches(&['-', '*', ' '][..]).trim();
            if cleaned.is_empty() { continue; }
            let low = cleaned.to_lowercase();
            let kind = if low.starts_with("must_not:") || low.starts_with("must-not:")
                || low.starts_with("禁止") || low.starts_with("不得") || low.starts_with("不允许")
                || low.starts_with("must not") || low.starts_with("shall not") { "must_not" }
            else if low.starts_with("must:") || low.starts_with("必须") || low.starts_with("应当") || low.starts_with("shall:") { "must" }
            else { "ordinary" };
            constraints.push(Constraint { kind: kind.to_owned(), text: cleaned.to_string() });
        }
        let fingerprint = digest(&format!("{section}\n{content}"));
        result.push(Requirement { id, source: relative.to_owned(), section, line, digest: fingerprint, content, constraints });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn explicit_and_auto_ids() {
        let rs = parse_document("r.md", "## [REQ-A-1] One\n- MUST_NOT: Do not repeat OCR\n## Two\n- MUST: save CSV");
        assert_eq!(rs.len(), 2);
        assert_eq!(rs[0].id, "REQ-A-1");
        assert!(rs[1].id.starts_with("AUTO-"));
        assert_eq!(rs[0].constraints[0].kind, "must_not");
    }
    #[test] fn ignore_fenced_headings() {
        let rs = parse_document("r.md", "## Real\nText\n```\n## Fake\n```\nAnother");
        assert_eq!(rs.len(), 1);
    }
}
