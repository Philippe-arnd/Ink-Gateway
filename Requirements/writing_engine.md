# Writing Engine Brain - Technical Specification

This document serves as the technical mandate for the OpenClaw `ink-engine` sub-agent running the Ink Gateway engine.

## 1. Environment & Permissions
The engine operates on the **Shared Volume** (`/data/ink-gateway/books/<book-name>/`), which is the Git working tree for the book repository. This same volume is mounted by SilverBullet.

- **Clone Location:** The book repo is checked out directly onto the shared volume at `/data/ink-gateway/books/<book-name>/`.
- **Git Auth:** The `ink-engine` agent uses configured SSH keys to push/pull from GitHub.
- **SilverBullet:** Purely file-based; no Git awareness. All Git operations are owned by `engine.py`.

## 2. Context Assembly (The Input)
For every session, the engine must load:
- **Permanent Context:** All files in `/Global Material/`.
- **Structural Context:** All files in `/Chapters material/` (to detect human edits by timestamp).
- **Active Context:** `/Review/current.md` — the rolling ~5-page context window (~1500 words).
- **Progress Anchoring:** Continuation point is determined from both:
    - The content of `/Review/current.md` (where prose last left off).
    - The last entry in `Summary.md` (narrative state checkpoint).

### Change Detection (Smart Sync)
- Check modification timestamps of all files on the shared volume.
- **Logic:** Any file with a modification date matching the current calendar day is a "Human Override."
- **Action:** Commit all modified files to `main` immediately before any AI generation:
  `git add . && git commit -m 'chore: human updates' && git push origin main`

## 3. Core Logic & Prioritization
1. **Human Writer Override:**
    - Detect and commit any files modified today to `main`.
    - Adapt the session plan based on detected changes (e.g. if a chapter outline changed, re-evaluate that chapter's direction).
2. **Instruction Processing:**
    - Scan `/Review/current.md` for `<!-- Claw: [Instruction] -->` comments.
    - **Action:** Replace targeted text + delete the comment.
3. **Narrative Generation:**
    - Anchor to the continuation point from `/Review/current.md` + `Summary.md`.
    - Reference `Outline.md` and the relevant `/Chapters material/` file.
    - Generate `nightly_output_target` words (from `Config.yml`; default ~1500 words / ~5 pages).
    - Style must strictly adhere to `/Global Material/Style_guide.md`.

**Implicit Approval:** No explicit approval from the human author is required. If no files were modified today, the previous draft is treated as accepted and the engine continues writing.

## 4. Maintenance & Sync Tasks (The Output)
After generating prose, the engine MUST execute in this order:

1. **Update `/Review/current.md`:** Overwrite with the new ~5 pages. This becomes the context window for the next session.
2. **Append to `Summary.md`:** Add a delta summary paragraph covering only the new session's events. Do not rewrite or truncate existing entries.
3. **Write Changelog entry:** Append a structured entry to `/Changelog/[YYYY-MM-DD].md` containing:
    - Date header
    - AI word count generated
    - List of human edits detected (filenames + one-line description of change)
    - One-paragraph narrative summary of the session
4. **Compile Manuscript:** Read the previous `Full_Book_[YYYY-MM-DD].md`, append the new pages, write a new `Full_Book_[YYYY-MM-DD].md` (today's date). This file is the persistent source of truth for all prose.
5. **Completion Check:** If `Outline.md` arcs are fulfilled and total word count is within ±10% of `target_length`:
    - Write a `COMPLETE` file to the repo root.
    - Call `openclaw cron delete <job-id>` to disable the nightly schedule.
6. **Sync to GitHub:**
    - `git push origin main` (human updates + all maintenance files).
    - `git push origin draft` (AI-generated prose).

## 5. Automation (The Cron Job)
- **Platform:** OpenClaw cron scheduler (`--session isolated`).
- **Schedule:** 02:00 UTC nightly.
- **Registration command (one per book):**
  ```bash
  openclaw cron add \
    --name "Ink: <Book Title>" \
    --cron "0 2 * * *" \
    --session isolated \
    --agent "ink-engine" \
    --thinking high \
    --message "Process book: https://github.com/Philippe-arnd/<book-repo>"
  ```
- **Repo URL delivery:** The `ink-engine` agent parses the GitHub repo URL from its incoming cron message and passes it to `engine.py` as an argument.
- **Pre-run check:** Before any generation, verify if files have been modified today. If yes: commit to `main` first.
