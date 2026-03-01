use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;
use std::path::Path;
use std::sync::OnceLock;
use tracing::{info, warn};

use crate::config::Config;
use crate::git;
use crate::state::InkState;

// ─── Shared regex (compiled once) ────────────────────────────────────────────

/// Returns the compiled regex for author INK instructions.
/// The mandatory space after `INK:` ensures engine markers are never matched.
/// Must stay consistent with maintenance.rs.
fn ink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<!-- INK: (.*?) -->").unwrap())
}

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
pub struct Chapters {
    pub current: Option<ChapterInfo>,
    pub next: Option<ChapterInfo>,
}

#[derive(Debug, Serialize)]
pub struct Instruction {
    pub anchor: String,
    pub instruction: String,
}

#[derive(Debug, Serialize)]
pub struct CurrentReview {
    pub content: String,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Serialize)]
pub struct WordCount {
    pub total: u32,
    pub target: u32,
    pub remaining: u32,
}

#[derive(Debug, Serialize)]
pub struct SessionPayload {
    pub session_already_run: bool,
    pub kill_requested: bool,
    pub stale_lock_recovered: bool,
    pub snapshot_tag: String,
    pub human_edits: Vec<String>,
    pub config: ConfigSnapshot,
    pub global_material: Vec<FileContent>,
    pub chapters: Chapters,
    pub current_review: CurrentReview,
    pub word_count: WordCount,
    pub chapter_close_suggested: bool,
    pub current_chapter_word_count: u32,
    pub chapter_progress_pct: u8,
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

fn lock_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".ink-running")
}

fn kill_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".ink-kill")
}

/// Returns age of the lock file in minutes, or None if no lock exists.
pub fn read_lock_age(repo: &Path) -> Option<i64> {
    let path = lock_path(repo);
    let content = std::fs::read_to_string(&path).ok()?;
    let timestamp: DateTime<Utc> = content.trim().parse().ok()?;
    let age = Utc::now().signed_duration_since(timestamp).num_minutes();
    Some(age)
}

/// Writes .ink-running with current UTC timestamp, commits and pushes.
pub fn create_lock(repo: &Path) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    std::fs::write(lock_path(repo), &now).with_context(|| "Failed to write .ink-running")?;

    git::run_git(repo, &["add", ".ink-running"])
        .with_context(|| "Failed to git add .ink-running")?;
    git::run_git(repo, &["commit", "-m", "chore: open session lock"])
        .with_context(|| "Failed to commit .ink-running")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push .ink-running")?;

    info!("Session lock created at {}", now);
    Ok(())
}

/// Removes the stale lock from the local filesystem only.
/// Safe because `create_lock` (called immediately after) stages a fresh `.ink-running`
/// with the current timestamp and pushes it, overwriting whatever was on the remote.
/// Do NOT use this on the kill path — use `git rm --ignore-unmatch .ink-running` there
/// so the removal is committed and pushed before returning.
pub fn remove_stale_lock(repo: &Path) -> Result<()> {
    let path = lock_path(repo);
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| "Failed to remove stale .ink-running")?;
        warn!("Stale lock removed");
    }
    Ok(())
}

/// Removes .ink-kill via git rm, commits, and pushes.
pub fn delete_kill_file(repo: &Path) -> Result<()> {
    git::run_git(repo, &["rm", "-f", ".ink-kill"]).with_context(|| "Failed to git rm .ink-kill")?;
    git::run_git(repo, &["commit", "-m", "chore: acknowledge kill request"])
        .with_context(|| "Failed to commit kill acknowledgement")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push kill acknowledgement")?;
    info!("Kill file removed");
    Ok(())
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
        .filter_map(|e| {
            let path = e.path();
            let filename = path.file_name()?.to_string_lossy().to_string();
            // Skip Config.yml — it's surfaced separately
            if filename == "Config.yml" {
                return None;
            }
            let mut content = std::fs::read_to_string(&path).ok()?;
            if filename == "Summary.md" {
                content = truncate_summary(&content, summary_entries);
            }
            Some(FileContent { filename, content })
        })
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

/// Truncate `text` to at most `max_words` prose words, respecting paragraph boundaries.
/// The last paragraph is always included even if it alone exceeds `max_words`.
fn truncate_to_last_words(text: &str, max_words: u32) -> String {
    let paras: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut accumulated: u32 = 0;
    let mut start_idx = paras.len();

    for (i, para) in paras.iter().enumerate().rev() {
        let para_words = para.split_whitespace().count() as u32;
        // Stop accumulating once we'd exceed the limit — but always include at least
        // the last paragraph (start_idx == paras.len() guard).
        if accumulated + para_words > max_words && start_idx < paras.len() {
            break;
        }
        accumulated += para_words;
        start_idx = i;
    }

    if start_idx == paras.len() {
        return String::new();
    }
    paras[start_idx..].join("\n\n")
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

pub fn extract_ink_instructions(text: &str) -> (String, Vec<Instruction>) {
    let re = ink_re();
    let mut instructions = Vec::new();

    for cap in re.captures_iter(text) {
        let full_match = cap.get(0).unwrap();
        let instruction_text = cap[1].trim().to_string();

        // Anchor = up to 200 chars of text preceding this comment
        let start = full_match.start();
        let preceding = &text[..start];
        let anchor: String = preceding
            .chars()
            .rev()
            .take(200)
            .collect::<String>()
            .chars()
            .rev()
            .collect();

        instructions.push(Instruction {
            anchor: anchor.trim().to_string(),
            instruction: instruction_text,
        });
    }

    // Strip only author instruction comments; engine markers (INK:NEW:, INK:REWORKED:)
    // are preserved so the engine can see what it wrote last session.
    let stripped = re.replace_all(text, "").to_string();
    (stripped, instructions)
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

    // Use the same counter as session-close so both modules always agree.
    let total = crate::maintenance::count_prose_words(&content);
    let remaining = target.saturating_sub(total);

    Ok(WordCount {
        total,
        target,
        remaining,
    })
}

// ─── Main orchestration ───────────────────────────────────────────────────────

pub fn session_open(repo: &Path) -> Result<SessionPayload> {
    // 1. Fetch remote state and switch to main — do NOT merge yet so that
    //    uncommitted local edits (e.g. INK instructions saved in an IDE) are
    //    detected and committed before origin/main can overwrite them.
    info!("Step 1: fetch and checkout main");
    git::preflight_fetch_and_checkout(repo)?;

    // 2. Check for kill file — must happen before any git writes
    let kill_requested = kill_path(repo).exists();
    if kill_requested {
        info!("Kill file detected — acknowledging and aborting");
        // Stage the lock removal via git so it is included in the kill commit and pushed.
        // --ignore-unmatch avoids a failure when no lock exists.
        git::run_git(repo, &["rm", "--ignore-unmatch", ".ink-running"])
            .with_context(|| "Failed to git rm .ink-running on kill")?;
        delete_kill_file(repo)?;

        return Ok(SessionPayload {
            session_already_run: false,
            kill_requested: true,
            stale_lock_recovered: false,
            snapshot_tag: String::new(),
            human_edits: vec![],
            config: ConfigSnapshot {
                target_length: 0,
                chapter_count: 0,
                chapter_structure: String::new(),
                words_per_session: 0,
                summary_context_entries: 5,
                words_per_chapter: 3000,
                current_chapter: 1,
            },
            global_material: vec![],
            chapters: Chapters {
                current: None,
                next: None,
            },
            current_review: CurrentReview {
                content: String::new(),
                instructions: vec![],
            },
            word_count: WordCount {
                total: 0,
                target: 0,
                remaining: 0,
            },
            chapter_close_suggested: false,
            current_chapter_word_count: 0,
            chapter_progress_pct: 0,
        });
    }

    // 3. Load config and state
    info!("Step 3: loading config and state");
    let config = Config::load(repo)?;
    let state = InkState::load(repo)?;

    // 3b. Compute chapter close suggestion early — needed to decide whether to load
    //     the next chapter outline (skip it when not near a chapter boundary).
    let chapter_close_suggested =
        state.current_chapter_word_count >= (config.words_per_chapter as f64 * 0.9) as u32;

    // 4. Collect human edits BEFORE merging with origin so that local
    //    uncommitted changes (IDE saves, INK instructions, etc.) are captured
    //    and committed before the ff-merge can overwrite them.
    //
    //    Two complementary methods — union:
    //    a) git status --short   → uncommitted working-tree changes vs HEAD
    //    b) git diff origin/main → ALL diffs between local tree and remote,
    //       catching edits made when local HEAD was already behind origin
    info!("Step 4: collecting human edits (local working tree + diff vs origin)");
    let mut human_edits = git::collect_modified_files(repo)?;
    for f in git::collect_diffs_vs_remote(repo)? {
        if !human_edits.contains(&f) {
            human_edits.push(f);
        }
    }

    // 5. Commit human edits locally (no push — push_tags handles that below)
    if !human_edits.is_empty() {
        info!("Step 5: committing {} human edit(s)", human_edits.len());
        git::commit_human_edits(repo, &human_edits)?;
    }

    // 5b. Now safe to merge: local changes are committed, so the ff-merge
    //     cannot overwrite them.
    info!("Step 5b: fast-forward merging origin/main");
    git::merge_ff_origin_main(repo)?;

    // 6. Create snapshot tag
    info!("Step 6: creating snapshot tag");
    let snapshot_tag = git::create_snapshot_tag(repo)?;

    // 7. Push main + tags
    info!("Step 7: pushing main + tags");
    git::push_tags(repo)?;

    // 8. Check lock
    info!("Step 8: checking session lock");
    let mut stale_lock_recovered = false;

    match read_lock_age(repo) {
        None => {
            // No lock — proceed normally
        }
        Some(age) if age <= config.session_timeout_minutes => {
            info!("Active lock found (age {}m) — session already running", age);
            return Ok(SessionPayload {
                session_already_run: true,
                kill_requested: false,
                stale_lock_recovered: false,
                snapshot_tag,
                human_edits,
                config: ConfigSnapshot::new(&config, state.current_chapter),
                global_material: vec![],
                chapters: Chapters {
                    current: None,
                    next: None,
                },
                current_review: CurrentReview {
                    content: String::new(),
                    instructions: vec![],
                },
                word_count: WordCount {
                    total: 0,
                    target: config.target_length,
                    remaining: 0,
                },
                chapter_close_suggested: false,
                current_chapter_word_count: state.current_chapter_word_count,
                chapter_progress_pct: 0,
            });
        }
        Some(age) => {
            warn!("Stale lock detected (age {}m) — recovering", age);
            remove_stale_lock(repo)?;
            stale_lock_recovered = true;
        }
    }

    // 9. Create new session lock
    info!("Step 9: creating session lock");
    create_lock(repo)?;

    // 10. Setup draft branch
    info!("Step 10: setting up draft branch");
    git::setup_draft_branch(repo)?;

    // 11. Load global material
    info!("Step 11: loading global material");
    let global_material = load_global_material(repo, config.summary_context_entries)?;

    // 12. Load current chapter
    info!("Step 12: loading chapter {}", state.current_chapter);
    let current_chapter = load_chapter(repo, state.current_chapter, &human_edits)?;

    // 13. Load next chapter only when chapter close is approaching — avoids sending
    //     the outline tokens every session when not near a chapter boundary.
    let next_chapter = if chapter_close_suggested {
        info!(
            "Step 13: chapter close suggested — loading next chapter {}",
            state.current_chapter + 1
        );
        load_chapter(repo, state.current_chapter + 1, &human_edits)?
    } else {
        info!("Step 13: chapter close not suggested — skipping next chapter load");
        None
    };

    // 14. Read current.md + extract INK instructions
    info!("Step 14: loading current review");
    let review_path = repo.join("Review").join("current.md");
    let raw_review = if review_path.exists() {
        std::fs::read_to_string(&review_path).with_context(|| "Failed to read Review/current.md")?
    } else {
        String::new()
    };
    let (mut stripped_review, instructions) = extract_ink_instructions(&raw_review);

    // 14b. Truncate the rolling window to stay within the model's context budget.
    //      Reserve OVERHEAD_TOKENS for system prompt, Global Material, chapters,
    //      summary, agent reasoning, and generated prose. The remainder is
    //      converted to words (÷ 1.35 tokens/word) and used as the hard cap.
    {
        const OVERHEAD_TOKENS: u32 = 60_000;
        const TOKENS_PER_WORD: f64 = 1.35;
        let max_words = if config.context_window_tokens > OVERHEAD_TOKENS {
            ((config.context_window_tokens - OVERHEAD_TOKENS) as f64 / TOKENS_PER_WORD) as u32
        } else {
            2_000 // minimum fallback for very small context models
        };
        let word_count = stripped_review.split_whitespace().count() as u32;
        if word_count > max_words {
            info!(
                "Step 14b: truncating current.md from {} words to last {} words \
                 (context budget: {} tokens)",
                word_count, max_words, config.context_window_tokens
            );
            stripped_review = truncate_to_last_words(&stripped_review, max_words);
        }
    }

    // 15. Load word count
    info!("Step 15: loading word count");
    let word_count = load_word_count(repo, config.target_length)?;

    // 16. Build payload
    let chapter_progress_pct = state
        .current_chapter_word_count
        .saturating_mul(100)
        .checked_div(config.words_per_chapter)
        .unwrap_or(0)
        .min(100) as u8;

    Ok(SessionPayload {
        session_already_run: false,
        kill_requested: false,
        stale_lock_recovered,
        snapshot_tag,
        human_edits,
        config: ConfigSnapshot::new(&config, state.current_chapter),
        global_material,
        chapters: Chapters {
            current: current_chapter,
            next: next_chapter,
        },
        current_review: CurrentReview {
            content: stripped_review,
            instructions,
        },
        word_count,
        chapter_close_suggested,
        current_chapter_word_count: state.current_chapter_word_count,
        chapter_progress_pct,
    })
}
