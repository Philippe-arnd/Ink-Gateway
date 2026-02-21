# Project: The Ink Gateway - Collaborative AI Book Writing Framework

## 1. Overview
A collaborative AI-driven writing framework hosted on a private VPS. It enables human-AI collaboration on books and novels through a "Reusable Workflow" model. The framework handles multi-book management, automated writing sessions, and bidirectional Git synchronization.

## 2. Technical Stack & Architecture
- **Infrastructure:** Self-hosted VPS.
- **Framework Model:** "The Ink Gateway" acts as the engine/workflow. Each book has its own dedicated GitHub repository.
- **Editor:** A browser-based markdown editor with git auto-sync. Auto-commits and pushes human edits to GitHub.
- **Versioning:** GitHub Repositories (one per book). All Git operations are owned by `ink-cli`.
- **Automation:** An agent gateway — a self-hosted AI agent platform. One cron job per book, each configured with the book's GitHub repo URL and the model to use in the agent message.
- **Agent:** `ink-engine` — a named agent with workspace at `/data/ink-gateway`. Its `AGENTS.md` contains the writing engine system prompt and inline tool definitions.
- **Core Binary:** `ink-cli` — a Rust binary handling all file I/O and Git. Called by the agent via two shell tools.

## 3. Repository Structure (Per Book)

```
/Global Material/      ← All permanent context loaded every session
  Soul.md              ← Narrator voice, tone, prose style
  Outline.md           ← Full plot arc and story goal
  Characters.md        ← Character profiles, arcs, consistency reference
  Lore.md              ← World-building and rules
  Summary.md           ← Append-only delta log; last summary_context_entries
                          paragraphs loaded into context per session
  Config.yml           ← target_length, chapter_count, chapter_structure,
                          words_per_session, summary_context_entries, current_chapter

/Chapters material/    ← Chapter outlines ONLY (no prose).
                          Only current_chapter and current_chapter+1 are loaded per session.
                          Human edits trigger AI re-evaluation of that chapter.
/Review/
  current.md           ← Rolling context window (words_per_session words).
                          Overwritten each session with new output.
/Changelog/
  YYYY-MM-DD-HH-MM.md  ← Structured: date, word count, human edits detected, narrative summary
/Current version/
  Full_Book.md         ← Single source of truth for all prose. Engine appends each session.
                          Git history + ink- tags provide versioning.
COMPLETE               ← Written by engine when book is finished (triggers cron self-deletion)
```

## 4. Operational Workflow

### 4.1 Engine Session Sequence
1. **Open:** `session-open` — git-setup (pre-flight commit of human edits, snapshot tag, draft branch) + read-context (load all Global Material, current + next chapter, current.md, extract INK instructions). Returns full JSON payload.
2. **Concurrency check:** If `.ink-running` lock file exists → abort. Another session is in progress or a previous session crashed. Multiple sessions per day are supported — each gets its own `ink-YYYY-MM-DD-HH-MM` snapshot tag.
3. **Plan:** Agent reads `human_edits` and `current.instructions` from the payload. Adapts session plan.
4. **Generate:** Agent produces `words_per_session` words adhering to `Soul.md` and `Outline.md`.
5. **Close:** `session-close` (prose via stdin) — writes `current.md`, appends `Summary.md`, writes `Changelog/` entry, appends `Full_Book.md`, pushes `main` and `draft`.
6. **Complete (conditional):** If `completion_ready` and narrative arcs fulfilled → `complete` — writes `COMPLETE` marker, final push, cron self-deletion.

**Implicit approval:** If no files were modified today (`human_edits` is empty), the previous draft is accepted and the engine continues writing.

**Instruction syntax:** Insert `<!-- INK: [Instruction] -->` anywhere in `current.md`. `session-open` extracts these into a typed array — the agent never scans raw markdown.

**Chapter advancement:** The author increments `current_chapter` in `Config.yml` (via the editor) to signal chapter completion. This is the only manual configuration step during a book's lifetime.

### 4.2 Completion & Buffer Logic
- **Buffer:** ±10% of `target_length` in word count.
- **Trigger:** `completion_ready` flag from `session-close` + agent confirms all `Outline.md` arcs fulfilled.

## 5. Multi-Book Management
- Each book has its own GitHub repository and one cron job in the agent gateway.
- Book registration = cron job creation. No central registry needed.
- `current_chapter` is the only per-session manual setting.

## 6. Security & Reliability
- **Git Snapshots:** Mandatory `ink-YYYY-MM-DD-HH-MM` tags before every session. Supports multiple sessions per day.
- **Idempotency:** Tag existence check at session start prevents double-writes on crash/restart.
- **Human priority:** Human-modified files are always committed to `main` before any generation.
- **Completion:** Engine auto-disables its cron job and writes `COMPLETE` when the book is finished.
