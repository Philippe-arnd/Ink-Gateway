use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ink_core::{book, init, maintenance};
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(
    name = "ink-cli",
    version,
    about = "Ink Gateway CLI for AI-driven fiction writing sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Wipe all book content and allow re-running init (requires confirmation)
    Reset {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Scaffold a new book repository with all required files and directories
    Init {
        /// Path to the book repository (must be an existing git repo)
        repo_path: PathBuf,
        /// Book title substituted into all template files
        #[arg(long, default_value = "Untitled")]
        title: String,
        /// Author name substituted into all template files
        #[arg(long, default_value = "Unknown")]
        author: String,
        /// Output JSON questions payload instead of running interactive prompts
        /// (forced automatically when stdout is not a TTY)
        #[arg(long)]
        agent: bool,
    },
    /// Advance to the next chapter, resetting the chapter word count
    AdvanceChapter {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Write CLAUDE.md and GEMINI.md so any AI agent can auto-detect and init an empty repo
    Seed {
        /// Path to the book repository (must be an existing git repo)
        repo_path: PathBuf,
    },
    /// Show current book state: chapter, word counts, lock status, completion
    Status {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Refresh AGENTS.md (and CLAUDE.md/GEMINI.md) from the latest embedded template
    UpdateAgents {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Validate repository structure, config, git remote, and session state
    Doctor {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Apply format patches to Full_Book.md (title, author, chapter headings). Reads JSON patch from stdin.
    ApplyFormat {
        #[arg(value_name = "REPO_PATH")]
        repo_path: PathBuf,
    },
}

fn main() -> Result<()> {
    // Initialize structured logging to stderr with env-filter
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Reset { repo_path } => {
            init::run_reset(&repo_path)?;
        }
        Commands::Init {
            repo_path,
            title,
            author,
            agent,
        } => {
            let result = init::run_init(&repo_path, &title, &author)?;
            let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
            if is_tty && !agent {
                // Human at a terminal without --agent: run interactive Q&A
                init::run_interactive_qa(&repo_path, &result)?;
            } else {
                // Called by agent, piped, or with --agent flag: output JSON payload
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        Commands::AdvanceChapter { repo_path } => {
            let result = maintenance::advance_chapter(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Seed { repo_path } => {
            let result = init::run_seed(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Status { repo_path } => {
            let result = maintenance::book_status(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::UpdateAgents { repo_path } => {
            let result = init::update_agents(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Doctor { repo_path } => {
            let result = maintenance::doctor(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ApplyFormat { repo_path } => {
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                .with_context(|| "Failed to read patch JSON from stdin")?;
            let patch: serde_json::Value =
                serde_json::from_str(&input).with_context(|| "Failed to parse patch JSON")?;
            let result = book::apply_format_patch(&repo_path, patch)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
