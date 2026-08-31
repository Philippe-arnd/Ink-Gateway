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

/// Creates a lightweight tag at the current HEAD, e.g. as an undo point
/// before a writing-session's edits begin (see `Ink-Gateway-App`'s
/// `sessions.rs`, which tags before running its intent-typed sessions and
/// offers `restore_version` back to the tag as "reject").
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
/// **new** commit — history is never rewritten.
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

/// Paths that differ between `tag` and current HEAD — every file a session
/// touched since its snapshot tag. Backs the web app's multi-file session
/// diff view.
pub fn changed_files(repo: &Path, tag: &str) -> Result<Vec<String>> {
    let output = run_git(repo, &["diff", "--name-only", tag, "HEAD"])
        .with_context(|| format!("Failed to diff against tag {}", tag))?;
    Ok(output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect())
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

    #[test]
    fn changed_files_lists_paths_touched_since_tag() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        commit_file(tmp.path(), "a.md", "v1", "seed a");
        let tag = create_snapshot_tag(tmp.path()).unwrap();
        commit_file(tmp.path(), "a.md", "v2", "change a");
        commit_file(tmp.path(), "b.md", "new", "add b");

        let files = changed_files(tmp.path(), &tag).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"a.md".to_string()));
        assert!(files.contains(&"b.md".to_string()));
    }

    #[test]
    fn changed_files_empty_when_nothing_changed_since_tag() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        commit_file(tmp.path(), "a.md", "v1", "seed a");
        let tag = create_snapshot_tag(tmp.path()).unwrap();

        let files = changed_files(tmp.path(), &tag).unwrap();
        assert!(files.is_empty());
    }
}
