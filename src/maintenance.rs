use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;

use crate::book::count_prose_words;
use crate::config::Config;
use crate::git;
use crate::state::InkState;

// ─── README helpers ────────────────────────────────────────────────────────────

/// Extract the first Markdown heading from `content` as a plain string.
/// Falls back to "Chapter N" if no heading is found.
fn extract_chapter_title(content: &str, chapter_num: u32) -> String {
    content
        .lines()
        .find_map(|line| {
            let stripped = line.trim_start_matches('#').trim();
            if line.starts_with('#') && !stripped.is_empty() {
                Some(stripped.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| format!("Chapter {}", chapter_num))
}

/// Rebuild the chapter list section in README.md.
///
/// Chapters 1..=`completed_through` are marked ✓.
/// `in_progress` (if Some) is marked *(in progress)*.
/// Chapters beyond `in_progress` (or `completed_through` when None) are not listed.
///
/// The section is delimited by the `<!-- INK:README:CHAPTERS -->` marker and the
/// next `\n---` separator. Non-fatal if README.md is absent or the marker is missing.
fn update_readme_chapters(
    repo: &Path,
    completed_through: u32,
    in_progress: Option<u32>,
) -> Result<()> {
    let readme_path = repo.join("README.md");
    if !readme_path.exists() {
        return Ok(());
    }

    const MARKER: &str = "<!-- INK:README:CHAPTERS -->";
    let content =
        std::fs::read_to_string(&readme_path).with_context(|| "Failed to read README.md")?;

    let Some(marker_pos) = content.find(MARKER) else {
        return Ok(());
    };

    // Build the chapter list
    let last = in_progress.unwrap_or(completed_through);
    let mut list = String::new();
    for i in 1..=last {
        let chapter_path = repo
            .join("Chapters material")
            .join(format!("Chapter_{:02}.md", i));
        let title = if chapter_path.exists() {
            let ch = std::fs::read_to_string(&chapter_path).unwrap_or_default();
            extract_chapter_title(&ch, i)
        } else {
            format!("Chapter {}", i)
        };
        let suffix = if Some(i) == in_progress {
            " *(in progress)*"
        } else {
            " ✓"
        };
        list.push_str(&format!("{}. **{}**{}\n", i, title, suffix));
    }

    // Replace from the marker line to the next \n--- separator (kept intact).
    // If the separator is absent the README has an unexpected structure — bail out
    // rather than silently truncating everything below the marker.
    let after_marker = &content[marker_pos + MARKER.len()..];
    let Some(sep_offset) = after_marker.find("\n---") else {
        return Ok(());
    };

    let new_content = format!(
        "{}{}\n\n{}{}",
        &content[..marker_pos],
        MARKER,
        list,
        &after_marker[sep_offset..]
    );

    std::fs::write(&readme_path, new_content).with_context(|| "Failed to write README.md")?;
    Ok(())
}

/// Update the `- **Status:**` line in README.md to `new_status`.
/// Non-fatal if README.md is absent.
fn update_readme_status(repo: &Path, new_status: &str) -> Result<()> {
    let readme_path = repo.join("README.md");
    if !readme_path.exists() {
        return Ok(());
    }
    let content =
        std::fs::read_to_string(&readme_path).with_context(|| "Failed to read README.md")?;
    let mut updated = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("- **Status:**") {
                format!("- **Status:** {}", new_status)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    std::fs::write(&readme_path, updated).with_context(|| "Failed to write README.md")?;
    Ok(())
}

// ─── advance-chapter ──────────────────────────────────────────────────────────

/// Advance to the next chapter by updating `.ink-state.yml`.
/// Returns `needs_chapter_outline` if the next chapter file is missing,
/// or `advanced` with the new chapter content on success.
/// Does NOT push — the caller decides when to sync.
pub fn advance_chapter(repo: &Path) -> Result<serde_json::Value> {
    let config = Config::load(repo)?;
    let mut state = InkState::load(repo)?;

    let next_chapter = state.current_chapter + 1;

    if next_chapter > config.chapter_count {
        return Ok(serde_json::json!({
            "status": "error",
            "message": format!("Already at last chapter ({}/{})", state.current_chapter, config.chapter_count),
        }));
    }

    // Guard: chapter must have reached ≥ 90 % of words_per_chapter
    let min_words = (config.words_per_chapter as f64 * 0.9) as u32;
    if state.current_chapter_word_count < min_words {
        return Ok(serde_json::json!({
            "status": "chapter_not_ready",
            "current_word_count": state.current_chapter_word_count,
            "target_word_count": config.words_per_chapter,
            "min_words_to_advance": min_words,
        }));
    }

    let chapter_filename = format!("Chapter_{:02}.md", next_chapter);
    let chapter_rel = format!("Chapters material/{}", chapter_filename);
    let chapter_path = repo.join(&chapter_rel);

    if !chapter_path.exists() {
        return Ok(serde_json::json!({
            "status": "needs_chapter_outline",
            "chapter": next_chapter,
            "chapter_file": chapter_rel,
        }));
    }

    let chapter_content = std::fs::read_to_string(&chapter_path)
        .with_context(|| format!("Failed to read {}", chapter_rel))?;

    // Advance state
    state.current_chapter = next_chapter;
    state.current_chapter_word_count = 0;
    state.save(repo)?;

    // Update README: mark previous chapter ✓, new chapter in progress
    update_readme_chapters(repo, next_chapter - 1, Some(next_chapter))?;
    update_readme_status(repo, &format!("In progress — Chapter {}", next_chapter))?;

    // Commit the state update (and chapter file + README if present)
    let readme_exists = repo.join("README.md").exists();
    let mut add_args = vec!["add", ".ink-state.yml", &chapter_rel];
    if readme_exists {
        add_args.push("README.md");
    }
    git::run_git(repo, &add_args).with_context(|| "Failed to git add for chapter advance")?;
    git::run_git(
        repo,
        &[
            "commit",
            "-m",
            &format!("chapter: advance to chapter {}", next_chapter),
        ],
    )
    .with_context(|| "Failed to commit chapter advance")?;

    info!("Advanced to chapter {}", next_chapter);

    Ok(serde_json::json!({
        "status": "advanced",
        "new_chapter": next_chapter,
        "chapter_file": chapter_rel,
        "chapter_content": chapter_content,
    }))
}

// ─── status ───────────────────────────────────────────────────────────────────

/// Return a lightweight read-only JSON snapshot of the book's current state.
/// Reads only local files — no git operations, no network.
pub fn book_status(repo: &Path) -> Result<serde_json::Value> {
    let state = InkState::load(repo)?;
    let config = Config::load(repo).ok();

    let book_path = repo.join("Current version").join("Full_Book.md");
    let total_word_count = if book_path.exists() {
        let content =
            std::fs::read_to_string(&book_path).with_context(|| "Failed to read Full_Book.md")?;
        count_prose_words(&content)
    } else {
        0
    };

    let lock_path = repo.join(".ink-running");
    let lock_age_seconds = crate::context::read_lock_age(repo);
    let complete = repo.join("COMPLETE").exists();
    let initialized = repo.join("Global Material").join("Config.yml").exists();

    let (
        target_length,
        words_per_chapter,
        words_per_session,
        chapter_close_suggested,
        completion_ready,
    ) = match &config {
        Some(c) => (
            c.target_length,
            c.words_per_chapter,
            c.words_per_session,
            state.current_chapter_word_count >= (c.words_per_chapter as f64 * 0.9) as u32,
            total_word_count >= (c.target_length as f64 * 0.9) as u32,
        ),
        None => (0, 0, 0, false, false),
    };

    Ok(serde_json::json!({
        "initialized": initialized,
        "complete": complete,
        "current_chapter": state.current_chapter,
        "current_chapter_word_count": state.current_chapter_word_count,
        "words_per_chapter": words_per_chapter,
        "chapter_close_suggested": chapter_close_suggested,
        "total_word_count": total_word_count,
        "target_length": target_length,
        "words_per_session": words_per_session,
        "completion_ready": completion_ready,
        "session_active": lock_path.exists(),
        "session_age_seconds": lock_age_seconds,
    }))
}

// ─── doctor ───────────────────────────────────────────────────────────────────

/// Validate the book repository structure and return a list of issues.
/// Checks file presence, Config.yml validity, git remote, draft branch, and lock state.
/// Note: the `git_remote_reachable` check makes a network call and may be slow on an
/// unreachable remote — all other checks are local-only.
pub fn doctor(repo: &Path) -> Result<serde_json::Value> {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut all_ok = true;

    macro_rules! check {
        ($name:expr, $ok:expr, $detail:expr) => {{
            let ok: bool = $ok;
            if !ok { all_ok = false; }
            checks.push(serde_json::json!({
                "name": $name,
                "ok": ok,
                "detail": $detail,
            }));
        }};
    }

    // ── Required Global Material files ───────────────────────────────────────
    for filename in &[
        "Config.yml",
        "Soul.md",
        "Outline.md",
        "Characters.md",
        "Lore.md",
    ] {
        let path = repo.join("Global Material").join(filename);
        check!(
            format!("global_{}", filename.to_lowercase().replace('.', "_")),
            path.exists(),
            if path.exists() {
                serde_json::Value::Null
            } else {
                serde_json::json!(format!("Global Material/{filename} not found"))
            }
        );
    }

    // ── Config.yml parses and validates ──────────────────────────────────────
    let loaded_config = Config::load(repo);
    match &loaded_config {
        Ok(cfg) => {
            check!("config_valid", true, serde_json::Value::Null);

            // ── Current chapter outline exists ────────────────────────────
            let state = InkState::load(repo).unwrap_or_default();
            let chapter_file = format!("Chapters material/Chapter_{:02}.md", state.current_chapter);
            let chapter_path = repo.join(&chapter_file);
            check!(
                "current_chapter_outline",
                chapter_path.exists(),
                if chapter_path.exists() {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(format!("{chapter_file} not found"))
                }
            );

            // ── Words-per-session sanity ──────────────────────────────────
            let sane = cfg.words_per_session >= 100 && cfg.words_per_session <= 10_000;
            check!(
                "words_per_session_sane",
                sane,
                if sane {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(format!(
                        "words_per_session={} — expected 100–10000",
                        cfg.words_per_session
                    ))
                }
            );
        }
        Err(e) => {
            check!("config_valid", false, serde_json::json!(e.to_string()));
            // Skip chapter check — can't read state without a valid config dir
        }
    }

    // ── Review/current.md ────────────────────────────────────────────────────
    let current_md = repo.join("Review").join("current.md");
    check!(
        "current_md",
        current_md.exists(),
        if current_md.exists() {
            serde_json::Value::Null
        } else {
            serde_json::json!("Review/current.md not found — run init first")
        }
    );

    // ── Git remote configured ─────────────────────────────────────────────────
    let remote_url = git::run_git(repo, &["remote", "get-url", "origin"]);
    check!(
        "git_remote_configured",
        remote_url.is_ok(),
        match &remote_url {
            Ok(url) => serde_json::json!(url),
            Err(e) => serde_json::json!(e.to_string()),
        }
    );

    // ── Git remote reachable (network call) ───────────────────────────────────
    if remote_url.is_ok() {
        match git::run_git(repo, &["ls-remote", "--exit-code", "--heads", "origin"]) {
            Ok(_) => check!("git_remote_reachable", true, serde_json::Value::Null),
            Err(e) => check!(
                "git_remote_reachable",
                false,
                serde_json::json!(e.to_string())
            ),
        }
    }

    // ── Draft branch exists locally ───────────────────────────────────────────
    let draft_exists = git::run_git(repo, &["show-ref", "--verify", "refs/heads/draft"]).is_ok();
    check!(
        "draft_branch",
        draft_exists,
        if draft_exists {
            serde_json::Value::Null
        } else {
            serde_json::json!("draft branch not found locally")
        }
    );

    // ── Session lock ──────────────────────────────────────────────────────────
    let lock_path = repo.join(".ink-running");
    if lock_path.exists() {
        let age = crate::context::read_lock_age(repo);
        let timeout = loaded_config
            .as_ref()
            .map(|c| c.session_timeout_minutes)
            .unwrap_or(60);
        let stale = age.map(|a| a > timeout).unwrap_or(false);
        check!(
            "session_lock",
            !stale,
            serde_json::json!(format!(
                "lock exists (age: {}m, timeout: {}m){}",
                age.unwrap_or(-1),
                timeout,
                if stale {
                    " — STALE, safe to remove"
                } else {
                    ""
                }
            ))
        );
    } else {
        check!("session_lock", true, serde_json::Value::Null);
    }

    Ok(serde_json::json!({
        "status": if all_ok { "healthy" } else { "issues" },
        "checks": checks,
    }))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── advance-chapter guard helpers ─────────────────────────────────────────

    fn write_test_config(dir: &std::path::Path, words_per_chapter: u32) {
        let global_dir = dir.join("Global Material");
        std::fs::create_dir_all(&global_dir).unwrap();
        let content = format!(
            "target_length: 80000\nchapter_count: 10\nchapter_structure: three-act\n\
             words_per_session: 800\nwords_per_chapter: {}\n",
            words_per_chapter
        );
        std::fs::write(global_dir.join("Config.yml"), content).unwrap();
    }

    fn write_test_state(dir: &std::path::Path, chapter: u32, word_count: u32) {
        let content = format!(
            "current_chapter: {}\ncurrent_chapter_word_count: {}\n",
            chapter, word_count
        );
        std::fs::write(dir.join(".ink-state.yml"), content).unwrap();
    }

    // ── advance-chapter guard tests ───────────────────────────────────────────

    #[test]
    fn advance_chapter_not_ready_below_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path(), 3000);
        write_test_state(tmp.path(), 1, 100);

        let result = advance_chapter(tmp.path()).unwrap();
        assert_eq!(result["status"], "chapter_not_ready");
        assert_eq!(result["current_word_count"], 100);
        assert_eq!(result["target_word_count"], 3000);
        assert_eq!(result["min_words_to_advance"], 2700);
    }

    #[test]
    fn advance_chapter_not_ready_at_zero_words() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path(), 3000);
        write_test_state(tmp.path(), 1, 0);

        let result = advance_chapter(tmp.path()).unwrap();
        assert_eq!(result["status"], "chapter_not_ready");
        assert_eq!(result["current_word_count"], 0);
    }

    // ── chapter_close_suggested formula tests (pure arithmetic, no I/O) ──────

    #[test]
    fn chapter_close_threshold_true_at_90_pct() {
        let words_per_chapter: u32 = 3000;
        let min: u32 = (words_per_chapter as f64 * 0.9) as u32; // 2700
        let current_word_count: u32 = 2700;
        assert!(current_word_count >= min);
    }

    #[test]
    fn chapter_close_threshold_false_below_90_pct() {
        let words_per_chapter: u32 = 3000;
        let min: u32 = (words_per_chapter as f64 * 0.9) as u32; // 2700
        let current_word_count: u32 = 2699;
        assert!(current_word_count < min);
    }

    // ── update_readme_chapters ────────────────────────────────────────────────

    #[test]
    fn readme_chapters_updates_chapter_list() {
        let tmp = tempfile::tempdir().unwrap();

        // Write Global Material so chapter titles can be resolved
        let global_dir = tmp.path().join("Global Material");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join("Config.yml"),
            "target_length: 80000\nchapter_count: 3\nchapter_structure: three-act\n\
             words_per_session: 800\nwords_per_chapter: 3000\n",
        )
        .unwrap();

        let readme = concat!(
            "# My Book\n\n",
            "<!-- INK:README:CHAPTERS -->\n",
            "1. **Chapter 1** *(in progress)*\n\n",
            "---\n",
            "*Footer content*\n",
        );
        std::fs::write(tmp.path().join("README.md"), readme).unwrap();

        update_readme_chapters(tmp.path(), 1, Some(2)).unwrap();

        let updated = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
        assert!(
            updated.contains("<!-- INK:README:CHAPTERS -->"),
            "marker preserved"
        );
        assert!(updated.contains("✓"), "completed chapter marked");
        assert!(
            updated.contains("in progress"),
            "in-progress chapter listed"
        );
        assert!(
            updated.contains("---\n*Footer content*"),
            "footer preserved"
        );
    }

    #[test]
    fn readme_chapters_no_separator_leaves_file_unchanged() {
        let tmp = tempfile::tempdir().unwrap();

        // README without the \n--- separator
        let readme = concat!(
            "# My Book\n\n",
            "<!-- INK:README:CHAPTERS -->\n",
            "1. **Chapter 1** *(in progress)*\n",
        );
        std::fs::write(tmp.path().join("README.md"), readme).unwrap();

        // Should return Ok(()) without writing
        update_readme_chapters(tmp.path(), 1, Some(2)).unwrap();

        let after = std::fs::read_to_string(tmp.path().join("README.md")).unwrap();
        assert_eq!(
            after, readme,
            "file must be unchanged when separator is absent"
        );
    }
}
