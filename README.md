<p align="center">
  <img src="logo.svg" alt="Ink Gateway" width="120"/>
</p>

# Ink Gateway

A collaborative AI-driven framework for writing books and novels. The engine runs writing sessions autonomously on any schedule, while the human author edits in a browser-based markdown editor — no Git knowledge required.

## How It Works

Three components sync through GitHub on a self-hosted VPS:

| Component | Role |
|---|---|
| **Markdown editor** | Human author's browser-based editor. Auto-commits and pushes edits to GitHub. |
| **GitHub** | Single source of truth. The sync layer between the editor and the engine. |
| **`ink-engine` agent** | Triggered on schedule. Pulls from GitHub, reads context, generates `words_per_session` words of prose, pushes all Git operations. |

Each book is an independent GitHub repository. The editor auto-syncs human edits throughout the day; the engine pulls them at the scheduled time, generates new content, and pushes back.

**Implicit approval:** If no files were modified today, the previous draft is accepted and the engine continues writing. Human edits are the only signal needed.

## Per-Book Repository Structure

```
/Global Material/
  Soul.md              # Narrator voice, tone, prose style
  Outline.md           # Full plot arc and story goal
  Characters.md        # Character profiles and arcs
  Lore.md              # World-building and rules
  Summary.md           # Append-only delta log; last summary_context_entries paragraphs in context
  Config.yml           # language, target_length, chapter_count, chapter_structure,
                       # words_per_session, summary_context_entries, current_chapter

/Chapters material/    # Chapter outlines only (no prose).
                       # Only current_chapter and next are loaded per session.
/Review/
  current.md           # Rolling context window (words_per_session words). Overwritten each session.
/Changelog/
  YYYY-MM-DD-HH-MM.md        # Date, word count, human edits detected, narrative summary
/Current version/
  Full_Book.md         # Source of truth for all prose. Engine appends each session.
                       # Git history + ink-YYYY-MM-DD-HH-MM tags provide versioning.
COMPLETE               # Written by engine when book is finished (triggers cron self-deletion)
```

## Engine Session

1. **Open** — `session-open`: pre-flight commit of human edits, snapshot tag, draft branch, load all context
2. **Concurrency check** — Abort if `.ink-running` lock exists (concurrent or crashed session). Each session gets its own `ink-YYYY-MM-DD-HH-MM` tag.
3. **Plan** — Read `human_edits` and `<!-- INK: -->` instructions from payload; adapt session
4. **Generate** — `words_per_session` words following `Soul.md` and `Outline.md`
5. **Close** — `session-close` (prose via stdin): write `current.md`, append `Summary.md`, write `Changelog/`, append `Full_Book.md`, push
6. **Complete (conditional)** — If word count within ±10% of target AND arcs fulfilled → write `COMPLETE` → cron self-deleted

## Starting a New Book

**1. Scaffold the repository** — run once in any existing git repo:

```bash
ink-cli init /path/to/book-repo --title "My Book" --author "Jane Doe"
```

This creates the full directory structure, populates all seed files, and commits everything to `main`. The response includes a `questions` array — the agent will ask the author 6 targeted questions (language, voice, plot arc, characters, world, Chapter 1 beats) and write the answers directly into the corresponding files, so the book is fully ready for the first writing session in one shot.

**2. Register the cron job** — one per book on the agent gateway:

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

The `--model` flag (or equivalent) is where the AI model is chosen — not in the book repo. No central book registry — each book is simply a cron job.

## For AI Agents

This section is the entry point for any AI model given a book repository URL.

> **Full engine instructions** are in `AGENTS.md` at the root of every scaffolded book repo. A model cloning a book repo will find everything it needs there, including the install step and complete session flow.

### Step 0 — Verify ink-cli is installed

```bash
ink-cli --version
```

If not found:

```bash
curl -sSfL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | sh
```

### Step 1 — Clone or sync the repository

```bash
# First time seeing this repo:
git clone <repo-url> /data/ink-gateway/books/<repo-name>

# Already cloned:
git -C /data/ink-gateway/books/<repo-name> pull origin main
```

### Step 2 — Determine what to do

Check whether `Global Material/Config.yml` exists in the local clone.

| `Global Material/Config.yml` | Action |
|---|---|
| **Absent** | Repository is empty — run `ink-cli init` (see below) |
| **Present** | Repository is ready — run a writing session (see `AGENTS.md`) |

### First-time setup — `ink-cli init`

Run this exactly once per book, before any writing session:

```bash
ink-cli init /data/ink-gateway/books/<repo-name> \
  --title "<Book Title>" \
  --author "<Author Name>"
```

Derive `--title` from the repository name (replace hyphens and underscores with spaces, apply title case). Derive `--author` from the GitHub username in the URL or from the triggering message if it specifies one.

On success you will receive:

```json
{
  "status": "initialized",
  "title": "Book Title",
  "author": "Author Name",
  "files_created": ["Global Material/Config.yml", "..."],
  "questions": [
    { "question": "What language should the engine write in?", "target_file": "Global Material/Config.yml" },
    { "question": "What is the narrative voice, tone, and prose style?", "target_file": "Global Material/Soul.md" },
    "..."
  ]
}
```

**After init, run the Q&A flow.** Present each question to the author one at a time. Once you have all answers, write them to their `target_file` (for `Config.yml`, update only the `language:` field; for all others, replace the seed content). Then commit and push with message `"init: populate global material from author Q&A"` and stop. The book is ready for its first writing session.

### Ongoing writing — session loop

If `Config.yml` already exists, follow the session flow defined in `AGENTS.md`:

```
session_open → (abort checks) → generate → session_close → (complete if done)
```

### Error handling

If `ink-cli` returns `"status": "error"` or exits with a non-zero code, stop immediately. Report the full error output to the author. Do not retry; do not continue to the next step.

---

## Human Authoring Flow

- Edit any file in the markdown editor. Auto-sync commits and pushes automatically.
- Insert `<!-- INK: [Instruction] -->` anywhere in `current.md` to request a targeted rewrite on the next session.
- Increment `current_chapter` in `Config.yml` when you're satisfied a chapter is done.

## API Cost Reference

| Model | Per 200-page book |
|---|---|
| Gemini Flash | ~$0.04 |
| Claude Sonnet | ~$1.93 |
| Claude Opus | ~$6.92 |

See `Requirements/cost-analysis.md` for the full breakdown.

## Implementation Status

| Phase | Status | Description |
|---|---|---|
| **Phase 1** | ✅ Complete | Editor git sync, agent registration, `session-open` subcommand |
| **Phase 2** | ✅ Complete | `session-close` + `complete`, `init` + book templates, full session automation |
| **Phase 3** | Planned | `ink-engine` AGENTS.md with inline tool definitions |
| **Phase 4** | Planned | Static site, validation layer |

See `Requirements/roadmap.md` for detailed task checklists and `Requirements/framework.md` for full architecture documentation.
