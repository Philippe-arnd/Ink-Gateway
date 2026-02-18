# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Ink Gateway** is a collaborative AI-driven fiction writing framework. It orchestrates nightly writing sessions between a human author and an AI agent (`ink-engine`) for Science Fiction and Fantasy novels. This repository is the **framework definition** — `engine.py` lives here. Each book has its own separate GitHub repository.

The project is currently in the **specification phase**. All authoritative documentation is in `Requirements/`. No implementation code exists yet.

## Architecture

Three components connected via a shared Docker volume on a self-hosted OVH VPS (managed via Coolify):

| Component | Role |
|---|---|
| **SilverBullet** (`write.philapps.com`) | Human author's markdown editor. Purely file-based — zero Git awareness. |
| **Shared Volume** (`/data/ink-gateway/books/`) | The Git working tree for each book repo. Both containers mount the same host directory. |
| **OpenClaw `ink-engine` agent** | Runs nightly, reads the volume, generates prose, handles all Git operations. |

**OpenClaw** ([docs.openclaw.ai](https://docs.openclaw.ai)) is a commercial self-hosted AI agent gateway. The `ink-engine` is a named agent registered with `openclaw agents add ink-engine --workspace /data/ink-gateway`. Each book gets its own isolated OpenClaw cron job — no central book registry needed.

## Per-Book Repository Structure

Each book repo is checked out at `/data/ink-gateway/books/<book-name>/`:

```
/Global Material/      ← Permanent AI context (loaded every session)
  Config.yml           ← model, target_length, chapter_count, chapter_structure, nightly_output_target
  Outline.md           ← Plot arc; completion is detected when all arcs here are fulfilled
  Summary.md           ← Append-only delta log; also used as a progress anchor
  Lore.md, Characters.md, Style_guide.md

/Chapters material/    ← Chapter outlines ONLY (no prose). Human edits here trigger AI re-evaluation.
/Review/
  current.md           ← Rolling ~5-page context window. Overwritten each session with new output.
/Changelog/
  YYYY-MM-DD.md        ← Structured: date, word count, human edits detected, narrative summary
/Current version/
  Full_Book_[date].md  ← Source of truth for ALL prose. Engine appends new pages each session.
COMPLETE               ← Written by engine when book is finished (triggers cron self-deletion)
```

## Nightly Engine Session (the core loop)

1. **Pre-flight:** Detect files modified today by timestamp → `git add . && git commit -m 'chore: human updates' && git push origin main`
2. **Snapshot:** `git tag pre-nightly-$(date +%F)`
3. **Branch:** `git checkout draft && git rebase main`
4. **Context load:** `/Global Material/` + `/Chapters material/` + `/Review/current.md`
5. **Anchor:** Determine continuation from `current.md` content + last `Summary.md` entry
6. **Instructions:** Process `<!-- Claw: [Instruction] -->` comments in `current.md` → rewrite + delete
7. **Generate:** `nightly_output_target` words following `Style_guide.md`
8. **Maintain (in order):** Overwrite `current.md` → append delta to `Summary.md` → write `/Changelog/[date].md` → append to `Full_Book_[date].md`
9. **Completion check:** Arcs fulfilled + word count within ±10% → write `COMPLETE` → `openclaw cron delete <job-id>`
10. **Sync:** `git push origin main` then `git push origin draft`

**Implicit approval:** Human edits = signal. No edit = previous draft accepted. Engine always continues writing.

## OpenClaw Cron Registration (one per book)

```bash
openclaw cron add \
  --name "Ink: <Book Title>" \
  --cron "0 2 * * *" \
  --session isolated \
  --agent "ink-engine" \
  --thinking high \
  --message "Process book: https://github.com/Philippe-arnd/<book-repo>"
```

The `ink-engine` agent parses the repo URL from the message and passes it to `engine.py` as an argument.

## Implementation Language & Key Files

- **`engine.py`** (to be built, lives in this repo) — Python orchestrator. Accepts a GitHub repo URL argument. Handles all Git operations, context reading, and calls the AI model.
- **`ink-engine` AGENTS.md** (Phase 3) — The writing engine system prompt. Lives in the agent's workspace on the VPS. See `Requirements/writing_engine.md` for the full technical mandate.

## Implementation Roadmap Summary

- **Phase 1:** Shared volume wiring, `ink-engine` agent setup, `engine.py` scaffold (Git ops + payload builder)
- **Phase 2:** Full nightly automation (cron, change detection, rolling `current.md`, append summary, manuscript compiler, completion handler)
- **Phase 3:** Author the `ink-engine` `AGENTS.md` system prompt
- **Phase 4:** Static site for `books.philapps.com`, validation layer

See `Requirements/roadmap.md` for detailed task checklists.
