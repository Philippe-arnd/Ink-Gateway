//! Fine-grained live edits to `Review/current.md`, for a browser editor or
//! chat agent changing a few words at a time (as opposed to the coarse,
//! whole-file rewrites the old scheduled-session flow used to do). Each
//! function commits locally — no push — callers batch pushes, same pattern
//! as `maintenance::advance_chapter`.
//!
//! Positions are character offsets, not byte offsets, so multi-byte UTF-8
//! text never causes a boundary panic.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::book::count_prose_words;
use crate::git;

fn current_path(repo: &Path) -> PathBuf {
    repo.join("Review").join("current.md")
}

fn read_current(repo: &Path) -> Result<String> {
    let path = current_path(repo);
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))
}

fn write_current(repo: &Path, content: &str, message: &str) -> Result<Value> {
    let path = current_path(repo);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    git::run_git(repo, &["add", "Review/current.md"])
        .with_context(|| "Failed to git add Review/current.md")?;
    git::run_git(repo, &["commit", "-m", message]).with_context(|| "Failed to commit live edit")?;

    Ok(json!({
        "word_count": count_prose_words(content),
        "char_count": content.chars().count(),
    }))
}

/// Insert `content` at character offset `position` in `Review/current.md`.
pub fn insert_text(repo: &Path, position: usize, content: &str) -> Result<Value> {
    let current = read_current(repo)?;
    let mut chars: Vec<char> = current.chars().collect();
    anyhow::ensure!(
        position <= chars.len(),
        "position {} out of bounds (document has {} characters)",
        position,
        chars.len()
    );
    chars.splice(position..position, content.chars());
    let new_content: String = chars.into_iter().collect();
    write_current(repo, &new_content, "edit: insert text")
}

/// Replace the character range `[start, end)` in `Review/current.md` with `content`.
pub fn rewrite_range(repo: &Path, start: usize, end: usize, content: &str) -> Result<Value> {
    anyhow::ensure!(start <= end, "start ({}) must be <= end ({})", start, end);
    let current = read_current(repo)?;
    let mut chars: Vec<char> = current.chars().collect();
    anyhow::ensure!(
        end <= chars.len(),
        "end {} out of bounds (document has {} characters)",
        end,
        chars.len()
    );
    chars.splice(start..end, content.chars());
    let new_content: String = chars.into_iter().collect();
    write_current(repo, &new_content, "edit: rewrite range")
}

/// The only paths `write_foundation_file` may touch — not client-controlled,
/// so a tool call can never be pointed at an arbitrary file in the repo.
pub const FOUNDATION_PATHS: &[&str] = &[
    "Global Material/Soul.md",
    "Global Material/Characters.md",
    "Global Material/Outline.md",
    "Global Material/Lore.md",
    "Chapters material/Chapter_01.md",
];

/// Overwrite one allow-listed foundational file with `content`, committing
/// locally (no push) — same write→add→commit shape as `write_current`,
/// generalized to a path from `FOUNDATION_PATHS` instead of the hardcoded
/// `Review/current.md`.
pub fn write_foundation_file(repo: &Path, rel_path: &str, content: &str, message: &str) -> Result<Value> {
    anyhow::ensure!(
        FOUNDATION_PATHS.contains(&rel_path),
        "{} is not an allow-listed foundational file",
        rel_path
    );

    let path = repo.join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;

    git::run_git(repo, &["add", rel_path]).with_context(|| format!("Failed to git add {rel_path}"))?;
    git::run_git(repo, &["commit", "-m", message]).with_context(|| "Failed to commit foundation write")?;

    Ok(json!({
        "word_count": count_prose_words(content),
        "char_count": content.chars().count(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(tmp: &std::path::Path) {
        git::run_git(tmp, &["init", "-q"]).unwrap();
        git::run_git(tmp, &["config", "user.email", "test@example.com"]).unwrap();
        git::run_git(tmp, &["config", "user.name", "Test"]).unwrap();
    }

    #[test]
    fn insert_text_into_empty_document() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());

        insert_text(tmp.path(), 0, "Hello world").unwrap();
        assert_eq!(read_current(tmp.path()).unwrap(), "Hello world");
    }

    #[test]
    fn insert_text_at_position_splits_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        write_current(tmp.path(), "Hello world", "seed").unwrap();

        insert_text(tmp.path(), 5, " brave").unwrap();
        assert_eq!(read_current(tmp.path()).unwrap(), "Hello brave world");
    }

    #[test]
    fn insert_text_out_of_bounds_errors() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        write_current(tmp.path(), "Hi", "seed").unwrap();

        let err = insert_text(tmp.path(), 99, "x").unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn rewrite_range_replaces_substring() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        write_current(tmp.path(), "The cat sat", "seed").unwrap();

        rewrite_range(tmp.path(), 4, 7, "dog").unwrap();
        assert_eq!(read_current(tmp.path()).unwrap(), "The dog sat");
    }

    #[test]
    fn rewrite_range_respects_multibyte_chars() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        write_current(tmp.path(), "café noir", "seed").unwrap();

        // "é" is one char at index 3 — replacing it must not byte-boundary panic.
        rewrite_range(tmp.path(), 3, 4, "e").unwrap();
        assert_eq!(read_current(tmp.path()).unwrap(), "cafe noir");
    }

    #[test]
    fn rewrite_range_inverted_bounds_errors() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        write_current(tmp.path(), "hi", "seed").unwrap();

        let err = rewrite_range(tmp.path(), 5, 1, "x").unwrap_err();
        assert!(err.to_string().contains("must be <="));
    }

    #[test]
    fn commits_are_local_only_no_push_attempted() {
        // No remote is configured in this test repo — if write_current tried to
        // push, this would fail. Asserting success proves the "no push" contract.
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        assert!(insert_text(tmp.path(), 0, "text").is_ok());
    }

    #[test]
    fn write_foundation_file_rejects_non_allowlisted_path() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());

        let err = write_foundation_file(tmp.path(), "Review/current.md", "x", "msg").unwrap_err();
        assert!(err.to_string().contains("not an allow-listed"));
    }

    #[test]
    fn write_foundation_file_writes_and_commits() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());

        let result = write_foundation_file(
            tmp.path(),
            "Global Material/Soul.md",
            "A richer soul.",
            "expand: Soul.md",
        )
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("Global Material/Soul.md")).unwrap();
        assert_eq!(content, "A richer soul.");
        assert_eq!(result["word_count"], 3);
        assert_eq!(result["char_count"], 14);

        let log = git::run_git(tmp.path(), &["log", "-1", "--format=%s"]).unwrap();
        assert_eq!(log, "expand: Soul.md");
    }
}
