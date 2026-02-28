use anyhow::{bail, Context, Result};
use chrono::Local;
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
            if line.len() >= 3 {
                let filename = line[3..].trim().to_string();
                if !filename.is_empty() {
                    return Some(filename);
                }
            }
            None
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
    let tag = format!("ink-{}", Local::now().format("%Y-%m-%d-%H-%M"));

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
    // Try to checkout existing draft branch, create it if it doesn't exist
    let checkout_result = run_git(repo, &["checkout", "draft"]);

    match checkout_result {
        Ok(_) => {
            info!("Checked out existing draft branch");
        }
        Err(_) => {
            info!("Creating new draft branch...");
            run_git(repo, &["checkout", "-b", "draft"])
                .with_context(|| "Failed to create draft branch")?;
        }
    }

    info!("Rebasing draft onto main...");
    run_git(repo, &["rebase", "main"]).with_context(|| "Failed to rebase draft onto main")?;

    Ok(())
}
