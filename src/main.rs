mod config;
mod context;
mod git;

use anyhow::Result;
use clap::{Parser, Subcommand};
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
    /// Close a writing session: write prose, update files, push (stub)
    SessionClose {
        /// Path to the book repository
        repo_path: PathBuf,
    },
    /// Mark book as complete and perform final push (stub)
    Complete {
        /// Path to the book repository
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
        Commands::SessionOpen { repo_path } => {
            run_session_open(&repo_path)?;
        }
        Commands::SessionClose { repo_path: _ } => {
            eprintln!("session-close is not yet implemented");
            std::process::exit(1);
        }
        Commands::Complete { repo_path: _ } => {
            eprintln!("complete is not yet implemented");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_session_open(repo_path: &PathBuf) -> Result<()> {
    let payload = context::session_open(repo_path)?;
    let json = serde_json::to_string_pretty(&payload)?;
    println!("{}", json);
    Ok(())
}
