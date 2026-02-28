use anyhow::{anyhow, Context, Result};
use inquire::{Confirm, Select, Text};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

// ─── Seed content ─────────────────────────────────────────────────────────────

/// Written to CLAUDE.md and GEMINI.md by `ink-cli seed`.
/// Self-contained: guides any AI agent from an empty repo through init without
/// requiring AGENTS.md to already exist.
const SEED_CONTENT: &str = "\
# Ink Gateway — Book Repository

You are the **ink-engine** fiction writing agent. Your sole interface to this repository is `ink-cli`.

---

## Step 1 — Verify ink-cli

```bash
ink-cli --version
```

If not found, install it:

```bash
curl -fsSL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | bash
```

---

## Step 2 — Detect repository state

Check whether `Global Material/Config.yml` exists.

| State | Action |
|---|---|
| **Absent** | Repository not initialized — follow §Initialize below |
| **Present** | Repository is ready — follow `AGENTS.md` for writing sessions |

---

## Initialize

```bash
ink-cli init <repo-path> --title \"<Book Title>\" --author \"<Author Name>\" --agent
```

Derive `--title` from the repository directory name (hyphens/underscores → spaces, title case).
Derive `--author` from the GitHub URL, repository metadata, or ask the user if unknown.

The command outputs JSON with a `questions` array. Each entry has `question`, `hint`, and `target_file`.

**Ask the author each question in order.** Once you have all 13 answers, extrapolate each brief answer into rich, detailed content, then write the files directly:

Questions 1–4 populate `Config.yml`:
- Q1: language → `language:` field
- Q2: book type (Flash fiction / Short story / Novel) — use to infer defaults for Q3 and Q4
- Q3: target pages → `target_length: <pages × 250>`; also compute `chapter_count: <ceil(target_words / 3000)>`
- Q4: pages per session → `words_per_session: <pages × 250>`

| File | What to write |
|---|---|
| `Global Material/Config.yml` | Update `language:`, `target_length:`, `words_per_session:`, and `chapter_count:` fields. Do not overwrite other fields. |
| `Global Material/Soul.md` | Full style guide (2–4 paragraphs): narrator voice, sentence rhythm, vocabulary level, emotional register, what to avoid. |
| `Global Material/Characters.md` | Full character sheet per character: appearance hints, personality, motivation, internal conflict, key relationships, arc across the book. |
| `Global Material/Outline.md` | Structured plot outline: opening act, rising tension, midpoint reversal, dark night of the soul, climax, resolution, central stakes, thematic undercurrent. |
| `Global Material/Lore.md` | World-building reference: setting atmosphere, history, social structures, world rules, sensory details the prose should reflect. |
| `Chapters material/Chapter_01.md` | Detailed scene beats for Chapter 1: what happens, in what order, what the reader should feel, what is established, what is withheld. |

Use this markdown structure:

```
Soul.md          →  # Soul\\n\\n## Genre & Tone\\n\\n...\\n\\n## Narrator & Perspective\\n\\n...\\n
Characters.md    →  # Characters\\n\\n## Protagonist\\n\\n...\\n\\n## Antagonist / Obstacle\\n\\n...\\n
Outline.md       →  # Outline\\n\\n## Opening\\n\\n...\\n\\n## Midpoint\\n\\n...\\n\\n## Ending\\n\\n...\\n
Lore.md          →  # Lore\\n\\n## Setting\\n\\n...\\n
Chapter_01.md    →  # Chapter 1\\n\\n## Beats\\n\\n...\\n
```

Then commit and push:

```bash
git -C <repo-path> add -A
git -C <repo-path> commit -m \"init: populate global material from author Q&A\"
git -C <repo-path> push origin main
```

Stop. Notify the author the book is ready — they can review `Global Material/` in their editor and start the first writing session when satisfied.
";

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
    pub hint: &'static str,
    pub target_file: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<&'static str>>,
}

/// Suggested (target_pages, session_pages) defaults for each book type.
fn suggested_defaults(book_type: &str) -> (u32, u32) {
    match book_type {
        "Flash fiction" => (5, 2),
        "Short story" => (20, 3),
        _ => (250, 6), // Novel
    }
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
        return Err(anyhow!(
            "repository already initialized — Global Material/Config.yml exists"
        ));
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
    write_file("AGENTS.md", AGENTS_MD, &mut files_created)?;
    write_file("Changelog/.gitkeep", "", &mut files_created)?;
    write_file(
        "Current version/Full_Book.md",
        "<!-- ⚠ INK-GATEWAY:MANAGED — Do not edit this file directly.\n\
         Human edits belong in Review/current.md.\n\
         Validated content is appended automatically after each session.\n\
         Use `ink-cli rollback` to undo the last session. -->\n",
        &mut files_created,
    )?;
    write_file(
        ".ink-state.yml",
        "current_chapter: 1\ncurrent_chapter_word_count: 0\n",
        &mut files_created,
    )?;

    git_commit_and_push(repo_path)?;

    let questions = vec![
        // ── Language ──────────────────────────────────────────────────────────
        Question {
            question: "What language should the engine write in?",
            hint: "e.g. English, French, Spanish, German — use the full language name",
            target_file: "Global Material/Config.yml",
            options: None,
        },
        // ── Book Format ───────────────────────────────────────────────────────
        Question {
            question: "What type of book are you writing?",
            hint: "Flash fiction: ~1–5 pages · Short story: ~5–30 pages · Novel: ~150–400 pages",
            target_file: "Global Material/Config.yml",
            options: Some(vec!["Flash fiction", "Short story", "Novel"]),
        },
        Question {
            question: "How many pages should the finished book be?",
            hint: "Flash fiction: 5 · Short story: 20 · Novel: 250 — each page ≈ 250 words",
            target_file: "Global Material/Config.yml",
            options: None,
        },
        Question {
            question: "How many pages should the engine write per session?",
            hint: "Flash fiction: 2 · Short story: 3 · Novel: 6 — one session runs on schedule",
            target_file: "Global Material/Config.yml",
            options: None,
        },
        // ── Voice & Style ──────────────────────────────────────────────────────
        Question {
            question: "What is the genre and overall tone?",
            hint: "e.g. Dark fantasy with literary prose, melancholic and immersive",
            target_file: "Global Material/Soul.md",
            options: None,
        },
        Question {
            question: "What is the narrator perspective and tense?",
            hint: "e.g. Third-person limited, past tense, close to the protagonist",
            target_file: "Global Material/Soul.md",
            options: None,
        },
        // ── Characters ─────────────────────────────────────────────────────────
        Question {
            question: "Who is the protagonist? Give a name and one defining trait.",
            hint: "e.g. Mara, a disgraced soldier haunted by a massacre she survived",
            target_file: "Global Material/Characters.md",
            options: None,
        },
        Question {
            question: "Who or what is the main antagonist or obstacle?",
            hint: "e.g. The Conclave, a religious order that controls all magic",
            target_file: "Global Material/Characters.md",
            options: None,
        },
        // ── Plot Arc ───────────────────────────────────────────────────────────
        Question {
            question: "How does the story open? What kicks it off?",
            hint: "1-2 sentences — the inciting event that sets everything in motion",
            target_file: "Global Material/Outline.md",
            options: None,
        },
        Question {
            question: "What is the midpoint turning point?",
            hint: "1-2 sentences — the moment that changes everything for the protagonist",
            target_file: "Global Material/Outline.md",
            options: None,
        },
        Question {
            question: "How does the story end?",
            hint: "1-2 sentences — the resolution and what the protagonist gains or loses",
            target_file: "Global Material/Outline.md",
            options: None,
        },
        // ── World & Setting ────────────────────────────────────────────────────
        Question {
            question: "Describe the world and setting.",
            hint: "e.g. A crumbling empire on the edge of a magical desert, post-industrial era",
            target_file: "Global Material/Lore.md",
            options: None,
        },
        // ── Chapter 1 ──────────────────────────────────────────────────────────
        Question {
            question: "What happens in Chapter 1? What should the reader feel by the end?",
            hint: "Key scene(s) and the emotional note the chapter closes on",
            target_file: "Chapters material/Chapter_01.md",
            options: None,
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

// ─── seed ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SeedPayload {
    pub status: &'static str,
    pub files_created: Vec<String>,
}

/// Write CLAUDE.md and GEMINI.md into `repo_path` so that any AI agent launched
/// in an empty repo knows to run `ink-cli init --agent` before the first session.
/// Idempotent: safe to re-run; overwrites existing files.
pub fn run_seed(repo_path: &Path) -> Result<SeedPayload> {
    let mut files_created: Vec<String> = Vec::new();

    for name in &["CLAUDE.md", "GEMINI.md"] {
        let path = repo_path.join(name);
        fs::write(&path, SEED_CONTENT).with_context(|| format!("Failed to write {}", name))?;
        files_created.push(name.to_string());
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

    run(&["add", "CLAUDE.md", "GEMINI.md"])?;
    run(&[
        "commit",
        "-m",
        "chore: add agent bootstrap files (CLAUDE.md, GEMINI.md)",
    ])?;

    let push = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;
    if !push.success() {
        tracing::warn!("git push skipped — no remote configured");
    }

    Ok(SeedPayload {
        status: "seeded",
        files_created,
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

    println!();
    println!(
        "  ⚠  Reset will permanently delete all book content in «{}».",
        repo_name
    );
    println!("  The git history is preserved, but all files will be removed.");
    println!("  You can re-run `ink-cli init` afterwards to start fresh.");
    println!();

    let input = Text::new(&format!("Type «{}» to confirm", repo_name))
        .prompt()
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
            "rm",
            "-rf",
            "--ignore-unmatch",
            "Global Material/",
            "Chapters material/",
            "Review/",
            "Changelog/",
            "Current version/",
            "AGENTS.md",
            "COMPLETE",
            ".ink-running",
            ".ink-kill",
            ".ink-state.yml",
        ])
        .current_dir(repo_path)
        .status();

    // Re-create .gitkeep placeholders so the directories exist for the next init
    for dir in &[
        "Changelog",
        "Chapters material",
        "Review",
        "Current version",
    ] {
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
    run(&[
        "commit",
        "-m",
        "reset: wipe book content for re-initialization",
    ])?;

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
        tracing::warn!(
            "git push origin main failed — no remote configured or push rejected; skipping"
        );
    }

    Ok(())
}

// ─── Interactive Q&A (TTY mode) ───────────────────────────────────────────────

/// Run when `init` is called from a real terminal. Asks 10 focused questions
/// using inline prompts, shows a summary, and commits on confirmation.
pub fn run_interactive_qa(repo_path: &Path, payload: &InitPayload) -> Result<()> {
    // (start_index, section_label)
    let sections: &[(usize, &str)] = &[
        (0, "Language"),
        (1, "Book Format"),
        (4, "Voice & Style"),
        (6, "Characters"),
        (8, "Plot Arc"),
        (11, "World & Setting"),
        (12, "Chapter 1"),
    ];

    println!();
    println!("  Ink Gateway — Book Setup");
    println!("  «{}» by {}", payload.title, payload.author);
    println!("  13 questions — about 5 minutes.");
    println!();

    let mut answers: Vec<(usize, String)> = Vec::new();

    for (i, q) in payload.questions.iter().enumerate() {
        // Print section header when a new section begins
        if let Some((_, name)) = sections.iter().find(|(start, _)| *start == i) {
            if i > 0 {
                println!();
            }
            println!(
                "  ── {} {}",
                name,
                "─".repeat(48_usize.saturating_sub(name.len()))
            );
        }

        let answer = if let Some(ref options) = q.options {
            // Select prompt (Q1: book type)
            match Select::new(q.question, options.clone()).prompt() {
                Ok(a) => a.to_string(),
                Err(inquire::InquireError::OperationCanceled)
                | Err(inquire::InquireError::OperationInterrupted) => {
                    println!("\n  Setup cancelled. No files were changed.");
                    return Ok(());
                }
                Err(e) => anyhow::bail!("Input error on question {}: {}", i + 1, e),
            }
        } else if i == 2 || i == 3 {
            // Text with a computed default based on book type (Q1)
            let book_type = answers
                .iter()
                .find(|(idx, _)| *idx == 1)
                .map(|(_, a)| a.as_str())
                .unwrap_or("Novel");
            let (default_pages, default_session) = suggested_defaults(book_type);
            let default_val = if i == 2 {
                default_pages
            } else {
                default_session
            };
            let default_str = default_val.to_string();
            let words = default_val * 250;
            let dynamic_hint = if i == 2 {
                format!("Suggested for {}: {} pages (~{} words) — press Enter to accept or type another number.", book_type, default_val, words)
            } else {
                format!("Suggested for {}: {} pages/session (~{} words) — press Enter to accept or type another number.", book_type, default_val, words)
            };
            match Text::new(q.question)
                .with_default(&default_str)
                .with_help_message(&dynamic_hint)
                .prompt()
            {
                Ok(a) => a,
                Err(inquire::InquireError::OperationCanceled)
                | Err(inquire::InquireError::OperationInterrupted) => {
                    println!("\n  Setup cancelled. No files were changed.");
                    return Ok(());
                }
                Err(e) => anyhow::bail!("Input error on question {}: {}", i + 1, e),
            }
        } else {
            match Text::new(q.question).with_help_message(q.hint).prompt() {
                Ok(a) => a,
                Err(inquire::InquireError::OperationCanceled)
                | Err(inquire::InquireError::OperationInterrupted) => {
                    println!("\n  Setup cancelled. No files were changed.");
                    return Ok(());
                }
                Err(e) => anyhow::bail!("Input error on question {}: {}", i + 1, e),
            }
        };

        answers.push((i, answer));
    }

    // Summary review
    println!();
    println!("  ── Review ───────────────────────────────────────────────────────");
    for (i, answer) in &answers {
        let q = &payload.questions[*i];
        let display = if answer.trim().is_empty() {
            "(skipped)"
        } else {
            answer.trim()
        };
        println!("  {}. {}:", i + 1, q.question);
        println!("     {}", display);
    }
    // Show derived Config.yml values
    let target_pages = answers
        .iter()
        .find(|(i, _)| *i == 2)
        .and_then(|(_, a)| a.trim().parse::<u32>().ok());
    let session_pages = answers
        .iter()
        .find(|(i, _)| *i == 3)
        .and_then(|(_, a)| a.trim().parse::<u32>().ok());
    if let (Some(tp), Some(sp)) = (target_pages, session_pages) {
        let target_words = tp * 250;
        let session_words = sp * 250;
        let chapters = ((target_words + 2999) / 3000).max(1);
        println!();
        println!(
            "  Config: {} pages → {} words, {} chapters, {} words/session",
            tp, target_words, chapters, session_words
        );
    }
    println!();

    let confirmed = match Confirm::new("Commit these answers and prepare the book?")
        .with_default(true)
        .prompt()
    {
        Ok(b) => b,
        Err(inquire::InquireError::OperationCanceled)
        | Err(inquire::InquireError::OperationInterrupted) => {
            println!("\n  Cancelled. No files were changed.");
            return Ok(());
        }
        Err(e) => anyhow::bail!("Confirmation error: {}", e),
    };

    if !confirmed {
        println!("\n  Cancelled. Run init again to start over.");
        return Ok(());
    }

    write_answers_to_files(repo_path, &answers)?;
    commit_qa_answers(repo_path)?;

    println!();
    println!("  Book is ready.");
    println!("  Review Global Material/ in your editor, then start the first writing session.");
    println!();

    Ok(())
}

/// Aggregate answers (by question index) and write them as structured markdown
/// to their respective target files. Multiple answers targeting the same file
/// are combined under section headings.
fn write_answers_to_files(repo_path: &Path, answers: &[(usize, String)]) -> Result<()> {
    let map: HashMap<usize, &str> = answers.iter().map(|(i, a)| (*i, a.as_str())).collect();

    // Config.yml — language (q0), target pages (q2), session pages (q3); chapter_count derived
    {
        let path = repo_path.join("Global Material/Config.yml");
        let content = fs::read_to_string(&path).with_context(|| "Failed to read Config.yml")?;
        let lang = map.get(&0).copied().unwrap_or("").trim().to_string();
        let target_pages = map
            .get(&2)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let session_pages = map
            .get(&3)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let target_words = target_pages * 250;
        let session_words = session_pages * 250;
        let chapter_count = ((target_words + 2999) / 3000).max(1);
        let updated = content
            .lines()
            .map(|line| {
                if line.starts_with("language:") && !lang.is_empty() {
                    format!("language: {}", lang)
                } else if line.starts_with("target_length:") && target_pages > 0 {
                    format!("target_length: {}", target_words)
                } else if line.starts_with("words_per_session:") && session_pages > 0 {
                    format!("words_per_session: {}", session_words)
                } else if line.starts_with("chapter_count:") && target_pages > 0 {
                    format!("chapter_count: {}", chapter_count)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{}\n", updated)).with_context(|| "Failed to write Config.yml")?;
    }

    // Soul.md — genre/tone (q4) + narrator/perspective (q5)
    {
        let genre = map.get(&4).copied().unwrap_or("").trim().to_string();
        let narrator = map.get(&5).copied().unwrap_or("").trim().to_string();
        if !genre.is_empty() || !narrator.is_empty() {
            let mut content = String::from("# Soul\n");
            if !genre.is_empty() {
                content.push_str("\n## Genre & Tone\n\n");
                content.push_str(&genre);
                content.push('\n');
            }
            if !narrator.is_empty() {
                content.push_str("\n## Narrator & Perspective\n\n");
                content.push_str(&narrator);
                content.push('\n');
            }
            fs::write(repo_path.join("Global Material/Soul.md"), content)
                .with_context(|| "Failed to write Soul.md")?;
        }
    }

    // Characters.md — protagonist (q6) + antagonist (q7)
    {
        let protag = map.get(&6).copied().unwrap_or("").trim().to_string();
        let antag = map.get(&7).copied().unwrap_or("").trim().to_string();
        if !protag.is_empty() || !antag.is_empty() {
            let mut content = String::from("# Characters\n");
            if !protag.is_empty() {
                content.push_str("\n## Protagonist\n\n");
                content.push_str(&protag);
                content.push('\n');
            }
            if !antag.is_empty() {
                content.push_str("\n## Antagonist / Obstacle\n\n");
                content.push_str(&antag);
                content.push('\n');
            }
            fs::write(repo_path.join("Global Material/Characters.md"), content)
                .with_context(|| "Failed to write Characters.md")?;
        }
    }

    // Outline.md — opening (q8) + midpoint (q9) + ending (q10)
    {
        let opening = map.get(&8).copied().unwrap_or("").trim().to_string();
        let midpoint = map.get(&9).copied().unwrap_or("").trim().to_string();
        let ending = map.get(&10).copied().unwrap_or("").trim().to_string();
        if !opening.is_empty() || !midpoint.is_empty() || !ending.is_empty() {
            let mut content = String::from("# Outline\n");
            if !opening.is_empty() {
                content.push_str("\n## Opening\n\n");
                content.push_str(&opening);
                content.push('\n');
            }
            if !midpoint.is_empty() {
                content.push_str("\n## Midpoint\n\n");
                content.push_str(&midpoint);
                content.push('\n');
            }
            if !ending.is_empty() {
                content.push_str("\n## Ending\n\n");
                content.push_str(&ending);
                content.push('\n');
            }
            fs::write(repo_path.join("Global Material/Outline.md"), content)
                .with_context(|| "Failed to write Outline.md")?;
        }
    }

    // Lore.md — world/setting (q11)
    if let Some(&setting) = map.get(&11) {
        let setting = setting.trim();
        if !setting.is_empty() {
            let content = format!("# Lore\n\n## Setting\n\n{}\n", setting);
            fs::write(repo_path.join("Global Material/Lore.md"), content)
                .with_context(|| "Failed to write Lore.md")?;
        }
    }

    // Chapter_01.md — beats (q12)
    if let Some(&beats) = map.get(&12) {
        let beats = beats.trim();
        if !beats.is_empty() {
            let content = format!("# Chapter 1\n\n## Beats\n\n{}\n", beats);
            fs::write(repo_path.join("Chapters material/Chapter_01.md"), content)
                .with_context(|| "Failed to write Chapter_01.md")?;
        }
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
    run(&[
        "commit",
        "-m",
        "init: populate global material from author Q&A",
    ])?;

    let push = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;
    if !push.success() {
        tracing::warn!("git push skipped — no remote configured");
    }

    Ok(())
}

// ─── update-agents ────────────────────────────────────────────────────────────

/// Overwrite `AGENTS.md` (and `CLAUDE.md`/`GEMINI.md` if present) with the
/// latest versions embedded in this build. Commits and pushes. Idempotent.
pub fn update_agents(repo_path: &Path) -> Result<serde_json::Value> {
    let mut files_updated: Vec<String> = Vec::new();

    // Always refresh AGENTS.md — it's the primary purpose of this command.
    fs::write(repo_path.join("AGENTS.md"), AGENTS_MD)
        .with_context(|| "Failed to write AGENTS.md")?;
    files_updated.push("AGENTS.md".to_string());

    // Refresh CLAUDE.md and GEMINI.md only if they already exist (seed files).
    for name in &["CLAUDE.md", "GEMINI.md"] {
        let path = repo_path.join(name);
        if path.exists() {
            fs::write(&path, SEED_CONTENT).with_context(|| format!("Failed to write {name}"))?;
            files_updated.push(name.to_string());
        }
    }

    // Stage the updated files.
    let mut args = vec!["add"];
    let file_refs: Vec<&str> = files_updated.iter().map(String::as_str).collect();
    args.extend_from_slice(&file_refs);
    let status = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .status()?;
    if !status.success() {
        anyhow::bail!("git add failed");
    }

    // Skip commit if nothing actually changed.
    let diff_empty = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(repo_path)
        .status()?;

    if diff_empty.success() {
        return Ok(serde_json::json!({
            "status": "up_to_date",
            "files_updated": [],
        }));
    }

    let commit = Command::new("git")
        .args([
            "commit",
            "-m",
            "chore: update agent files to latest ink-gateway version",
        ])
        .current_dir(repo_path)
        .status()?;
    if !commit.success() {
        anyhow::bail!("git commit failed");
    }

    let push = Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(repo_path)
        .status()?;
    if !push.success() {
        tracing::warn!("git push skipped — no remote configured");
    }

    Ok(serde_json::json!({
        "status": "updated",
        "files_updated": files_updated,
    }))
}
