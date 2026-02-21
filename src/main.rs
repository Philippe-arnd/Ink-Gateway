mod config;
mod context;
mod git;
mod init;
mod maintenance;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::Read;
use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "ink-cli", about = "Ink Gateway CLI for AI-driven fiction writing sessions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a writing session: git sync, load context, output JSON payload
    SessionOpen {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Close a writing session: read prose from stdin, write files, push
    SessionClose {
        /// Path to the book repository
        repo_path: PathBuf,
        /// One-paragraph narrative summary of this session (appended to Summary.md and Changelog)
        #[arg(long)]
        summary: Option<String>,
        /// Human-edited files from the session-open payload (repeatable)
        #[arg(long = "human-edit")]
        human_edits: Vec<String>,
    },
    /// Mark book as complete and perform final push
    Complete {
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
        Commands::SessionOpen { repo_path } => {
            let payload = context::session_open(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        Commands::SessionClose { repo_path, summary, human_edits } => {
            let mut prose = String::new();
            std::io::stdin()
                .read_to_string(&mut prose)
                .context("Failed to read prose from stdin")?;
            let result = maintenance::close_session(
                &repo_path,
                &prose,
                summary.as_deref(),
                &human_edits,
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Complete { repo_path } => {
            let result = maintenance::complete_session(&repo_path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Init { repo_path, title, author } => {
            let result = init::run_init(&repo_path, &title, &author)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
