# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Ink Gateway** is a collaborative AI-driven fiction writing framework. It orchestrates writing sessions between a human author and an AI agent (`ink-engine`) for books and novels. This repository is the **framework definition** — `ink-cli` (the Rust binary) lives here. Each book has its own separate GitHub repository.

## Architecture

Three components synced through GitHub on a self-hosted VPS:

| Component | Role |
|---|---|
| **Markdown editor** | Human author's browser-based editor. Auto-commits and pushes edits to GitHub. No Git knowledge required. |
| **GitHub** | Single source of truth and sync layer between editor and engine. |
| **Agent gateway `ink-engine` agent** | Triggered on schedule, pulls from GitHub, generates prose, pushes all Git operations back. |

The **agent gateway** is a self-hosted AI agent platform. The `ink-engine` is a named agent with workspace at `/data/ink-gateway`. Each book gets its own isolated cron job.

`ink-cli` is invoked via two shell tool definitions declared inline in `AGENTS.md` — no separate skill layer.

## Per-Book Repository Structure

```
/Global Material/      ← All permanent context; loaded every session
  Soul.md              ← Narrator voice, tone, prose style
  Outline.md           ← Full plot arc and story goal
  Characters.md        ← Character profiles, arcs, consistency reference
  Lore.md              ← World-building and rules
  Summary.md           ← Append-only delta log; last summary_context_entries
                          paragraphs loaded per session
  Config.yml           ← language, target_length, chapter_count, chapter_structure,
                          words_per_session, summary_context_entries, current_chapter
                          (model is set at the agent gateway level, not here)

/Chapters material/    ← Chapter outlines ONLY (no prose).
                          Only current_chapter and current_chapter+1 are loaded per session.
/Review/
  current.md           ← Rolling context window (words_per_session words).
                          Overwritten each session.
/Changelog/
  YYYY-MM-DD.md        ← Date, word count, human edits detected, narrative summary
/Current version/
  Full_Book.md         ← Single source of truth for all prose. Engine appends each session.
                          Git history + ink-YYYY-MM-DD-HH-MM tags provide versioning.
COMPLETE               ← Written by engine when book is finished (triggers cron self-deletion)
```

## Engine Session (the core loop)

1. **Open:** `session-open` → git-setup (pre-flight commit, snapshot tag, draft branch) + read-context (all Global Material, current+next chapter, current.md with INK instructions extracted) → full JSON payload
2. **Concurrency check:** If `session_already_run` is `true` (`.ink-running` lock exists) → stop. Multiple sessions per day are supported; each gets an `ink-YYYY-MM-DD-HH-MM` tag.
3. **Plan:** Read `human_edits` and `current.instructions` from payload; adapt session
4. **Generate:** `words_per_session` words adhering to `Soul.md` and `Outline.md`
5. **Close:** `session-close` (prose via stdin) → write `current.md`, append `Summary.md`, write `Changelog/`, append `Full_Book.md`, push `main` + `draft`
6. **Complete (conditional):** If `completion_ready` AND arcs fulfilled → `complete` → write `COMPLETE`, final push, cron deleted

**Implicit approval:** `human_edits` is empty = previous draft accepted; engine continues writing.

**Instruction syntax:** `<!-- INK: [Instruction] -->` in `current.md` — extracted by `session-open` into a typed array.

**Chapter advancement:** Author increments `current_chapter` in `Config.yml` to advance to the next chapter.

## Agent Cron Registration (one per book)

```bash
agent-gateway cron add \
  --name "Ink: <Book Title>" \
  --cron "<cron-schedule>" \
  --session isolated \
  --agent "ink-engine" \
  --model <model-id> \
  --thinking high \
  --message "Process book: https://github.com/<github-username>/<book-repo>"
```

The `--model` flag (or equivalent in your gateway) is the only place the AI model is configured — it is not stored in the book repo. All AI credentials are managed by the agent gateway.

## Implementation Language & Key Files

- **`ink-cli`** — Rust binary. Four subcommands: `init`, `session-open`, `session-close`, `complete`.
- **`Cargo.toml`** — dependency manifest.
- **`ink-engine` AGENTS.md** (Phase 3) — Writing engine system prompt + inline tool definitions.

### `ink-cli` Subcommands

| Subcommand | Phase | Responsibility | Output |
|---|---|---|---|
| `init <repo-path>` | Setup | Scaffold all dirs + seed files → commit + push; present 6 Q&A questions | JSON: `status`, `files_created`, `questions` |
| `session-open <repo-path>` | Start | git-setup + read-context → full payload | JSON payload |
| `session-close <repo-path>` | End | stdin prose → write + maintain + push | JSON: word counts + `completion_ready` |
| `complete <repo-path>` | Finish | Write `COMPLETE` + final push | JSON: `{ "status": "complete" }` |

### Source Layout

```
src/
  main.rs          ← clap router + top-level error handling
  init.rs          ← init subcommand: scaffold dirs, write seed files, git commit
  git.rs           ← git operations (pre-flight, snapshot, branch, push)
  context.rs       ← context aggregation, INK instruction extraction, JSON output
  maintenance.rs   ← summary / changelog / Full_Book compiler
  config.rs        ← Config.yml parsing (serde_yaml)
templates/         ← seed files embedded via include_str! (Soul, Outline, Characters, Lore, etc.)
Cargo.toml
```

### Key Crates

| Crate | Purpose |
|---|---|
| `clap` | Subcommand CLI (`derive` feature) |
| `serde` + `serde_yaml` | Parse `Config.yml` |
| `serde_json` | Structured JSON output for all subcommands |
| `chrono` | Date-stamped tags, filenames, changelog entries |
| `walkdir` | Directory traversal for `Global Material/` |
| `regex` | Extract `<!-- INK: ... -->` instruction comments |
| `anyhow` | Ergonomic error propagation |
| `tracing` + `tracing-subscriber` | Structured logging |

## Implementation Roadmap Summary

- **Phase 1:** ✅ Editor git sync, agent registration, `session-open` subcommand
- **Phase 2:** ✅ `session-close` + `complete` subcommands, `init` subcommand + templates, cron registration
- **Phase 3:** `ink-engine` AGENTS.md with inline tool definitions
- **Phase 4:** Static site, validation layer

See `Requirements/roadmap.md` for detailed task checklists.
