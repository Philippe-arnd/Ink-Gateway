<p align="center">
  <img src="logo.svg" alt="Ink Gateway" width="120"/>
</p>

# Ink Gateway

> A collaborative AI-driven framework for writing books and novels. The engine writes autonomously on any schedule; the human author edits in a browser-based markdown editor — no Git knowledge required.

---

## 🧭 How It Works

Three components sync through GitHub on a self-hosted VPS:

| Component | Role |
|---|---|
| ✏️ **Markdown editor** | Browser-based editor. Auto-commits and pushes every save to GitHub. |
| 🐙 **GitHub** | Single source of truth. Sync layer between editor and engine. |
| 🤖 **`ink-engine` agent** | Triggered on schedule. Pulls, reads context, writes `words_per_session` words of prose, pushes everything back. |

Each book is an **independent GitHub repository**. The editor syncs human edits throughout the day; the engine picks them up at session time, generates new prose, and pushes back.

**Implicit approval:** if no files were modified since the last session, the previous draft is accepted and the engine continues writing. Human edits are the only signal needed.

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
                       # words_per_session, summary_context_entries, current_chapter

/Chapters material/    # Chapter outlines only — no prose
                       # Only current_chapter and next are loaded per session
/Review/
  current.md           # Rolling context window. Overwritten each session.
/Changelog/
  YYYY-MM-DD-HH-MM.md # Word count, human edits, narrative summary per session
/Current version/
  Full_Book.md         # All prose. Engine appends each session.
                       # Git history + ink-* tags = versioning + rollback points
COMPLETE               # Written by engine when book is finished
```

---

## 🖥️ CLI Reference

Install `ink-cli` on any Linux machine:

```bash
curl -sSfL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | sh
```

### Command overview

| Command | Description |
|---|---|
| `ink-cli init <repo>` | 📖 Scaffold a new book — interactive Q&A in TTY, JSON payload for agents |
| `ink-cli session-open <repo>` | 🔓 Start a writing session — sync, detect edits, load context |
| `ink-cli session-close <repo>` | 🔒 End a writing session — write prose, update files, push |
| `ink-cli complete <repo>` | 🏁 Mark book as finished — write `COMPLETE` marker, final push |
| `ink-cli reset <repo>` | 🗑️ Wipe all content — allows re-running `init` (confirmation required) |
| `ink-cli rollback <repo>` | ⏪ Revert to before the last session — force-push (confirmation required) |

---

### 📖 `init` — Scaffold a new book

```bash
ink-cli init <repo-path> --title "<Book Title>" --author "<Author Name>"
```

Run once per book in an existing git repository. Creates all directories, seed files, and `AGENTS.md`, then commits.

| Mode | Behaviour |
|---|---|
| **TTY** (human at terminal) | Interactive Q&A: language prompt + `$EDITOR` for each question. Writes answers, commits, pushes — book ready in one shot. |
| **Non-TTY** (agent / pipe) | Outputs JSON with `status`, `files_created`, and a `questions` array for the agent to process. |

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

Reads prose from stdin, writes `Review/current.md`, appends `Summary.md` and `Full_Book.md`, writes a `Changelog/` entry, releases the lock, pushes `main` + `draft`.

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

**1. Create a GitHub repo** and clone it locally (or on your VPS):

```bash
git clone https://github.com/<github-username>/<book-repo> /path/to/book
```

**2. Scaffold and configure:**

```bash
ink-cli init /path/to/book --title "My Book" --author "Jane Doe"
```

The CLI will walk you through 6 questions in your `$EDITOR` — language, narrative voice, plot arc, characters, world, and Chapter 1 beats. Everything is committed automatically.

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
- 💬 **Direct the engine** by adding `<!-- INK: [your instruction] -->` anywhere in `current.md`. It will be picked up on the next session.
- 📖 **Advance chapters** by incrementing `current_chapter` in `Config.yml` when you're satisfied a chapter is complete.
- ⏪ **Undo a bad session** with `ink-cli rollback`.
- 🔄 **Start over** with `ink-cli reset` followed by `ink-cli init`.

---

## 🤖 For AI Agents

> Full engine instructions are in `AGENTS.md` at the root of every scaffolded book repo. Everything the engine needs — install step, session flow, guardrails — is there.

### Step 0 — Verify `ink-cli` is installed

```bash
ink-cli --version
```

If not found:

```bash
curl -sSfL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | sh
```

### Step 1 — Clone or sync the repository

```bash
# First time:
git clone <repo-url> /data/ink-gateway/books/<repo-name>

# Already cloned:
git -C /data/ink-gateway/books/<repo-name> pull origin main
```

### Step 2 — Determine what to do

| `Global Material/Config.yml` | Action |
|---|---|
| **Absent** | Run `ink-cli init` — then follow the Q&A flow |
| **Present** | Run a writing session — follow `AGENTS.md` |

### Step 3 — Init (first time only)

```bash
ink-cli init /data/ink-gateway/books/<repo-name> \
  --title "<Book Title>" \
  --author "<Author Name>"
```

Derive `--title` from the repo name (hyphens/underscores → spaces, title case). Derive `--author` from the GitHub username or triggering message.

The JSON response includes a `questions` array. Present each question to the author one at a time. Write answers to their `target_file` (`Config.yml` → update only the `language:` field; all others → replace full content). Commit and push with `"init: populate global material from author Q&A"`. Stop — the book is ready for its first writing session.

### Step 4 — Session loop (every scheduled run)

```
session-open → abort checks → plan → generate → session-close → complete?
```

See `AGENTS.md` in the book repo for the full engine prompt and guardrails.

### Error handling

If `ink-cli` returns `"status": "error"` or exits non-zero — **stop immediately**. Do not retry. Report the full error to the author.

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
| **Phase 2** | ✅ Complete | `session-close`, `complete`, `init`, `reset`, `rollback`, interactive TUI |
| **Phase 3** | 🔲 Planned | `ink-engine` AGENTS.md with inline tool definitions |
| **Phase 4** | 🔲 Planned | Static site, validation layer |
