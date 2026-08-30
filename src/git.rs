use anyhow::{bail, Context, Result};
use chrono::Local;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

pub fn run_git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("Failed to spawn git with args: {:?}", args))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("git {:?} failed: {}", args, stderr)
    }
}

/// Fetch remote state and switch to main. Does NOT merge — call
/// `merge_ff_origin_main` separately after human edits are committed.
pub fn preflight_fetch_and_checkout(repo: &Path) -> Result<()> {
    info!("Fetching origin...");
    run_git(repo, &["fetch", "origin"]).with_context(|| "Failed to fetch from origin")?;

    info!("Checking out main...");
    run_git(repo, &["checkout", "main"]).with_context(|| "Failed to checkout main")?;

    Ok(())
}

/// Fast-forward local main onto origin/main. Call this AFTER human edits
/// are committed so the merge cannot overwrite uncommitted local changes.
pub fn merge_ff_origin_main(repo: &Path) -> Result<()> {
    info!("Fast-forward merging origin/main...");
    run_git(repo, &["merge", "--ff-only", "origin/main"])
        .with_context(|| "Failed to merge origin/main (non-fast-forward?)")?;
    Ok(())
}

/// Returns files that differ between the local working tree and origin/main.
/// This catches IDE saves that were never committed/pushed — the diff between
/// what the user has locally and what the remote last committed.
pub fn collect_diffs_vs_remote(repo: &Path) -> Result<Vec<String>> {
    match run_git(repo, &["diff", "origin/main", "--name-only"]) {
        Ok(output) => Ok(output
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()),
        Err(_) => Ok(vec![]), // origin/main may not exist on a fresh local repo
    }
}

pub fn collect_modified_files(repo: &Path) -> Result<Vec<String>> {
    let output = run_git(repo, &["status", "--short"])?;
    let files: Vec<String> = output
        .lines()
        .filter_map(|line| {
            // git status --short format: "XY filename" (2-char status + space + path).
            // Use .get() to avoid panic on multi-byte UTF-8 characters.
            let raw = line.get(3..)?.trim().to_string();
            if raw.is_empty() {
                return None;
            }
            // For renames/copies ("R old -> new"), extract the destination path.
            if let Some(arrow_pos) = raw.find(" -> ") {
                let dest = raw[arrow_pos + 4..].trim().to_string();
                if !dest.is_empty() {
                    return Some(dest);
                }
            }
            Some(raw)
        })
        .collect();
    Ok(files)
}

pub fn commit_human_edits(repo: &Path, files: &[String]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    info!("Committing {} human-edited file(s)...", files.len());

    run_git(repo, &["add", "."]).with_context(|| "Failed to git add")?;

    // `git diff --cached --quiet` exits 0 when nothing is staged, 1 when
    // there are staged changes. The human_edits list may contain files from
    // collect_diffs_vs_remote that reflect remote-ahead commits rather than
    // actual local edits — in that case the working tree is clean and there
    // is nothing to commit.
    let nothing_staged = run_git(repo, &["diff", "--cached", "--quiet"]).is_ok();
    if nothing_staged {
        info!("Nothing staged after git add — skipping commit (working tree already clean)");
        return Ok(());
    }

    run_git(repo, &["commit", "-m", "chore: human updates"])
        .with_context(|| "Failed to commit human edits")?;

    // No push here — push_tags (called later in session_open) carries this
    // commit to origin together with the snapshot tag in a single push.
    Ok(())
}

pub fn create_snapshot_tag(repo: &Path) -> Result<String> {
    let tag = format!("ink-{}", Local::now().format("%Y-%m-%d-%H-%M-%S"));

    match run_git(repo, &["tag", &tag]) {
        Ok(_) => {
            info!("Created snapshot tag: {}", tag);
        }
        Err(e) => {
            // Tag may already exist (idempotent retry) — warn but don't fail
            warn!("Could not create tag {} (may already exist): {}", tag, e);
        }
    }

    Ok(tag)
}

pub fn push_tags(repo: &Path) -> Result<()> {
    run_git(repo, &["push", "origin", "main", "--tags"])
        .with_context(|| "Failed to push main with tags")?;
    Ok(())
}

pub fn setup_draft_branch(repo: &Path) -> Result<()> {
    // Create or force-reset draft to match main — atomic, never conflicts.
    // This matches the pattern used in complete_session (git branch -f draft main).
    info!("Setting up draft branch (force-reset to main)...");
    run_git(repo, &["checkout", "-B", "draft", "main"])
        .with_context(|| "Failed to create/reset draft branch")?;
    Ok(())
}

// ─── Versioning (web editor: version history + restore) ───────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct VersionEntry {
    pub commit: String,
    pub date: String,
    pub message: String,
}

/// List the commit history touching `rel_path`, most recent first. Backs the
/// web editor's version history panel — every commit that touched the file is
/// a restore point, no separate versions table needed.
pub fn list_versions(repo: &Path, rel_path: &str) -> Result<Vec<VersionEntry>> {
    let output = run_git(
        repo,
        &[
            "log",
            "--follow",
            "--format=%H%x1f%aI%x1f%s",
            "--",
            rel_path,
        ],
    )
    .with_context(|| format!("Failed to list git history for {}", rel_path))?;

    let entries = output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\u{1f}');
            let commit = parts.next()?.to_string();
            let date = parts.next()?.to_string();
            let message = parts.next().unwrap_or_default().to_string();
            Some(VersionEntry {
                commit,
                date,
                message,
            })
        })
        .collect();

    Ok(entries)
}

/// Restore `rel_path` to its content at `commit`, writing it forward as a
/// **new** commit — history is never rewritten. Matches the framework's
/// existing forward-only versioning philosophy (`Full_Book.md` is append-only;
/// `rollback` is the one intentional exception, reserved for undoing a bad
/// autonomous session, not for everyday editor use).
pub fn restore_version(repo: &Path, rel_path: &str, commit: &str) -> Result<()> {
    let content = run_git(repo, &["show", &format!("{}:{}", commit, rel_path)])
        .with_context(|| format!("Failed to read {} at {}", rel_path, commit))?;

    let path = repo.join(rel_path);
    std::fs::write(&path, format!("{}\n", content))
        .with_context(|| format!("Failed to write {}", path.display()))?;

    run_git(repo, &["add", rel_path]).with_context(|| "Failed to git add restored file")?;
    run_git(
        repo,
        &[
            "commit",
            "-m",
            &format!("restore: {} to {}", rel_path, commit),
        ],
    )
    .with_context(|| "Failed to commit restore")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(tmp: &std::path::Path) {
        run_git(tmp, &["init", "-q"]).unwrap();
        run_git(tmp, &["config", "user.email", "test@example.com"]).unwrap();
        run_git(tmp, &["config", "user.name", "Test"]).unwrap();
    }

    fn commit_file(tmp: &std::path::Path, rel_path: &str, content: &str, message: &str) {
        let path = tmp.join(rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        run_git(tmp, &["add", rel_path]).unwrap();
        run_git(tmp, &["commit", "-m", message]).unwrap();
    }

    #[test]
    fn list_versions_returns_history_most_recent_first() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        commit_file(tmp.path(), "current.md", "v1", "first draft");
        commit_file(tmp.path(), "current.md", "v2", "second draft");

        let versions = list_versions(tmp.path(), "current.md").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].message, "second draft");
        assert_eq!(versions[1].message, "first draft");
    }

    #[test]
    fn list_versions_empty_for_untracked_path() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        commit_file(tmp.path(), "other.md", "x", "unrelated");

        let versions = list_versions(tmp.path(), "current.md").unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn restore_version_writes_forward_commit_without_rewriting_history() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        commit_file(tmp.path(), "current.md", "v1 content", "first draft");
        let first_commit = list_versions(tmp.path(), "current.md").unwrap()[0]
            .commit
            .clone();
        commit_file(tmp.path(), "current.md", "v2 content", "second draft");

        restore_version(tmp.path(), "current.md", &first_commit).unwrap();

        let restored = std::fs::read_to_string(tmp.path().join("current.md")).unwrap();
        assert_eq!(restored.trim(), "v1 content");

        let versions = list_versions(tmp.path(), "current.md").unwrap();
        assert_eq!(versions.len(), 3, "restore must add a commit, not rewrite");
        assert!(versions[0].message.starts_with("restore:"));
    }
}
