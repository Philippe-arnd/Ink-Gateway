<p align="center">
  <img src="logo.svg" alt="Ink Gateway" width="120"/>
</p>

# Ink Gateway

> A collaborative AI-driven framework for writing books and novels. The engine writes autonomously on any schedule; the human author edits either in the IDE or in a browser-based markdown editor — no Git knowledge required.

---

## 🧭 How It Works

Three components sync through GitHub:

| Component | Role |
|---|---|
| ✏️ **Editor** | IDE or browser-based editor. Auto-commits and pushes every save to GitHub. |
| 🐙 **GitHub** | Single source of truth. Sync layer between editor and engine. |
| 🤖 **`ink` agent** | Triggered on schedule. Pulls, reads context, writes prose, pushes everything back. |

Each book is an **independent GitHub repository**. The editor syncs human edits throughout the day; the ink agent picks them up at session time, generates new prose, and pushes back.

**Implicit approval:** if the text produced by the engine is not modified by the author, the draft (`current.md`) is accepted at the next session and the engine continues writing. Human edits are the only signal needed.

---

## 📁 Per-Book Repository Structure

```
/Global Material/
  Soul.md              # Narrator voice, tone, prose style
  Outline.md           # Full plot arc and story goal
  Characters.md        # Character profiles and arcs
  Lore.md              # World-building and rules
  Summary.md           # Append-only session log (last N paragraphs in context)
  Config.yml           # language, target_length, chapter_count, chapter_structure,
                       # words_per_session, summary_context_entries,
                       # words_per_chapter (chapter close threshold, default 3000),
                       # words_per_page (pagination in Full_Book.md, default 250),
                       # current_review_window_words (payload cap, default 0 = unlimited)

/Chapters material/    # Chapter outlines only — no prose
                       # current chapter + next (only when chapter close is near)
.ink-state.yml         # Engine-managed state: current_chapter, current_chapter_word_count
                       # Committed to git — never edit manually
/Review/
  current.md           # Rolling prose window. The engine reads and rewrites this each
                       # session. Author adds <!-- INK: --> instructions here.
/Changelog/
  YYYY-MM-DD-HH-MM.md # Word count, human edits, narrative summary per session
/Current version/
  Full_Book.md         # Validated prose only. Auto-managed — do not edit directly.
                       # Includes <!-- PAGE N --> pagination markers.
                       # Git history + ink-* tags = versioning + rollback points.
COMPLETE               # Written by engine when book is finished
```

### The `current.md` / `Full_Book.md` split

| File | Role | Who touches it |
|---|---|---|
| `current.md` | Rolling prose window — last session's output + author instructions | The ink agent reads and rewrites it every session. The author adds `<!-- INK: -->` comments to direct the engine. |
| `Full_Book.md` | Vault of all validated prose — auto-managed, read-only for the author | Engine appends at each `session-close`; **never edit manually** |

**Split rule:** everything in `current.md` **before** the first `<!-- INK: [instruction] -->` tag is validated. `session-close` automatically moves it to `Full_Book.md`. The engine then rewrites `current.md` from that split point onwards, marking reworked and new sections with diff markers.

```
current.md after a session:

  [clean prose — validated next session]

  <!-- INK: make this paragraph more tense -->   ← author instruction

  <!-- INK:REWORKED:START -->
  ...engine's rewrite of the instructed passage...
  <!-- INK:REWORKED:END -->

  <!-- INK:NEW:START -->
  ...new continuation prose from this session...
  <!-- INK:NEW:END -->
```

---

## 🖥️ Installation

Install both binaries with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | bash
```

This installs `ink-cli` and `ink-gateway-mcp` to `~/.local/bin`.

### MCP integration (Claude Code / Gemini CLI)

Once installed, register the MCP server so your AI client can call the tools natively:

```bash
# Claude Code
claude mcp add ink-gateway -- ~/.local/bin/ink-gateway-mcp

# Gemini CLI
gemini mcp add ink-gateway -- ~/.local/bin/ink-gateway-mcp
```

The MCP server exposes `session_open`, `session_close`, `complete`, `advance_chapter`, `init`, and `seed` as native tools — no shell wrappers needed.

---

## 🛠️ CLI Reference

### Command overview

| Command | Description |
|---|---|
| `ink-cli seed <repo>` | 🌱 Bootstrap for AI agents — write `CLAUDE.md` + `GEMINI.md` so any AI CLI auto-detects and runs `init` |
| `ink-cli init <repo>` | 📖 Scaffold a new book — interactive Q&A in TTY, JSON payload for agents (`--agent` forces JSON in TTY) |
| `ink-cli session-open <repo>` | 🔓 Start a writing session — sync, detect edits, load context |
| `ink-cli session-close <repo>` | 🔒 End a writing session — split current.md, update Full_Book, push |
| `ink-cli complete <repo>` | 🏁 Mark book as finished — write `COMPLETE` marker, final push |
| `ink-cli advance-chapter <repo>` | 📑 Advance to next chapter — update `.ink-state.yml`, commit (no push) |
| `ink-cli reset <repo>` | 🗑️ Wipe all content — allows re-running `init` (confirmation required) |
| `ink-cli rollback <repo>` | ⏪ Revert to before the last session — force-push (confirmation required) |

---

### 🌱 `seed` — Bootstrap agent files

```bash
ink-cli seed <repo-path>
```

Run once on an empty repo **before** launching any AI agent. Creates `CLAUDE.md` and `GEMINI.md` at the repo root with self-contained instructions that guide any AI CLI (Claude Code, Gemini CLI, etc.) through the full initialization flow — no manual steps beyond this command.

```bash
# Typical flow
git clone https://github.com/<user>/<book-repo> /path/to/book
ink-cli seed /path/to/book    # ← one command, then launch any AI CLI
claude                         # agent reads CLAUDE.md, runs init --agent, asks questions, extrapolates, commits
```

`seed` is idempotent — safe to re-run to refresh the bootstrap files. It does **not** initialize the book; that is left to the agent after you answer the 10 questions interactively in the AI chat.

---

### 📖 `init` — Scaffold a new book

```bash
ink-cli init <repo-path> --title "<Book Title>" --author "<Author Name>"
```

Run once per book in an existing git repository. Creates all directories, seed files, and `AGENTS.md`, then commits.

| Mode | Behaviour |
|---|---|
| **TTY** (human at terminal) | 10 inline questions grouped by section (Language / Voice / Characters / Plot / World / Chapter 1). Shows a review summary, asks confirmation, commits and pushes — book ready in one shot. |
| **TTY + `--agent`** | Same as non-TTY: outputs JSON payload without launching prompts. Use this to hand the questions off to an AI model in your IDE. |
| **Non-TTY** (agent / pipe) | Outputs JSON with `status`, `files_created`, and a `questions` array (each with `question`, `hint`, `target_file`). The agent presents questions, **extrapolates** answers into rich Global Material, writes files, commits. |

---

### 🔓 `session-open` — Start a writing session

```bash
ink-cli session-open <repo-path>
```

Fetches from origin, detects and commits local human edits (including uncommitted IDE saves), merges origin, creates a snapshot tag `ink-YYYY-MM-DD-HH-MM`, acquires the session lock, and loads all context. Outputs a full JSON payload.

**Check these abort fields before generating:**

| Field | Meaning |
|---|---|
| `kill_requested: true` | Author created `.ink-kill` — stop immediately |
| `session_already_run: true` | Active lock exists — another session is running |
| `stale_lock_recovered: true` | Crashed lock cleaned up — proceed normally |

---

### 🔒 `session-close` — End a writing session

```bash
echo "$prose" | ink-cli session-close <repo-path> \
  --summary "One-paragraph narrative summary" \
  --human-edit "Chapters material/Chapter_03.md"   # repeatable
```

Reads new prose from stdin (the engine's new `current.md` content), then:
1. Extracts the **validated section** from old `current.md` (everything before the first `<!-- INK: -->` instruction), strips engine markers, and appends it to `Full_Book.md` with `<!-- PAGE N -->` pagination markers
2. Overwrites `Review/current.md` with the new prose from stdin
3. Updates `current_chapter_word_count` in `.ink-state.yml`
4. Appends to `Summary.md`, writes a `Changelog/` entry, releases the lock, pushes `main` + `draft`

```json
{
  "session_word_count": 1487,
  "total_word_count": 43210,
  "target_length": 90000,
  "current_chapter_word_count": 2341,
  "completion_ready": false,
  "status": "closed"
}
```

`total_word_count` reflects **validated prose only** (words in `Full_Book.md`, excluding comment markers).

---

### 🏁 `complete` — Mark book as finished

```bash
ink-cli complete <repo-path>
```

Writes the `COMPLETE` marker, commits, and pushes. Call only when `completion_ready` is `true` **and** all narrative arcs are genuinely fulfilled.

---

### 🗑️ `reset` — Wipe all book content

```bash
ink-cli reset <repo-path>
```

Removes all book content (Global Material, Chapters, Review, Changelog, Full_Book.md, AGENTS.md). Preserves directory structure and git history. User must **type the repository name** to confirm. After reset, `init` can be run again.

---

### ⏪ `rollback` — Revert to before the last session

```bash
ink-cli rollback <repo-path>
```

Finds the most recent `ink-*` snapshot tag (created at `session-open` time), hard-resets `main` and `draft` to that state, and force-pushes both branches. The last session's prose, Summary.md entry, and Changelog entry are all removed. Requires **y/n confirmation**.

---

## 👤 For Human Authors

### Starting a new book

**1. Create a GitHub repo** and clone it locally:

```bash
git clone https://github.com/<github-username>/<book-repo> /path/to/book
```

**2a. If you want an AI agent to handle the setup** (recommended — richer Global Material):

```bash
ink-cli seed /path/to/book   # creates CLAUDE.md + GEMINI.md, commits, pushes
claude                        # or: gemini  — agent reads the file, runs init, asks you the 10 questions
```

**2b. If you prefer to answer the questions yourself at the terminal:**

```bash
ink-cli init /path/to/book --title "My Book" --author "Jane Doe"
```

The CLI asks 10 short questions grouped by section — language, voice & style, characters, plot arc, world, and Chapter 1 beats. All inline, no editor needed. Everything is committed automatically.

**3. Register the cron job** on your agent gateway:

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

The `--model` flag is the only place the AI model is configured — it is not stored in the book repo.

### Day-to-day authoring

- ✏️ **Edit** any file in your markdown editor — changes auto-commit and push.
- 💬 **Direct the engine** by adding `<!-- INK: [your instruction] -->` anywhere in `current.md`. Everything before this marker is treated as validated and moved to `Full_Book.md`. The engine rewrites from this point onwards.
- ✅ **Validate silently** by not adding any INK instructions — the engine treats the entire `current.md` as approved and appends it to `Full_Book.md`.
- 📑 **Chapter advancement is automatic** — the engine calls `advance-chapter` when the chapter word count reaches 90% of `words_per_chapter`. No manual action needed.
- ⏪ **Undo a bad session** with `ink-cli rollback`.
- 🔄 **Start over** with `ink-cli reset` followed by `ink-cli init`.

---

## 💰 API Cost Reference

| Model | Per 200-page book |
|---|---|
| Gemini Flash | ~$0.04 |
| Claude Sonnet | ~$1.93 |
| Claude Opus | ~$6.92 |

---

## 🚦 Implementation Status

| Phase | Status | Description |
|---|---|---|
| **Phase 1** | ✅ Complete | Editor git sync, agent registration, `session-open` |
| **Phase 2** | ✅ Complete | `session-close`, `complete`, `init`, `reset`, `rollback`, `advance-chapter`, interactive TUI, current.md/Full_Book split, pagination, chapter automation |
| **Phase 3** | ✅ Complete | `ink-engine` AGENTS.md — full session flow, chapter advancement, completion discipline, rework loop |
| **Phase 4** | ✅ Complete | `ink-gateway-mcp` — native MCP server for Claude Code and Gemini CLI |
