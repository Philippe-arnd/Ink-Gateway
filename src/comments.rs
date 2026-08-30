//! Highlight-and-comment threads anchored to `Review/current.md`.
//!
//! Generalises the CLI's single `<!-- INK: [instruction] -->` marker into a
//! real multi-thread model for the web editor: select a range, attach a
//! comment (human or AI), resolve it later. Stored as a git-tracked YAML file
//! so threads get history, diffing, and versioning for free — no database.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::git;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub anchor_start: usize,
    pub anchor_end: usize,
    pub author: String,
    pub text: String,
    pub resolved: bool,
    pub created_at: String,
}

fn comments_path(repo: &Path) -> PathBuf {
    repo.join("Comments").join("current.yml")
}

fn load_all(repo: &Path) -> Result<Vec<Comment>> {
    let path = comments_path(repo);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_yaml::from_str(&content).with_context(|| "Failed to parse Comments/current.yml")
}

fn save_all(repo: &Path, comments: &[Comment]) -> Result<()> {
    let path = comments_path(repo);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let content =
        serde_yaml::to_string(comments).with_context(|| "Failed to serialize comments")?;
    std::fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))
}

fn commit(repo: &Path, message: &str) -> Result<()> {
    git::run_git(repo, &["add", "Comments/current.yml"])
        .with_context(|| "Failed to git add Comments/current.yml")?;
    git::run_git(repo, &["commit", "-m", message])
        .with_context(|| "Failed to commit comment change")?;
    Ok(())
}

/// List all comment threads (open and resolved) anchored to `Review/current.md`.
pub fn list_comments(repo: &Path) -> Result<Vec<Comment>> {
    load_all(repo)
}

/// Attach a comment/instruction to a character range of `Review/current.md`.
/// `author` is `"human"` or `"ai"`. Commits locally (no push — callers batch
/// pushes, same pattern as `maintenance::advance_chapter`).
pub fn add_comment(
    repo: &Path,
    anchor_start: usize,
    anchor_end: usize,
    author: &str,
    text: &str,
) -> Result<Comment> {
    anyhow::ensure!(
        anchor_start <= anchor_end,
        "anchor_start ({}) must be <= anchor_end ({})",
        anchor_start,
        anchor_end
    );

    let mut comments = load_all(repo)?;
    let comment = Comment {
        id: format!("c-{}", Utc::now().format("%Y%m%d%H%M%S%6f")),
        anchor_start,
        anchor_end,
        author: author.to_string(),
        text: text.to_string(),
        resolved: false,
        created_at: Utc::now().to_rfc3339(),
    };
    comments.push(comment.clone());
    save_all(repo, &comments)?;
    commit(repo, &format!("comment: add ({})", author))?;
    Ok(comment)
}

/// Mark a comment thread resolved. Commits locally (no push).
pub fn resolve_comment(repo: &Path, id: &str) -> Result<Comment> {
    let mut comments = load_all(repo)?;
    let comment = comments
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow::anyhow!("comment not found: {}", id))?;
    comment.resolved = true;
    let resolved = comment.clone();
    save_all(repo, &comments)?;
    commit(repo, &format!("comment: resolve {}", id))?;
    Ok(resolved)
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
    fn list_comments_empty_when_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_comments(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn add_comment_persists_and_commits() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());

        let comment = add_comment(tmp.path(), 10, 20, "human", "make this darker").unwrap();
        assert!(!comment.resolved);

        let all = list_comments(tmp.path()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "make this darker");

        let log = git::run_git(tmp.path(), &["log", "--oneline"]).unwrap();
        assert!(log.contains("comment: add"));
    }

    #[test]
    fn add_comment_rejects_inverted_range() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        let err = add_comment(tmp.path(), 20, 10, "human", "x").unwrap_err();
        assert!(err.to_string().contains("must be <="));
    }

    #[test]
    fn resolve_comment_marks_resolved() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        let comment = add_comment(tmp.path(), 0, 5, "ai", "note").unwrap();

        let resolved = resolve_comment(tmp.path(), &comment.id).unwrap();
        assert!(resolved.resolved);

        let all = list_comments(tmp.path()).unwrap();
        assert!(all.iter().find(|c| c.id == comment.id).unwrap().resolved);
    }

    #[test]
    fn resolve_comment_missing_id_errors() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        let err = resolve_comment(tmp.path(), "does-not-exist").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
