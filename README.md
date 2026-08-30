<p align="center">
  <img src="logo.svg" alt="Ink Gateway" width="120"/>
</p>

<p align="center">
  <a href="https://github.com/Philippe-arnd/Ink-Gateway/releases/latest"><img src="https://img.shields.io/github/v/release/Philippe-arnd/Ink-Gateway" alt="Latest Release"/></a>
  <a href="https://github.com/Philippe-arnd/Ink-Gateway/actions/workflows/ci.yml"><img src="https://github.com/Philippe-arnd/Ink-Gateway/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"/></a>
</p>

# Ink Gateway

> A git-native engine for AI-assisted fiction writing. Every book is a git working copy; every edit — human or AI — is a commit. `ink-cli` scaffolds and maintains books; [Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App) is where the actual writing happens.

---

## 🧭 How It Works

`ink-cli`'s shared engine (`ink_core`, a Rust library) is depended on directly
by the [Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App)
web editor — no subprocess shelling, no separate database for book content.
Versioning, comments, and chat history all live in git:

| Concern | How |
|---|---|
| Prose | A git working copy per book, edited live in the web app |
| Versioning | Every edit is a commit; restore = a new forward commit from an old blob |
| Comments | `Comments/current.yml`, git-tracked |
| Scaffolding & maintenance | This repo's `ink-cli` — `init`, `advance-chapter`, `apply-format`, `status`, `doctor` |

`ink-gateway-mcp` exposes the same maintenance commands as native MCP tools,
for driving a book from Claude Code, Gemini CLI, or any other MCP client
instead of (or alongside) the web app.

---

## 📁 Per-Book Repository Structure

```
/Global Material/
  Soul.md              # Narrator voice, tone, prose style
  Outline.md           # Full plot arc and story goal
  Characters.md        # Character profiles and arcs
  Lore.md              # World-building and rules
  Summary.md           # Append-only narrative log
  Config.yml           # language, target_length, chapter_count, chapter_structure,
                       # words_per_session, summary_context_entries,
                       # words_per_chapter (advance-chapter threshold, default 3000),
                       # words_per_page (pagination in Full_Book.md, default 250)

/Chapters material/    # Chapter outlines only — no prose
.ink-state.yml         # current_chapter, current_chapter_word_count — committed to
                       # git, updated by `advance-chapter`; never edit manually
/Review/
  current.md           # The live, editable draft — what the web app's editor and
                       # AI co-author read and write
/Comments/
  current.yml          # Highlight-and-comment threads anchored to current.md
/Changelog/
  YYYY-MM-DD-HH-MM.md  # Historical session logs (from the pre-web-app engine)
/Current version/
  Full_Book.md         # Validated prose, if populated by an earlier writing pass.
                       # Nothing currently writes to this automatically — see
                       # "Full_Book.md" below.
COMPLETE               # Present if the book was sealed by the pre-web-app engine
```

### Full_Book.md

Earlier versions of this framework had an autonomous scheduled agent
(`session-open` → write → `session-close`) that validated prose out of
`current.md` into `Full_Book.md` automatically. That flow has been removed —
writing now happens live in the web app, directly against `current.md`, with
git commits as the versioning story. `Full_Book.md` is no longer written to
automatically; `ink-cli apply-format` can still patch an existing one
(headings, front matter), and books written before this change keep their
history. A replacement "publish/export" step is a known gap, not yet built.

---

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

The MCP server exposes `advance_chapter`, `apply_format`, `init`, `seed`,
`status`, `update_agents`, and `doctor` as native tools — no shell wrappers
needed.

---

## 🛠️ CLI Reference

| Command | Description |
|---|---|
| `ink-cli seed <repo>` | 🌱 Bootstrap for AI agents — write `CLAUDE.md` + `GEMINI.md` so any AI CLI auto-detects and runs `init` |
| `ink-cli init <repo>` | 📖 Scaffold a new book — interactive Q&A in TTY, JSON payload for agents (`--agent` forces JSON in TTY) |
| `ink-cli advance-chapter <repo>` | 📑 Advance to next chapter — update `.ink-state.yml`, commit (no push) |
| `ink-cli apply-format <repo>` | 🎨 Patch `Full_Book.md` structure (title, author, chapter headings) via JSON on stdin — commits + pushes |
| `ink-cli reset <repo>` | 🗑️ Wipe all content — allows re-running `init` (confirmation required) |
| `ink-cli status <repo>` | 📊 Read-only snapshot — chapter, word counts, lock status, completion flags |
| `ink-cli update-agents <repo>` | 🔄 Refresh `AGENTS.md` (and seed files) from the latest embedded template |
| `ink-cli doctor <repo>` | 🩺 Validate repo structure, config, git remote, and session lock state |

---

## 👤 For Human Authors

**1. Create a GitHub repo** (or a local bare repo) and clone it locally:

```bash
git clone https://github.com/<github-username>/<book-repo> /path/to/book
```

**2. Scaffold the book** (interactive Q&A — title, genre, characters, etc.):

```bash
ink-cli init /path/to/book
```

**3. Write it** — open `/path/to/book` in
[Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App), or edit
`Review/current.md` directly and drive the AI co-author from your own MCP
client via `ink-gateway-mcp`.

**Undo/start over:**
- `ink-cli reset` followed by `ink-cli init` wipes and re-scaffolds.
- Versioning and restore live in the web app, backed by `ink_core`'s
  `git::list_versions`/`restore_version` (git commit history, forward-only).

---

## 🚦 Implementation Status

| Phase | Status | Description |
|---|---|---|
| **Phase 1** | ✅ Complete | `init`, `seed`, scaffolding, interactive TUI |
| **Phase 2** | ✅ Complete | `ink_core` extracted as a library — `edit`, `comments`, `git::list_versions`/`restore_version`, `context::get_book_context` — consumed directly by [Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App) |
| **Phase 3** | ✅ Complete | `ink-gateway-mcp` — native MCP server for Claude Code and Gemini CLI |
| ~~Autonomous scheduled sessions~~ | ❌ Removed | `session-open`/`session-close`/`complete`/`rollback` and the cron-driven `ink-engine` loop — superseded by the web app's live, human-reviewed writing sessions |
