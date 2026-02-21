use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;
use std::path::Path;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::config::Config;
use crate::git;

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
}

#[derive(Debug, Serialize)]
pub struct ConfigSnapshot {
    pub target_length: u32,
    pub chapter_count: u32,
    pub chapter_structure: String,
    pub words_per_session: u32,
    pub summary_context_entries: usize,
    pub current_chapter: u32,
}

impl From<&Config> for ConfigSnapshot {
    fn from(c: &Config) -> Self {
        ConfigSnapshot {
            target_length: c.target_length,
            chapter_count: c.chapter_count,
            chapter_structure: c.chapter_structure.clone(),
            words_per_session: c.words_per_session,
            summary_context_entries: c.summary_context_entries,
            current_chapter: c.current_chapter,
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
    std::fs::write(lock_path(repo), &now)
        .with_context(|| "Failed to write .ink-running")?;

    git::run_git(repo, &["add", ".ink-running"])
        .with_context(|| "Failed to git add .ink-running")?;
    git::run_git(repo, &["commit", "-m", "chore: open session lock"])
        .with_context(|| "Failed to commit .ink-running")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push .ink-running")?;

    info!("Session lock created at {}", now);
    Ok(())
}

/// Removes a stale lock file (local only, no git op needed — overwritten by create_lock next).
pub fn remove_stale_lock(repo: &Path) -> Result<()> {
    let path = lock_path(repo);
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| "Failed to remove stale .ink-running")?;
        warn!("Stale lock removed");
    }
    Ok(())
}

/// Removes .ink-kill via git rm, commits, and pushes.
pub fn delete_kill_file(repo: &Path) -> Result<()> {
    git::run_git(repo, &["rm", "-f", ".ink-kill"])
        .with_context(|| "Failed to git rm .ink-kill")?;
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
    let mut files: Vec<FileContent> = WalkDir::new(&global_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            let filename = path.file_name()?.to_string_lossy().to_string();
            // Skip Config.yml — it's surfaced separately
            if filename == "Config.yml" {
                return None;
            }
            let mut content = std::fs::read_to_string(path).ok()?;
            if filename == "Summary.md" {
                content = truncate_summary(&content, summary_entries);
            }
            Some(FileContent { filename, content })
        })
        .collect();

    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(files)
}

pub fn truncate_summary(text: &str, n: usize) -> String {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let start = paragraphs.len().saturating_sub(n);
    paragraphs[start..].join("\n\n")
}

pub fn load_chapter(
    repo: &Path,
    num: u32,
    human_edits: &[String],
) -> Result<Option<ChapterInfo>> {
    let relative = format!("Chapters material/Chapter_{:02}.md", num);
    let path = repo.join(&relative);

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read chapter {}", num))?;

    let modified_today = human_edits.iter().any(|f| f.contains(&format!("Chapter_{:02}.md", num)));

    Ok(Some(ChapterInfo {
        path: relative,
        content,
        modified_today,
    }))
}

pub fn extract_ink_instructions(text: &str) -> (String, Vec<Instruction>) {
    let re = Regex::new(r"(?s)<!--\s*INK:\s*(.*?)\s*-->").expect("Invalid INK regex");
    let mut instructions = Vec::new();

    for cap in re.captures_iter(text) {
        let full_match = cap.get(0).unwrap();
        let instruction_text = cap[1].to_string();

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

    // Strip all INK comment tags from the content
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

    let content = std::fs::read_to_string(&path)
        .with_context(|| "Failed to read Full_Book.md")?;

    let total = content.split_whitespace().count() as u32;
    let remaining = target.saturating_sub(total);

    Ok(WordCount { total, target, remaining })
}

// ─── Main orchestration ───────────────────────────────────────────────────────

pub fn session_open(repo: &Path) -> Result<SessionPayload> {
    // 1. Preflight git sync
    info!("Step 1: preflight sync");
    git::preflight_sync(repo)?;

    // 2. Check for kill file — must happen before any git writes
    let kill_requested = kill_path(repo).exists();
    if kill_requested {
        info!("Kill file detected — acknowledging and aborting");
        // Remove lock if present
        if lock_path(repo).exists() {
            remove_stale_lock(repo)?;
        }
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
                current_chapter: 1,
            },
            global_material: vec![],
            chapters: Chapters { current: None, next: None },
            current_review: CurrentReview {
                content: String::new(),
                instructions: vec![],
            },
            word_count: WordCount { total: 0, target: 0, remaining: 0 },
        });
    }

    // 3. Load config
    info!("Step 3: loading config");
    let config = Config::load(repo)?;

    // 4. Collect modified files BEFORE committing (reflects human changes)
    info!("Step 4: collecting human edits");
    let human_edits = git::collect_modified_files(repo)?;

    // 5. Commit human edits if any
    if !human_edits.is_empty() {
        info!("Step 5: committing {} human edit(s)", human_edits.len());
        git::commit_human_edits(repo, &human_edits)?;
    }

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
                config: ConfigSnapshot::from(&config),
                global_material: vec![],
                chapters: Chapters { current: None, next: None },
                current_review: CurrentReview {
                    content: String::new(),
                    instructions: vec![],
                },
                word_count: WordCount { total: 0, target: config.target_length, remaining: 0 },
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

    // 12 & 13. Load current and next chapters
    info!("Step 12-13: loading chapters {} and {}", config.current_chapter, config.current_chapter + 1);
    let current_chapter = load_chapter(repo, config.current_chapter, &human_edits)?;
    let next_chapter = load_chapter(repo, config.current_chapter + 1, &human_edits)?;

    // 14. Read current.md + extract INK instructions
    info!("Step 14: loading current review");
    let review_path = repo.join("Review").join("current.md");
    let raw_review = if review_path.exists() {
        std::fs::read_to_string(&review_path)
            .with_context(|| "Failed to read Review/current.md")?
    } else {
        String::new()
    };
    let (stripped_review, instructions) = extract_ink_instructions(&raw_review);

    // 15. Load word count
    info!("Step 15: loading word count");
    let word_count = load_word_count(repo, config.target_length)?;

    // 16. Build payload
    Ok(SessionPayload {
        session_already_run: false,
        kill_requested: false,
        stale_lock_recovered,
        snapshot_tag,
        human_edits,
        config: ConfigSnapshot::from(&config),
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
    })
}
