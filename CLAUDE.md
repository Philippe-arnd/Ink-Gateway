# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Ink Gateway** is a git-native engine for AI-assisted fiction writing. This
repository is `ink_core` (a Rust library) plus two thin binaries over it:
`ink-cli` (scaffolding/maintenance commands) and `ink-gateway-mcp` (the same
commands as native MCP tools). Each book is its own git repository.

**Writing itself happens in
[Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App)**, a
separate web app that depends on `ink_core` directly (git dependency, no
subprocess shelling) and adds the live editor, comments, versioning UI, and
the AI co-author's tool-use loop. This repo no longer runs an autonomous
scheduled writing agent — that flow (`session-open`/`session-close`/
`complete`/`rollback`, an `ink-engine` cron job) was removed once the web
app's live, human-reviewed sessions superseded it.

## Architecture

```
src/
  lib.rs          ← declares the ink_core library's modules
  main.rs         ← ink-cli entry point: clap router
  mcp_server.rs   ← ink-gateway-mcp entry point: JSON-RPC 2.0 over stdio
  init.rs         ← seed + init + reset subcommands; inquire TUI; scaffold + Q&A
  git.rs          ← git plumbing: run_git, create_snapshot_tag, list_versions,
                    restore_version
  edit.rs         ← insert_text / rewrite_range — character-offset live edits
                    to Review/current.md, used directly by the web app's editor
                    and AI co-author
  comments.rs     ← highlight-and-comment threads (Comments/current.yml)
  context.rs      ← get_book_context (read-only snapshot for the web app),
                    load_global_material, load_chapter, load_word_count
  maintenance.rs  ← advance-chapter, status, doctor, README helpers
  book.rs         ← count_prose_words, apply_format_patch
  config.rs       ← Config.yml parsing (serde_yaml)
  state.rs        ← .ink-state.yml parsing (current_chapter, current_chapter_word_count)
templates/        ← seed files embedded via include_str! (Soul, Outline, Characters, Lore, etc.)
```

`main.rs` and `mcp_server.rs` are thin — both depend on the `ink_core`
library target (see `Cargo.toml`'s `[lib]` section) rather than duplicating
logic. Adding a capability both binaries need means adding it to a module
under `src/` and re-exporting it from `lib.rs`, not writing it twice.

## Per-Book Repository Structure

```
/Global Material/      ← All permanent context: Soul, Outline, Characters, Lore, Summary, Config.yml
/Chapters material/    ← Chapter outlines ONLY (no prose)
.ink-state.yml         ← current_chapter, current_chapter_word_count — committed to
                          git, updated by advance-chapter; never edit manually
/Review/
  current.md           ← The live, editable draft. What the web app's TipTap
                          editor and AI co-author read/write via edit.rs.
/Comments/
  current.yml          ← Highlight-and-comment threads, git-tracked
/Changelog/
  YYYY-MM-DD-HH-MM.md  ← Historical session logs (from the removed cron engine;
                          nothing writes new entries here anymore)
/Current version/
  Full_Book.md         ← Validated/paginated prose, if a book has one from
                          before the cron engine was removed. Nothing currently
                          writes to this automatically — see README.md.
COMPLETE                ← Present if a book was sealed by the removed cron engine
```

## `ink-cli` / `ink-gateway-mcp` Commands

| Command | Responsibility | Output |
|---|---|---|
| `seed <repo-path>` | Write `CLAUDE.md` + `GEMINI.md` to bootstrap agent-driven init on an empty repo; commit + push. Idempotent. | JSON: `status`, `files_created` |
| `init <repo-path>` | Scaffold dirs + seed files + commit; TTY: interactive Q&A; TTY + `--agent` or non-TTY: JSON with `questions` array | JSON: `status`, `files_created`, `questions` |
| `advance-chapter <repo-path>` | Advance to next chapter: check next chapter outline exists (`needs_chapter_outline` if missing), update `.ink-state.yml`, commit. Does NOT push. | JSON: `{ "status": "advanced", "new_chapter", "chapter_file", "chapter_content" }` or `needs_chapter_outline` / `chapter_not_ready` / `error` |
| `reset <repo-path>` | Wipe all book content; confirmation required | Console |
| `status <repo-path>` | Read-only snapshot: chapter, word counts, lock status, completion flags. No git ops. | JSON |
| `apply-format <repo-path>` | Apply format patches to `Full_Book.md` (stdin: JSON with `prepend` and `insert_headings`). Commits + pushes. | JSON: `{ "status": "applied", "patches_applied": N, "warnings": [...] }` |
| `update-agents <repo-path>` | Overwrite `AGENTS.md` (and `CLAUDE.md`/`GEMINI.md` if present) from latest embedded template; commit + push. | JSON |
| `doctor <repo-path>` | Validate repo structure, config, git remote, session lock state | JSON: list of named checks |

## What the Web App Calls Directly (not via CLI subcommand)

These are library functions in `ink_core`, called by
[Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App)'s Axum
API as a git dependency — no CLI subcommand exists for them because there's
no need to shell out when you're already in Rust:

- `edit::insert_text`, `edit::rewrite_range` — character-offset edits to `Review/current.md`
- `comments::add_comment`, `comments::resolve_comment`, `comments::list_comments`
- `git::list_versions`, `git::restore_version`, `git::create_snapshot_tag`
- `context::get_book_context` — read-only snapshot (Global Material + current chapter + `current.md` + comments + word count), zero git mutation

## Key Crates

| Crate | Purpose |
|---|---|
| `clap` | Subcommand CLI (`derive` feature) |
| `serde` + `serde_yaml` | Parse `Config.yml`, `Comments/current.yml` |
| `serde_json` | Structured JSON output for all subcommands |
| `chrono` | Date-stamped tags, filenames |
| `regex` | Legacy `<!-- INK: ... -->` marker parsing (book.rs) |
| `anyhow` | Ergonomic error propagation |
| `inquire` | Interactive TTY prompts for `init`/`reset` confirmations |
| `tracing` + `tracing-subscriber` | Structured logging |

## Testing

`cargo test` covers `ink_core`'s modules directly (unit tests colocated with
each module). Git-touching functions (`edit.rs`, `comments.rs`, `git.rs`) use
`tempfile` + a real local `git init` in the temp dir — no mocking, no
network (all operations are local commits, no push).
