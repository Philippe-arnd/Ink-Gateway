use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;
use std::path::Path;
use tracing::info;

use crate::config::Config;
use crate::git;

// ─── Output types ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ClosePayload {
    pub session_word_count: u32,
    pub total_word_count: u32,
    pub target_length: u32,
    pub completion_ready: bool,
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct CompletePayload {
    pub status: &'static str,
    pub total_word_count: u32,
}

// ─── session-close ─────────────────────────────────────────────────────────────

pub fn close_session(
    repo: &Path,
    prose: &str,
    summary: Option<&str>,
    human_edits: &[String],
) -> Result<ClosePayload> {
    let lock_path = repo.join(".ink-running");

    // Guard: lock must exist
    if !lock_path.exists() {
        let error = serde_json::json!({"error": "no active session", "status": "error"});
        println!("{}", serde_json::to_string_pretty(&error).unwrap());
        std::process::exit(1);
    }

    let config = Config::load(repo)?;
    let now = Local::now();
    let session_word_count = prose.split_whitespace().count() as u32;

    // 1. Overwrite Review/current.md
    info!("Writing Review/current.md");
    let review_dir = repo.join("Review");
    std::fs::create_dir_all(&review_dir)
        .with_context(|| "Failed to create Review/")?;
    std::fs::write(review_dir.join("current.md"), prose)
        .with_context(|| "Failed to write Review/current.md")?;

    // 2. Append delta paragraph to Summary.md
    info!("Appending to Summary.md");
    let summary_path = repo.join("Global Material").join("Summary.md");
    let delta_text = summary
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!(
                "Session {} — {} words written.",
                now.format("%Y-%m-%d %H:%M"),
                session_word_count
            )
        });
    let delta = format!("\n\n{}", delta_text.trim());
    let mut existing_summary = if summary_path.exists() {
        std::fs::read_to_string(&summary_path)
            .with_context(|| "Failed to read Summary.md")?
    } else {
        String::new()
    };
    existing_summary.push_str(&delta);
    std::fs::write(&summary_path, &existing_summary)
        .with_context(|| "Failed to write Summary.md")?;

    // 3. Write Changelog/YYYY-MM-DD-HH-MM.md
    info!("Writing changelog entry");
    let changelog_dir = repo.join("Changelog");
    std::fs::create_dir_all(&changelog_dir)
        .with_context(|| "Failed to create Changelog/")?;
    let changelog_filename = format!("{}.md", now.format("%Y-%m-%d-%H-%M"));
    let changelog_path = changelog_dir.join(&changelog_filename);

    let mut changelog = format!(
        "# Session {}\n\n**Words written:** {}\n",
        now.format("%Y-%m-%d %H:%M"),
        session_word_count
    );
    if !human_edits.is_empty() {
        changelog.push_str("\n**Human edits:**\n");
        for edit in human_edits {
            changelog.push_str(&format!("- {}\n", edit));
        }
    }
    if let Some(s) = summary {
        changelog.push_str(&format!("\n**Summary:**\n{}\n", s.trim()));
    }

    std::fs::write(&changelog_path, &changelog)
        .with_context(|| format!("Failed to write {}", changelog_path.display()))?;

    // 4. Append prose to Current version/Full_Book.md
    info!("Appending to Full_Book.md");
    let book_dir = repo.join("Current version");
    std::fs::create_dir_all(&book_dir)
        .with_context(|| "Failed to create 'Current version/'")?;
    let book_path = book_dir.join("Full_Book.md");

    let mut book_content = if book_path.exists() {
        std::fs::read_to_string(&book_path)
            .with_context(|| "Failed to read Full_Book.md")?
    } else {
        String::new()
    };
    if !book_content.is_empty() && !book_content.ends_with('\n') {
        book_content.push('\n');
    }
    book_content.push('\n');
    book_content.push_str(prose.trim_start());
    std::fs::write(&book_path, &book_content)
        .with_context(|| "Failed to write Full_Book.md")?;

    let total_word_count = book_content.split_whitespace().count() as u32;

    // 5. Commit everything on draft (including lock removal) and push main + draft
    info!("Committing session on draft branch");
    git::run_git(repo, &["rm", "-f", ".ink-running"])
        .with_context(|| "Failed to git rm .ink-running")?;
    git::run_git(repo, &["add", "-A"])
        .with_context(|| "Failed to git add session files")?;
    git::run_git(repo, &["commit", "-m", "session: write prose"])
        .with_context(|| "Failed to commit session files")?;
    git::run_git(repo, &["push", "origin", "draft"])
        .with_context(|| "Failed to push draft")?;

    info!("Fast-forward merging draft into main and pushing");
    git::run_git(repo, &["checkout", "main"])
        .with_context(|| "Failed to checkout main")?;
    git::run_git(repo, &["merge", "--ff-only", "draft"])
        .with_context(|| "Failed to fast-forward merge draft into main")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push main")?;

    let completion_ready = total_word_count >= (config.target_length as f64 * 0.9) as u32;

    Ok(ClosePayload {
        session_word_count,
        total_word_count,
        target_length: config.target_length,
        completion_ready,
        status: "closed",
    })
}

// ─── complete ─────────────────────────────────────────────────────────────────

pub fn complete_session(repo: &Path) -> Result<CompletePayload> {
    let complete_path = repo.join("COMPLETE");

    // Guard: COMPLETE must not already exist
    if complete_path.exists() {
        let error = serde_json::json!({"error": "book already complete", "status": "error"});
        println!("{}", serde_json::to_string_pretty(&error).unwrap());
        std::process::exit(1);
    }

    // Ensure we're on main
    git::run_git(repo, &["checkout", "main"])
        .with_context(|| "Failed to checkout main for complete")?;

    // Write COMPLETE marker
    info!("Writing COMPLETE marker");
    std::fs::write(&complete_path, "")
        .with_context(|| "Failed to write COMPLETE")?;

    // Remove stale .ink-running if still present
    let lock_path = repo.join(".ink-running");
    if lock_path.exists() {
        git::run_git(repo, &["rm", "-f", ".ink-running"])
            .with_context(|| "Failed to git rm .ink-running")?;
    }

    // Count total words
    let book_path = repo.join("Current version").join("Full_Book.md");
    let total_word_count = if book_path.exists() {
        let content = std::fs::read_to_string(&book_path)
            .with_context(|| "Failed to read Full_Book.md for word count")?;
        content.split_whitespace().count() as u32
    } else {
        0
    };

    // Commit and push
    git::run_git(repo, &["add", "-A"])
        .with_context(|| "Failed to git add COMPLETE")?;
    git::run_git(repo, &["commit", "-m", "book: complete"])
        .with_context(|| "Failed to commit completion")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push completion")?;

    Ok(CompletePayload {
        status: "complete",
        total_word_count,
    })
}
