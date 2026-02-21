# Agentic Architecture for Ink Gateway

The core logic is a compiled Rust binary (`ink-cli`) that the AI agent calls as tools. This decoupling ensures that file and Git logic remains portable while the creative personality is specific to each book.

## 1. Overview

The architecture is split into two layers:

1. **Core Logic:** A Rust CLI binary (`ink-cli`) — all file operations and Git sync. Produces structured JSON output for the agent to consume.
2. **Creative Brain (`ink-engine` agent):** Uses `ink-cli` subcommands as its tools. Guided by the book's `Global Material/` files and the per-book `Soul.md`.

The skill/tool definitions are two shell commands declared directly in `AGENTS.md` — no separate skill layer document needed.

---

## 2. The Core Layer: `ink-cli`

The binary is the only component that touches the filesystem and Git. The agent never does file I/O directly.

**Binary:** `ink-cli`

| Subcommand | Phase | Responsibility | Output |
|---|---|---|---|
| `session-open <repo-path>` | Start | git-setup + read-context in one shot | Full JSON payload (see schema) |
| `session-close <repo-path>` | End | Reads prose from **stdin** → writes `current.md`, updates `Summary.md`, writes `Changelog/` entry, appends to `Full_Book.md`, pushes | JSON: word counts + `completion_ready` |
| `complete <repo-path>` | Finish | Writes `COMPLETE` marker, final git push | JSON: `{ "status": "complete", "total_word_count": N }` |

**Source layout:**
```
src/
  main.rs          ← clap router + top-level error handling
  git.rs           ← git operations (pre-flight, snapshot, branch, push)
  context.rs       ← context aggregation, INK instruction extraction, JSON output
  maintenance.rs   ← summary / changelog / Full_Book compiler
  config.rs        ← Config.yml parsing (serde_yaml)
```

### `session-open` JSON Schema

```json
{
  "config": {
    "target_length": 90000,
    "chapter_count": 30,
    "chapter_structure": "linear",
    "words_per_session": 1500,
    "summary_context_entries": 5,
    "current_chapter": 3,
    "session_timeout_minutes": 60
  },
  "kill_requested": false,
  "session_already_run": false,
  "stale_lock_recovered": false,
  "global_material": [
    { "filename": "Outline.md",    "content": "..." },
    { "filename": "Summary.md",    "content": "... last summary_context_entries paragraphs only ..." },
    { "filename": "Lore.md",       "content": "..." },
    { "filename": "Characters.md", "content": "..." },
    { "filename": "Soul.md",       "content": "..." }
  ],
  "chapters": {
    "current": { "path": "Chapters material/Chapter_03.md", "content": "...", "modified_today": false },
    "next":    { "path": "Chapters material/Chapter_04.md", "content": "...", "modified_today": false }
  },
  "current": {
    "content": "... previous session prose ...",
    "instructions": [
      { "anchor": "surrounding passage text", "instruction": "rewrite in third person" }
    ]
  },
  "word_count": {
    "total": 43210,
    "target": 90000,
    "remaining": 46790
  },
  "human_edits": ["Chapters material/Chapter_03.md"]
}
```

> **`kill_requested`** — `true` when `.ink-kill` exists in the repo root. The user creates this file via the editor to cancel the next scheduled session. `session-open` deletes `.ink-kill` and any stale lock, then returns immediately with `kill_requested: true` and no other context fields populated. The agent MUST abort without calling any further tools.
>
> **`session_already_run`** — `true` when a `.ink-running` lock file exists AND its timestamp is within `session_timeout_minutes`. Indicates a concurrent session is in progress. The agent MUST abort when this is `true`. The lock file content is an ISO 8601 start timestamp written by `session-open`.
>
> **`stale_lock_recovered`** — `true` when a `.ink-running` lock was found but its timestamp exceeded `session_timeout_minutes`. The lock was auto-removed and the session proceeds normally. The agent should log this for the operator (indicates a previous session was killed or crashed without cleanup).
>
> **`chapters`** — only `current_chapter` and `current_chapter + 1` are loaded. The full chapter list is not sent to the model. The author increments `current_chapter` in `Config.yml` to signal chapter completion.
>
> **`instructions`** — `<!-- INK: [text] -->` comments extracted from `current.md` as a typed array. The agent never scans raw markdown.
>
> **`Summary.md`** — only the last `summary_context_entries` paragraphs are included. The full file is preserved on disk.

### Binary Guardrails

`session-close` and `complete` include mechanical guards against double-execution:

| Subcommand | Guard | Behavior on violation |
|---|---|---|
| `session-close` | Checks `.ink-running` exists before proceeding | `{ "error": "no active session", "status": "error" }`, exit non-zero |
| `complete` | Checks `COMPLETE` does not already exist | `{ "error": "book already complete", "status": "error" }`, exit non-zero |

These fire regardless of agent behavior. Agent-level rules (see `writing_engine.md §6.2`) are the first line of defense; binary guards are the fallback.

### `session-close` JSON Output

```json
{
  "session_word_count": 1487,
  "total_word_count": 43210,
  "target_length": 90000,
  "completion_ready": false,
  "status": "closed"
}
```

---

## 3. The Agent Layer: `ink-engine`

The "Writer". Knows how to write and when to call its tools — never touches files or Git directly.

### Tool Definitions (in AGENTS.md)

Two shell tools, defined inline — no separate skill file needed:

```
Tool: session_open
Shell: ink-cli session-open $repo_path

Tool: session_close
Shell: ink-cli session-close $repo_path
Stdin: generated prose
```

The `complete` subcommand is rarely invoked; it can be defined as a third tool or called directly in the agent's system prompt instructions for the end-of-book case.

### Per-Book Identity

All creative identity lives in `Global Material/` alongside the other permanent context:

| File | Purpose |
|---|---|
| `Soul.md` | Narrator's voice, tone, and prose style — the heart of the book's identity |
| `Outline.md` | Plot arc, structure, and story goal |
| `Characters.md` | Character profiles, arcs, and consistency reference |
| `Lore.md` | World-building, rules, and history |

No separate root-level identity files. Everything the agent needs is in one folder.

---

## 4. Execution Flow

1. **Trigger:** Cron job starts the `ink-engine` agent with the repo path.
2. **Open:** Agent calls `session_open(repo_path)` → receives full context payload.
3. **Idempotency check:** If `session_already_run` is `true` → stop. No generation.
4. **Plan:** Agent reads `human_edits` and `current.instructions` from the payload. Adapts session plan accordingly.
5. **Generate:** Agent produces `words_per_session` words of prose, applying any INK instruction rewrites, adhering to `Soul.md` and `Outline.md`.
6. **Close:** Agent calls `session_close(repo_path)` with prose piped via stdin → `current.md` written, maintenance run, repo pushed. Receives word count response.
7. **Completion (conditional):** If `completion_ready` is `true` AND the agent confirms narrative closure → agent calls `complete(repo_path)` → receives `{ "status": "complete" }` → notifies the user via the gateway's notification channel → signals the gateway to delete this cron job. These are the final actions; no further tool calls are made.

---

## 5. Benefits

- **Single binary deploy:** `cargo build --release` produces one file. No runtime, no venv, no dependency drift.
- **2 tool calls per session:** Minimal LLM turn overhead.
- **Bounded token usage:** Only active chapters loaded; `Summary.md` truncated; no duplicate identity files.
- **Model agnostic:** The AI model is selected at the agent gateway level (cron job or agent registration). Changing model requires no code changes and no edits to the book repo. `ink-cli` is unaware of models entirely.
- **Idempotent:** Tag-based session detection prevents double-writes on crash/restart.
- **Auditable:** Every session produces a `Changelog/` entry and an `ink-YYYY-MM-DD-HH-MM` Git tag. Multiple sessions per day each get their own tag.
