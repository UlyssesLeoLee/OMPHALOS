use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub source: String,
    pub section: String,
    pub line: usize,
    pub content: String,
    pub digest: String,
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub kind: String,  // must | must_not | ordinary
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub schema_version: u32,
    pub project: String,
    pub digest: String,
    pub approved_at_utc: String,
    pub requirements: Vec<Requirement>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    pub requirements: RequirementsConfig,
    #[serde(default)]
    pub guard: GuardConfig,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct RequirementsConfig {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuardConfig {
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_min_score")]
    pub min_score: usize,
    #[serde(default)]
    pub extra_aliases: Vec<Alias>,
}

impl Default for GuardConfig {
    fn default() -> Self { Self { top_k: default_top_k(), min_score: default_min_score(), extra_aliases: Vec::new() } }
}
fn default_top_k() -> usize { 3 }
fn default_min_score() -> usize { 2 }

#[derive(Debug, Deserialize)]
pub struct Alias {
    pub canonical: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub requirement_id: String,
    pub source: String,
    pub line: usize,
    pub section: String,
    pub excerpt: String,
    pub score: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assessment {
    pub outcome: String, // compatible | conflict | uncertain | new_requirement
    pub action: String, // allow | discuss | block
    pub summary: String,
    pub hits: Vec<Hit>,
    pub options: Vec<String>,
    pub evidence_kind: String,
    pub baseline_digest: String,
}

pub fn meta(root: &std::path::Path, name: &str) -> PathBuf { root.join(".concept-guard").join(name) }
