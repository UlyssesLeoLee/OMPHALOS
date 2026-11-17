use crate::model::{Assessment, Baseline, Config, Hit, Requirement};
use std::collections::{HashMap, HashSet};

// This is a deliberately conservative *candidate* comparator, not an LLM or a semantic proof.
fn terms(s: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    let mut ascii = String::new();
    let mut cjk = String::new();
    let flush_ascii = |txt: &mut String, out: &mut HashSet<String>| {
        if txt.len() >= 2 { out.insert(txt.to_ascii_lowercase()); }
        txt.clear();
    };
    let flush_cjk = |txt: &mut String, out: &mut HashSet<String>| {
        let chars: Vec<char> = txt.chars().collect();
        if chars.len() == 1 { out.insert(chars[0].to_string()); }
        for pair in chars.windows(2) { out.insert(pair.iter().collect()); }
        if chars.len() >= 3 { for triple in chars.windows(3) { out.insert(triple.iter().collect()); } }
        txt.clear();
    };
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            flush_cjk(&mut cjk, &mut result); ascii.push(c.to_ascii_lowercase());
        } else if matches!(c as u32, 0x3400..=0x9FFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF) {
            flush_ascii(&mut ascii, &mut result); cjk.push(c);
        } else { flush_ascii(&mut ascii, &mut result); flush_cjk(&mut cjk, &mut result); }
    }
    flush_ascii(&mut ascii, &mut result); flush_cjk(&mut cjk, &mut result);
    result
}

fn positive_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["增加", "新增", "引入", "启用", "添加", "开启", "接入", "实现", "支持", "使える", "追加", "導入", "enable", "add ", "introduce", "implement"].iter().any(|w| lower.contains(w))
}
fn negative_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["不要", "取消", "禁用", "移除", "禁止", "不需要", "废弃", "廃止", "除外", "remove", "disable", "avoid"].iter().any(|w| lower.contains(w))
}

fn key_signal(a: &str, b: &str) -> bool {
    // Stronger than just a CJK common-character overlap: shared named technical token.
    let at = terms(a); let bt = terms(b);
    at.intersection(&bt).any(|s| s.len() >= 3 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        || at.intersection(&bt).filter(|s| s.chars().count() >= 3).count() >= 2
}

fn expanded(text: &str, config: &Config) -> String {
    let mut result = text.to_owned();
    let low = text.to_lowercase();
    for alias in &config.guard.extra_aliases {
        if low.contains(&alias.canonical.to_lowercase()) || alias.variants.iter().any(|v| low.contains(&v.to_lowercase())) {
            result.push(' '); result.push_str(&alias.canonical);
            for variant in &alias.variants { result.push(' '); result.push_str(variant); }
        }
    }
    result
}

pub fn assess(prompt: &str, b: &Baseline, cfg: &Config, evidence_kind: &str) -> Assessment {
    let expanded_prompt = expanded(prompt, cfg);
    let qt = terms(&expanded_prompt);
    let mut frequency: HashMap<String, usize> = HashMap::new();
    for r in &b.requirements {
        for t in terms(&expanded(&format!("{} {}", r.section, r.content), cfg)) { *frequency.entry(t).or_default() += 1; }
    }
    let mut matches: Vec<(usize, &Requirement)> = b.requirements.iter().filter_map(|r| {
        let rt = terms(&expanded(&format!("{} {}", r.section, r.content), cfg));
        let mut score = 0_usize;
        for shared in qt.intersection(&rt) {
            let rare = frequency.get(shared).copied().unwrap_or(1);
            if shared.len() >= 2 { score += if rare <= 2 { 3 } else { 1 }; }
        }
        if score >= cfg.guard.min_score { Some((score, r)) } else { None }
    }).collect();
    matches.sort_by(|a, b| b.0.cmp(&a.0));
    let mut conflict = false;
    let mut hits = Vec::new();
    for (score, r) in matches.into_iter().take(cfg.guard.top_k.min(10)) {
        let negation = r.constraints.iter().any(|c| c.kind == "must_not" && key_signal(&expanded_prompt, &expanded(&c.text, cfg)));
        // Code evidence is not proof: treat as review only, not a definite semantic contradiction.
        let violation = negation && positive_intent(prompt) && !negative_intent(prompt) && evidence_kind == "user_prompt";
        conflict |= violation;
        hits.push(Hit {
            requirement_id: r.id.clone(), source: r.source.clone(), line: r.line,
            section: r.section.clone(),
            excerpt: r.content.chars().take(200).collect(), score,
            reason: if violation { "New positive directive intersects an explicit MUST_NOT constraint (heuristic; requires confirmation)".into() }
                else if negation { "Potential forbidden-operation overlap; review evidence and scope".into() }
                else { "Relevant requirement candidate, no deterministic contradiction established".into() },
        });
    }
    let (outcome, action, summary, options) = if conflict {
        ("conflict", "block", "New request may contradict an approved MUST_NOT requirement. Human Concept discussion required.",
        vec!["A 保持要件并调整实现（推荐）", "B 讨论修订要件与影响", "C 实验分支隔离", "D 证据不足，进一步调查"])
    } else if hits.is_empty() && positive_intent(prompt) {
        ("new_requirement", "discuss", "No matching approved clause found; requirement coverage is unknown.",
        vec!["A 补充正式要件", "B 指定既有要件", "C 仅探索，不进入正式基线"])
    } else if hits.is_empty() {
        ("uncertain", "allow", "No reliable requirement matches; absence of conflict is not proof of compliance.", vec![])
    } else if hits.iter().any(|h| h.reason.starts_with("Potential")) {
        ("uncertain", "discuss", "Possible semantic divergence; deterministic evidence is insufficient.",
        vec!["A 确认符合要件", "B 调整实现", "C 讨论要件变更", "D 提供更多证据"])
    } else {
        ("compatible", "allow", "Related clauses found, no explicit contradiction detected by local heuristics.", vec![])
    };
    Assessment {
        outcome: outcome.to_string(), action: action.to_string(), summary: summary.to_string(), hits, options: options.iter().map(|s| s.to_string()).collect(),
        evidence_kind: evidence_kind.into(), baseline_digest: b.digest.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProjectConfig, RequirementsConfig, GuardConfig, Requirement, Constraint};
    fn config() -> Config { Config {project:ProjectConfig{name:"p".into()},requirements:RequirementsConfig{paths:vec!["docs/requirements".into()]}, guard: GuardConfig::default()} }
    fn baseline() -> Baseline {
        Baseline{schema_version:1,project:"p".into(),digest:"demo".into(),approved_at_utc:"123".into(),requirements:vec![Requirement{
            id:"REQ-QC-01".into(), source:"docs/requirements/qc.md".into(),section:"QC 导入".into(),line:2,
            content:"MUST_NOT: 中国侧导入时不得重复执行 OCR".into(),digest:"a".into(),
            constraints:vec![Constraint{kind:"must_not".into(),text:"MUST_NOT: 中国侧导入时不得重复执行 OCR".into()}]
        }]}
    }
    #[test] fn explicit_ocr_conflict() {
        let a=assess("中国侧导入时新增 OCR 识别",&baseline(),&config(),"user_prompt");
        assert_eq!(a.outcome,"conflict"); assert_eq!(a.action,"block");
    }
    #[test] fn removal_is_not_conflict() {
        let a=assess("中国侧导入时移除 OCR",&baseline(),&config(),"user_prompt");
        assert_ne!(a.action,"block");
    }
    #[test] fn code_line_is_review_not_proof() {
        let a=assess("add ocr.process()",&baseline(),&config(),"code_diff");
        assert_ne!(a.action,"block");
    }
}
