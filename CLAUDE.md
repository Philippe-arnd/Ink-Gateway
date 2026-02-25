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
                          words_per_session, summary_context_entries, words_per_chapter
                          (chapter close threshold, default 3000), words_per_page
                          (pagination, default 250), session_timeout_minutes,
                          current_review_window_words (rolling prose window cap in
                          session-open payload, default 0 = unlimited)
                          (model is set at the agent gateway level, not here)

/Chapters material/    ← Chapter outlines ONLY (no prose).
                          Only current_chapter and current_chapter+1 are loaded per session.
.ink-state.yml         ← Engine-managed state: current_chapter (1-indexed), current_chapter_word_count.
                          Committed to git; never edit manually.
/Review/
  current.md           ← Rolling prose window. Engine rewrites this every session.
                          Author adds <!-- INK: [instruction] --> comments to direct the engine.
                          Everything before the first INK instruction = validated prose.
/Changelog/
  YYYY-MM-DD-HH-MM.md ← Word count, human edits detected, narrative summary per session
/Current version/
  Full_Book.md         ← Validated prose only. Auto-managed, read-only for the author.
                          Includes <!-- PAGE N --> pagination markers (every words_per_page words).
                          Starts with a managed-file header comment.
                          Git history + ink-YYYY-MM-DD-HH-MM tags provide versioning.
COMPLETE               ← Written by engine when book is finished (triggers cron self-deletion)
```

## current.md / Full_Book.md Contract

**`current.md`** is the rolling prose window. The engine reads it, applies INK instructions, and rewrites it each session. The author edits it between sessions (in the markdown editor) to leave instructions.

**`Full_Book.md`** is the vault of validated prose. It is never edited manually — `session-close` appends to it automatically. It includes `<!-- PAGE N -->` markers and a managed-file header.

**Split rule:** at `session-close`, everything in `current.md` **before** the first `<!-- INK: [instruction] -->` tag (note: the space after the colon is the distinguishing feature vs engine markers) is extracted as validated and appended to `Full_Book.md`. The engine's new prose (from stdin) becomes the new `current.md`.

**Engine output format** for `current.md`:
- Reworked passages: `<!-- INK:REWORKED:START -->` ... `<!-- INK:REWORKED:END -->`
- New continuation prose: `<!-- INK:NEW:START -->` ... `<!-- INK:NEW:END -->`
- These markers live only in `current.md`, never in `Full_Book.md` — `session-close` and `complete` strip them automatically before appending validated prose.

**Implicit validation:** if there are no INK instruction comments in `current.md`, the entire file is validated and moved to `Full_Book.md`.

## Engine Session (the core loop)

1. **Open:** `session-open` → git-setup (pre-flight commit, snapshot tag, draft branch) + read-context (all Global Material, current chapter + next chapter if `chapter_close_suggested`, current.md with INK instructions extracted) → full JSON payload
2. **Abort check:** If `session_already_run` is `true` (`.ink-running` lock exists) → stop.
3. **Analyse:** Read `current_review.content` and `current_review.instructions`; identify human edits and INK directives
4. **Consistency check:** Cross-reference plan against `Soul.md`, `Outline.md`, `Characters.md`, `Lore.md`, and active chapter outline
5. **Generate:** Write reworked blocks (one per INK instruction) + new continuation prose (`words_per_session` words)
6. **Close:** `session-close` (prose via stdin) → extract validated section → append to `Full_Book.md` with pagination → overwrite `current.md` → append `Summary.md` → write `Changelog/` → push `main` + `draft`
7. **Complete (loop):** If `completion_ready` AND arcs fulfilled → call `complete`:
   - If `status: "needs_revision"` → run a normal session (`session-open` → rework blocks only, no new prose → `session-close`) → call `complete` again → repeat until clean
   - If `status: "complete"` → book sealed: `current.md` replaced with placeholder, `Full_Book.md` finalized, `COMPLETE` written, pushed, cron deleted

**Instruction syntax:** `<!-- INK: [Instruction] -->` (space after colon) in `current.md` — extracted by `session-open` into a typed array.

**Chapter advancement:** Automated via `advance-chapter`. When `session-open` returns `chapter_close_suggested: true` (chapter word count ≥ 90% of `words_per_chapter`), the engine calls `advance-chapter`. If the next chapter outline is missing, `advance-chapter` returns `needs_chapter_outline` and the engine writes it first, then retries. On success, `.ink-state.yml` is updated with the new chapter number and a reset word count.

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

- **`ink-cli`** — Rust binary. Eight subcommands: `seed`, `init`, `session-open`, `session-close`, `complete`, `advance-chapter`, `reset`, `rollback`.
- **`ink-gateway-mcp`** — MCP server binary. Exposes six tools (`session_open`, `session_close`, `complete`, `advance_chapter`, `init`, `seed`) as native MCP tools over stdio JSON-RPC 2.0. Register with `claude mcp add ink-gateway -- ~/.local/bin/ink-gateway-mcp`.
- **`Cargo.toml`** — dependency manifest. Version format: `YYYY.M.DD-N`. Both binaries are in the same crate.
- **`ink-engine` AGENTS.md** (Phase 3) — Writing engine system prompt + inline tool definitions.

### `ink-cli` Subcommands

| Subcommand | Responsibility | Output |
|---|---|---|
| `seed <repo-path>` | Write `CLAUDE.md` + `GEMINI.md` to bootstrap agent-driven init on an empty repo; commit + push. Idempotent. | JSON: `status`, `files_created` |
| `init <repo-path>` | Scaffold dirs + seed files + commit; TTY: 10-question inquire TUI; TTY + `--agent` or non-TTY: JSON with `questions` array (each has `question`, `hint`, `target_file`) | JSON: `status`, `files_created`, `questions` |
| `session-open <repo-path>` | git-setup + read-context → full payload | JSON payload |
| `session-close <repo-path>` | stdin prose → split current.md → append validated to Full_Book (with pagination) → write new current.md → maintain + push. If engine produced no REWORKED blocks despite pending INK instructions, carries the pending section forward to next session. | JSON: word counts + `completion_ready` |
| `complete <repo-path>` | Check for pending INK instructions in current.md; if found → `needs_revision` JSON; if clean → append current.md to Full_Book.md, write COMPLETE, push | JSON: `{ "status": "needs_revision", "current_review": { "content", "instructions" } }` or `{ "status": "complete", "total_word_count" }` |
| `advance-chapter <repo-path>` | Advance to next chapter: check next chapter file exists (returns `needs_chapter_outline` if missing), update `.ink-state.yml`, commit. Does NOT push. | JSON: `{ "status": "advanced", "new_chapter", "chapter_file", "chapter_content" }` or `{ "status": "needs_chapter_outline", "chapter", "chapter_file" }` or `{ "status": "error", "message" }` |
| `reset <repo-path>` | Wipe all book content; user must type repo name to confirm | Console |
| `rollback <repo-path>` | Hard-reset to most recent ink-* tag + force-push; y/n confirmation | Console |

### Source Layout

```
src/
  main.rs          ← ink-cli entry point: clap router + top-level error handling
  mcp_server.rs    ← ink-gateway-mcp entry point: JSON-RPC 2.0 stdio MCP server
  init.rs          ← seed + init + reset subcommands; inquire TUI; scaffold + Q&A
  git.rs           ← git operations (pre-flight, snapshot, branch, push)
  context.rs       ← context aggregation, INK instruction extraction, JSON output
  maintenance.rs   ← session-close (split/pagination/Full_Book), complete, advance-chapter, rollback
  config.rs        ← Config.yml parsing (serde_yaml)
  state.rs         ← .ink-state.yml parsing (current_chapter, current_chapter_word_count)
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
| `std::fs::read_dir` | Directory traversal for `Global Material/` (stdlib, no extra dep) |
| `regex` | Extract `<!-- INK: ... -->` instruction comments |
| `anyhow` | Ergonomic error propagation |
| `inquire` | Interactive TTY prompts for `init` and `reset`/`rollback` confirmations |
| `tracing` + `tracing-subscriber` | Structured logging |

## Implementation Roadmap Summary

- **Phase 1:** ✅ Editor git sync, agent registration, `session-open` subcommand
- **Phase 2:** ✅ `session-close` + `complete` + `init` + `reset` + `rollback` + `advance-chapter`, interactive TUI, current.md/Full_Book split, pagination, chapter automation
- **Phase 3:** ✅ `ink-engine` AGENTS.md — full session flow, chapter advancement, completion discipline, rework loop

See `Requirements/roadmap.md` for detailed task checklists.
