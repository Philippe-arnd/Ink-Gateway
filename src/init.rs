use anyhow::Result;
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

#[derive(Serialize)]
pub struct InitPayload {
    pub status: &'static str,
    pub title: String,
    pub author: String,
    pub files_created: Vec<String>,
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

    // Changelog/.gitkeep — keeps the empty directory tracked by git
    write_file("Changelog/.gitkeep", "", &mut files_created)?;

    // Full_Book.md — starts empty
    write_file("Current version/Full_Book.md", "", &mut files_created)?;

    // Git operations
    git_commit_and_push(repo_path)?;

    Ok(InitPayload {
        status: "initialized",
        title: title.to_string(),
        author: author.to_string(),
        files_created,
    })
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
