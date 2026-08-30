use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::Path;

use crate::comments::Comment;
use crate::config::Config;
use crate::state::InkState;

// ─── Output types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FileContent {
    pub filename: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChapterInfo {
    pub path: String,
    pub content: String,
    pub modified_today: bool,
}

#[derive(Debug, Serialize)]
pub struct WordCount {
    pub total: u32,
    pub target: u32,
    pub remaining: u32,
}

#[derive(Debug, Serialize)]
pub struct ConfigSnapshot {
    pub target_length: u32,
    pub chapter_count: u32,
    pub chapter_structure: String,
    pub words_per_session: u32,
    pub summary_context_entries: usize,
    pub words_per_chapter: u32,
    pub current_chapter: u32,
}

impl ConfigSnapshot {
    fn new(config: &Config, current_chapter: u32) -> Self {
        ConfigSnapshot {
            target_length: config.target_length,
            chapter_count: config.chapter_count,
            chapter_structure: config.chapter_structure.clone(),
            words_per_session: config.words_per_session,
            summary_context_entries: config.summary_context_entries,
            words_per_chapter: config.words_per_chapter,
            current_chapter,
        }
    }
}

// ─── Lock file helpers ────────────────────────────────────────────────────────
//
// The lock itself (`.ink-running`) was only ever written by the now-removed
// scheduled-session flow (`session-open`/`session-close`). `read_lock_age`
// survives here because `status`/`doctor` (maintenance.rs) still report it —
// a leftover lock from before this flow was removed, or one written by an
// external tool, is still worth surfacing as a diagnostic.

fn lock_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".ink-running")
}

/// Returns age of the lock file in minutes, or None if no lock exists.
pub fn read_lock_age(repo: &Path) -> Option<i64> {
    let path = lock_path(repo);
    let content = std::fs::read_to_string(&path).ok()?;
    let timestamp: DateTime<Utc> = content.trim().parse().ok()?;
    let age = Utc::now().signed_duration_since(timestamp).num_minutes();
    Some(age)
}

// ─── Loading helpers ──────────────────────────────────────────────────────────

pub fn load_global_material(repo: &Path, summary_entries: usize) -> Result<Vec<FileContent>> {
    let global_dir = repo.join("Global Material");
    let mut files: Vec<FileContent> = std::fs::read_dir(&global_dir)
        .with_context(|| {
            format!(
                "Failed to read Global Material/ at {}",
                global_dir.display()
            )
        })?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| -> Result<Option<FileContent>> {
            let path = e.path();
            let filename = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => return Ok(None),
            };
            // Skip Config.yml — it's surfaced separately
            if filename == "Config.yml" {
                return Ok(None);
            }
            let mut content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read Global Material/{}", filename))?;
            if filename == "Summary.md" {
                content = truncate_summary(&content, summary_entries);
            }
            Ok(Some(FileContent { filename, content }))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(files)
}

/// The minimum word count for a Summary.md paragraph to be considered substantive.
/// One-liner auto-generated entries ("Session X — N words written.") are filtered out
/// so that `summary_context_entries` selects meaningful narrative paragraphs.
const MIN_SUMMARY_PARAGRAPH_WORDS: usize = 15;

pub fn truncate_summary(text: &str, n: usize) -> String {
    let all: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    // Prefer substantive paragraphs; fall back to all if none qualify
    let substantive: Vec<&str> = all
        .iter()
        .filter(|p| p.split_whitespace().count() >= MIN_SUMMARY_PARAGRAPH_WORDS)
        .copied()
        .collect();

    let pool = if substantive.is_empty() {
        &all
    } else {
        &substantive
    };
    let start = pool.len().saturating_sub(n);
    pool[start..].join("\n\n")
}

pub fn load_chapter(repo: &Path, num: u32, human_edits: &[String]) -> Result<Option<ChapterInfo>> {
    let relative = format!("Chapters material/Chapter_{:02}.md", num);
    let path = repo.join(&relative);

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read chapter {}", num))?;

    let modified_today = human_edits
        .iter()
        .any(|f| f.contains(&format!("Chapter_{:02}.md", num)));

    Ok(Some(ChapterInfo {
        path: relative,
        content,
        modified_today,
    }))
}

pub fn load_word_count(repo: &Path, target: u32) -> Result<WordCount> {
    let path = repo.join("Current version").join("Full_Book.md");

    if !path.exists() {
        return Ok(WordCount {
            total: 0,
            target,
            remaining: target,
        });
    }

    let content = std::fs::read_to_string(&path).with_context(|| "Failed to read Full_Book.md")?;
    let total = crate::book::count_prose_words(&content);
    let remaining = target.saturating_sub(total);

    Ok(WordCount {
        total,
        target,
        remaining,
    })
}

// ─── Live editing snapshot (web editor / chat) ─────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BookContext {
    pub config: ConfigSnapshot,
    pub global_material: Vec<FileContent>,
    pub current_chapter: Option<ChapterInfo>,
    pub current: String,
    pub comments: Vec<Comment>,
    pub word_count: WordCount,
}

/// Read-only snapshot of everything a browser editor or chat turn needs:
/// Global Material, the active chapter outline, the live `Review/current.md`
/// draft, open comment threads, and word counts. Performs **no git
/// mutation** — no lock, no snapshot tag, no push — safe to call on every
/// chat turn or editor load.
pub fn get_book_context(repo: &Path) -> Result<BookContext> {
    let config = Config::load(repo)?;
    let state = InkState::load(repo)?;

    let global_material = load_global_material(repo, config.summary_context_entries)?;
    let current_chapter = load_chapter(repo, state.current_chapter, &[])?;

    let review_path = repo.join("Review").join("current.md");
    let current = if review_path.exists() {
        std::fs::read_to_string(&review_path).with_context(|| "Failed to read Review/current.md")?
    } else {
        String::new()
    };

    let comments = crate::comments::list_comments(repo)?;
    let word_count = load_word_count(repo, config.target_length)?;

    Ok(BookContext {
        config: ConfigSnapshot::new(&config, state.current_chapter),
        global_material,
        current_chapter,
        current,
        comments,
        word_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_config(dir: &std::path::Path) {
        let global_dir = dir.join("Global Material");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join("Config.yml"),
            "target_length: 80000\nchapter_count: 10\nchapter_structure: three-act\n\
             words_per_session: 800\n",
        )
        .unwrap();
        std::fs::write(global_dir.join("Soul.md"), "Wry, dry narrator.").unwrap();
    }

    #[test]
    fn get_book_context_reads_full_snapshot_without_git_mutation() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path());

        let chapters_dir = tmp.path().join("Chapters material");
        std::fs::create_dir_all(&chapters_dir).unwrap();
        std::fs::write(
            chapters_dir.join("Chapter_01.md"),
            "Outline for chapter one.",
        )
        .unwrap();

        let review_dir = tmp.path().join("Review");
        std::fs::create_dir_all(&review_dir).unwrap();
        std::fs::write(review_dir.join("current.md"), "Draft prose in progress.").unwrap();

        let ctx = get_book_context(tmp.path()).unwrap();

        assert_eq!(ctx.current, "Draft prose in progress.");
        assert!(ctx.global_material.iter().any(|f| f.filename == "Soul.md"));
        assert!(ctx.current_chapter.is_some());
        assert!(ctx.comments.is_empty());
        assert_eq!(ctx.word_count.target, 80000);

        // Purely read-only: no session lock created as a side effect.
        assert!(!tmp.path().join(".ink-running").exists());
    }

    #[test]
    fn get_book_context_missing_current_md_is_empty_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path());

        let ctx = get_book_context(tmp.path()).unwrap();
        assert_eq!(ctx.current, "");
        assert!(ctx.current_chapter.is_none());
    }
}
