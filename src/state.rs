use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_current_chapter() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InkState {
    #[serde(default = "default_current_chapter")]
    pub current_chapter: u32,
    #[serde(default)]
    pub current_chapter_word_count: u32,
}

impl Default for InkState {
    fn default() -> Self {
        InkState {
            current_chapter: 1,
            current_chapter_word_count: 0,
        }
    }
}

impl InkState {
    /// Load `.ink-state.yml` from the repo root. Returns defaults if the file
    /// does not exist (first-run or migrated repos).
    pub fn load(repo_path: &Path) -> Result<Self> {
        let path = repo_path.join(".ink-state.yml");
        if !path.exists() {
            return Ok(InkState::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read .ink-state.yml at {}", path.display()))?;
        let state: InkState = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse .ink-state.yml")?;
        anyhow::ensure!(state.current_chapter >= 1,
            ".ink-state.yml: current_chapter must be >= 1, got {}", state.current_chapter);
        Ok(state)
    }

    /// Write the current state to `.ink-state.yml` atomically (write-then-rename).
    /// Prevents a corrupted state file if the process crashes mid-write.
    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let path = repo_path.join(".ink-state.yml");
        let tmp_path = repo_path.join(".ink-state.yml.tmp");
        let content = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize .ink-state.yml")?;
        std::fs::write(&tmp_path, content)
            .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, &path)
            .with_context(|| "Failed to atomically replace .ink-state.yml")?;
        Ok(())
    }
}
