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

fn default_words_per_page() -> u32 {
    250
}

fn default_words_per_chapter() -> u32 {
    3000
}

// current_review_window_words: 0 means no limit
fn default_current_review_window_words() -> u32 {
    0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    #[serde(default = "default_language")]
    #[allow(dead_code)] // read by the ink-engine agent via JSON, not by Rust code
    pub language: String,
    pub target_length: u32,
    pub chapter_count: u32,
    pub chapter_structure: String,
    pub words_per_session: u32,
    #[serde(default = "default_summary_context_entries")]
    pub summary_context_entries: usize,
    #[serde(default = "default_session_timeout_minutes")]
    pub session_timeout_minutes: i64,
    #[serde(default = "default_words_per_page")]
    pub words_per_page: u32,
    #[serde(default = "default_words_per_chapter")]
    pub words_per_chapter: u32,
    #[serde(default = "default_current_review_window_words")]
    pub current_review_window_words: u32,
}

impl Config {
    pub fn load(repo_path: &Path) -> Result<Self> {
        let config_path = repo_path.join("Global Material").join("Config.yml");
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read Config.yml at {}", config_path.display()))?;
        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse Config.yml")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.target_length > 0,
            "Config.yml: target_length must be > 0, got {}", self.target_length);
        anyhow::ensure!(self.chapter_count >= 1,
            "Config.yml: chapter_count must be >= 1, got {}", self.chapter_count);
        anyhow::ensure!(self.words_per_session > 0,
            "Config.yml: words_per_session must be > 0, got {}", self.words_per_session);
        anyhow::ensure!(self.words_per_chapter > 0,
            "Config.yml: words_per_chapter must be > 0, got {}", self.words_per_chapter);
        anyhow::ensure!(self.words_per_page > 0,
            "Config.yml: words_per_page must be > 0, got {}", self.words_per_page);
        anyhow::ensure!(self.session_timeout_minutes > 0,
            "Config.yml: session_timeout_minutes must be > 0, got {}", self.session_timeout_minutes);
        Ok(())
    }
}
