use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

const CONFIG_YML: &str = include_str!("../templates/Config.yml");
const SOUL_MD: &str = include_str!("../templates/Soul.md");
const OUTLINE_MD: &str = include_str!("../templates/Outline.md");
const CHARACTERS_MD: &str = include_str!("../templates/Characters.md");
const LORE_MD: &str = include_str!("../templates/Lore.md");
const CHAPTER_01_MD: &str = include_str!("../templates/Chapter_01.md");
const CURRENT_MD: &str = include_str!("../templates/current.md");
const AGENTS_MD: &str = include_str!("../templates/AGENTS.md");

#[derive(Serialize)]
pub struct Question {
    pub question: &'static str,
    pub target_file: &'static str,
}

#[derive(Serialize)]
pub struct InitPayload {
    pub status: &'static str,
    pub title: String,
    pub author: String,
    pub files_created: Vec<String>,
    pub questions: Vec<Question>,
}

fn fill(template: &str, title: &str, author: &str) -> String {
    template
        .replace("{{TITLE}}", title)
        .replace("{{AUTHOR}}", author)
}

pub fn run_init(repo_path: &Path, title: &str, author: &str) -> Result<InitPayload> {
    // Guard: already initialized
    let config_path = repo_path.join("Global Material/Config.yml");
    if config_path.exists() {
        eprintln!(
            "{}",
            serde_json::json!({
                "error": "repository already initialized",
                "status": "error"
            })
        );
        std::process::exit(1);
    }

    let mut files_created: Vec<String> = Vec::new();

    // Create directories
    for dir in &[
        "Global Material",
        "Chapters material",
        "Review",
        "Changelog",
        "Current version",
    ] {
        fs::create_dir_all(repo_path.join(dir))?;
    }

    // Helper closure to write a file and record its relative path
    let write_file = |rel: &str, contents: &str, files: &mut Vec<String>| -> Result<()> {
        let full = repo_path.join(rel);
        fs::write(&full, contents)?;
        files.push(rel.to_string());
        Ok(())
    };

    write_file(
        "Global Material/Config.yml",
        &fill(CONFIG_YML, title, author),
        &mut files_created,
    )?;
    write_file(
        "Global Material/Soul.md",
        &fill(SOUL_MD, title, author),
        &mut files_created,
    )?;
    write_file(
        "Global Material/Outline.md",
        &fill(OUTLINE_MD, title, author),
        &mut files_created,
    )?;
    write_file(
        "Global Material/Characters.md",
        &fill(CHARACTERS_MD, title, author),
        &mut files_created,
    )?;
    write_file(
        "Global Material/Lore.md",
        &fill(LORE_MD, title, author),
        &mut files_created,
    )?;
    // Summary.md — empty, append-only
    write_file("Global Material/Summary.md", "", &mut files_created)?;

    write_file(
        "Chapters material/Chapter_01.md",
        &fill(CHAPTER_01_MD, title, author),
        &mut files_created,
    )?;

    write_file(
        "Review/current.md",
        &fill(CURRENT_MD, title, author),
        &mut files_created,
    )?;

    // AGENTS.md — standalone engine instructions at repo root
    write_file("AGENTS.md", AGENTS_MD, &mut files_created)?;

    // Changelog/.gitkeep — keeps the empty directory tracked by git
    write_file("Changelog/.gitkeep", "", &mut files_created)?;

    // Full_Book.md — starts empty
    write_file("Current version/Full_Book.md", "", &mut files_created)?;

    // Git operations
    git_commit_and_push(repo_path)?;

    let questions = vec![
        Question {
            question: "What language should the engine write in? \
                       (e.g. English, French, Spanish, German, Italian — use the full language name)",
            target_file: "Global Material/Config.yml",
        },
        Question {
            question: "What is the narrative voice, tone, and prose style for this book? \
                       Describe the narrator, sentence rhythm, vocabulary level, and emotional register.",
            target_file: "Global Material/Soul.md",
        },
        Question {
            question: "What is the full plot arc — beginning, middle, and end? \
                       Include the central conflict, major turning points, and resolution.",
            target_file: "Global Material/Outline.md",
        },
        Question {
            question: "Who are the main characters? For each: name, personality, motivation, \
                       key relationships, and arc across the book.",
            target_file: "Global Material/Characters.md",
        },
        Question {
            question: "Describe the world: its setting, history, geography, societies, \
                       and any rules or lore the engine must respect.",
            target_file: "Global Material/Lore.md",
        },
        Question {
            question: "What should happen in Chapter 1? List the key beats, scenes, \
                       and what the reader should feel by the end of it.",
            target_file: "Chapters material/Chapter_01.md",
        },
    ];

    Ok(InitPayload {
        status: "initialized",
        title: title.to_string(),
        author: author.to_string(),
        files_created,
        questions,
    })
}

// ─── reset ────────────────────────────────────────────────────────────────────

/// Wipe all book content so the repository can be re-initialized with `init`.
/// The user must type the repository directory name to confirm — this is a
/// destructive, irreversible operation.
pub fn run_reset(repo_path: &Path) -> Result<()> {
    let repo_name = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf())
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("this-repository")
        .to_string();

    println!("\n  ⚠  Reset will permanently delete all book content in «{}».", repo_name);
    println!("  The git history is preserved, but all files will be removed.");
    println!("  You can re-run `ink-cli init` afterwards to start fresh.\n");
    println!("  To confirm, type the repository name:\n");

    let input: String = dialoguer::Input::<String>::new()
        .with_prompt(format!("  Type «{}» to confirm", repo_name))
        .interact_text()
        .with_context(|| "Failed to read confirmation input")?;

    if input.trim() != repo_name {
        println!("\n  Name does not match — reset cancelled.\n");
        return Ok(());
    }

    println!("\n  Removing book content…");

    // Remove all tracked content directories and files in one git rm call.
    // --ignore-unmatch silences errors for files that don't exist.
    let _ = Command::new("git")
        .args([
            "rm", "-rf", "--ignore-unmatch",
            "Global Material/",
            "Chapters material/",
            "Review/",
            "Changelog/",
            "Current version/",
            "AGENTS.md",
            "COMPLETE",
            ".ink-running",
            ".ink-kill",
        ])
        .current_dir(repo_path)
        .status();

    // Re-create .gitkeep placeholders so the directories exist for the next init
    for dir in &["Changelog", "Chapters material", "Review", "Current version"] {
        let dir_path = repo_path.join(dir);
        fs::create_dir_all(&dir_path)?;
        fs::write(dir_path.join(".gitkeep"), "")?;
    }

    let run = |args: &[&str]| -> Result<()> {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("git {} failed", args.join(" "));
        }
        Ok(())
    };

    run(&["add", "-A"])?;
    run(&["commit", "-m", "reset: wipe book content for re-initialization"])?;

    let push = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;
    if !push.success() {
        tracing::warn!("git push skipped — no remote configured");
    }

    println!("\n  Reset complete.");
    println!("  Run `ink-cli init <repo-path> --title \"...\" --author \"...\"` to start fresh.\n");

    Ok(())
}

fn git_commit_and_push(repo_path: &Path) -> Result<()> {
    let run = |args: &[&str]| -> Result<()> {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("git {} failed with status {}", args.join(" "), status);
        }
        Ok(())
    };

    run(&["add", "-A"])?;
    run(&["commit", "-m", "init: scaffold book repository"])?;

    // Push is best-effort: skip if no remote is configured (common in local smoke tests)
    let push_status = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;

    if !push_status.success() {
        tracing::warn!("git push origin main failed — no remote configured or push rejected; skipping");
    }

    Ok(())
}

// ─── Interactive Q&A (TTY mode) ───────────────────────────────────────────────

/// Run when `init` is called from a real terminal. Asks each question
/// interactively and writes the answers directly to their target files, then
/// commits and pushes — so the book is fully ready in one shot.
pub fn run_interactive_qa(repo_path: &Path, payload: &InitPayload) -> Result<()> {
    let total = payload.questions.len();

    println!("\n  Ink Gateway — Book Setup");
    println!("  «{}» by {}", payload.title, payload.author);
    println!("  Answer {total} questions to configure your book.\n");

    for (i, q) in payload.questions.iter().enumerate() {
        println!("  ── [{}/{}] ─────────────────────────────────────", i + 1, total);
        println!("  {}\n", q.question);

        let answer = if q.target_file == "Global Material/Config.yml" {
            // Language: single-line prompt with default
            dialoguer::Input::<String>::new()
                .with_prompt("  Language")
                .default("English".into())
                .interact_text()
                .with_context(|| "Failed to read language input")?
        } else {
            // All other questions: open $EDITOR with the question as a comment header
            let prefill = format!(
                "# {}\n# Write your answer below. Delete this header. Save and close to continue.\n\n",
                q.question
            );
            match dialoguer::Editor::new()
                .extension(".md")
                .edit(&prefill)
                .with_context(|| "Failed to open editor")?
            {
                Some(text) => strip_comment_header(&text),
                None => {
                    println!("  (skipped — seed template kept)\n");
                    continue;
                }
            }
        };

        write_qa_answer(repo_path, q.target_file, &answer)
            .with_context(|| format!("Failed to write {}", q.target_file))?;
        println!("  ✓  {}\n", q.target_file);
    }

    // Commit all answers
    println!("  Committing answers…");
    commit_qa_answers(repo_path)?;
    println!("\n  Book is ready. Review Global Material/ in your editor and");
    println!("  start the first writing session when satisfied.\n");

    Ok(())
}

/// Strip leading `#` comment lines (and the blank line after them) that were
/// added as an instruction header in the editor pre-fill.
fn strip_comment_header(text: &str) -> String {
    text.lines()
        .skip_while(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_start_matches('\n')
        .to_string()
}

/// Write a Q&A answer to its target file.
/// Config.yml is special — only the `language:` field is updated.
/// All other files have their full contents replaced.
fn write_qa_answer(repo_path: &Path, target_file: &str, answer: &str) -> Result<()> {
    let path = repo_path.join(target_file);
    if target_file == "Global Material/Config.yml" {
        let content = fs::read_to_string(&path)?;
        let updated = content
            .lines()
            .map(|line| {
                if line.starts_with("language:") {
                    format!("language: {}", answer.trim())
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{}\n", updated))?;
    } else {
        fs::write(&path, answer)?;
    }
    Ok(())
}

fn commit_qa_answers(repo_path: &Path) -> Result<()> {
    let run = |args: &[&str]| -> Result<()> {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .status()?;
        if !status.success() {
            anyhow::bail!("git {} failed", args.join(" "));
        }
        Ok(())
    };

    run(&["add", "-A"])?;
    run(&["commit", "-m", "init: populate global material from author Q&A"])?;

    let push = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;
    if !push.success() {
        tracing::warn!("git push skipped — no remote configured");
    }

    Ok(())
}
