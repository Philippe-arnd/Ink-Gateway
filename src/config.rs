use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

fn default_language() -> String {
    "English".to_string()
}

fn default_summary_context_entries() -> usize {
    5
}

fn default_session_timeout_minutes() -> i64 {
    60
}

fn default_current_chapter() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    #[serde(default = "default_language")]
    pub language: String,
    pub target_length: u32,
    pub chapter_count: u32,
    pub chapter_structure: String,
    pub words_per_session: u32,
    #[serde(default = "default_summary_context_entries")]
    pub summary_context_entries: usize,
    #[serde(default = "default_current_chapter")]
    pub current_chapter: u32,
    #[serde(default = "default_session_timeout_minutes")]
    pub session_timeout_minutes: i64,
}

impl Config {
    pub fn load(repo_path: &Path) -> Result<Self> {
        let config_path = repo_path.join("Global Material").join("Config.yml");
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read Config.yml at {}", config_path.display()))?;
        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse Config.yml")?;
        Ok(config)
    }
}
