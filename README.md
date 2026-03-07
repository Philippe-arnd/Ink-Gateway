<p align="center">
  <img src="logo.svg" alt="Ink Gateway" width="120"/>
</p>

<p align="center">
  <a href="https://github.com/Philippe-arnd/Ink-Gateway/releases/latest"><img src="https://img.shields.io/github/v/release/Philippe-arnd/Ink-Gateway" alt="Latest Release"/></a>
  <a href="https://github.com/Philippe-arnd/Ink-Gateway/actions/workflows/ci.yml"><img src="https://github.com/Philippe-arnd/Ink-Gateway/actions/workflows/ci.yml/badge.svg" alt="CI"/></a>
</p>

# Ink Gateway

> A collaborative AI-driven framework for writing books and novels. The engine writes autonomously on any schedule; the human author edits either in the IDE or in a browser-based markdown editor — no Git knowledge required.

---

## 🧭 How It Works

Three components sync through GitHub:

| Component | Role |
|---|---|
| ✏️ **Editor** | IDE or browser-based editor. For the Human writer to review and edit. |
| 🐙 **GitHub** | Single source of truth. Sync layer between editor and engine. |
| 🤖 **`ink` agent** | Triggered on schedule. Pulls, reads context, writes prose, pushes everything back. |

Each book is an **independent GitHub repository**. The editor syncs human edits throughout the day; the ink agent picks them up at session time, generates new prose, and pushes back.

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


## 🖥️ Installation

Install both binaries with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | bash
```

This installs `ink-cli` and `ink-gateway-mcp` to `~/.local/bin`.

### MCP integration 

Once installed, register the MCP server so your AI client can call the tools natively:

```bash
# Claude Code
claude mcp add ink-gateway -- ~/.local/bin/ink-gateway-mcp
```

The MCP server exposes `session_open`, `session_close`, `complete`, `advance_chapter`, `apply_format`, `init`, `seed`, `status`, `update_agents`, and `doctor` as native tools — no shell wrappers needed.

---

## 🛠️ CLI Reference

### Command overview

| Command | Description |
|---|---|
| `ink-cli seed <repo>` | 🌱 Bootstrap for AI agents — write `CLAUDE.md` + `GEMINI.md` so any AI CLI auto-detects and runs `init` |
| `ink-cli init <repo>` | 📖 Scaffold a new book — interactive Q&A in TTY, JSON payload for agents (`--agent` forces JSON in TTY) |
| `ink-cli session-open <repo>` | 🔓 Start a writing session — sync, detect edits, load context |
| `ink-cli session-close <repo>` | 🔒 End a writing session — split current.md, update Full_Book, push |
| `ink-cli complete <repo>` | 🏁 Seal the book — checks pending revisions, format, then writes `COMPLETE` and pushes |
| `ink-cli advance-chapter <repo>` | 📑 Advance to next chapter — update `.ink-state.yml`, commit (no push) |
| `ink-cli apply-format <repo>` | 🎨 Patch `Full_Book.md` structure (title, author, chapter headings) via JSON on stdin — commits + pushes |
| `ink-cli reset <repo>` | 🗑️ Wipe all content — allows re-running `init` (confirmation required) |
| `ink-cli rollback <repo>` | ⏪ Revert to before the last session — force-push (confirmation required) |
| `ink-cli status <repo>` | 📊 Read-only snapshot — chapter, word counts, lock status, completion flags |
| `ink-cli update-agents <repo>` | 🔄 Refresh `AGENTS.md` (and seed files) from the latest embedded template |
| `ink-cli doctor <repo>` | 🩺 Validate repo structure, config, git remote, and session state before first cron run |


---

## 👤 For Human Authors

### Starting a new book

**1. Create a GitHub repo** and clone it locally:

```bash
git clone https://github.com/<github-username>/<book-repo> /path/to/book
```

**2. Scaffold the book** (interactive Q&A — title, genre, characters, etc.):

```bash
ink-cli init /path/to/book
```

**3. Register the writing agent** via your agent gateway (one cron job per book):

```bash
agent-gateway cron add \
  --name "Ink: <Book Title>" \
  --cron "<schedule>" \
  --agent "ink-engine" \
  --model <model-id> \
  --message "Process book: https://github.com/<github-username>/<book-repo>"
```

The engine runs on schedule from here — no further setup needed.

---

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
