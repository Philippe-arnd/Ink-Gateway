# Project: The Ink Gateway - Collaborative AI Book Writing Framework

## 1. Overview
A collaborative AI-driven writing framework hosted on a private VPS. It enables human-AI collaboration on Science Fiction and Fantasy novels through a "Reusable Workflow" model. The framework handles multi-book management, automated nocturnal writing sessions, and bidirectional Git synchronization.

## 2. Technical Stack & Architecture
- **Infrastructure:** Self-hosted on OVH VPS via Coolify.
- **Framework Model:** "The Ink Gateway" acts as the engine/workflow. Each book has its own dedicated GitHub repository.
- **Editor:** [SilverBullet](https://silverbullet.md/) (Docker-based) at `write.philapps.com`. Uses the built-in Git library (`git.autoSync`) to commit and push human edits to GitHub automatically. No shared volume with OpenClaw — GitHub is the transport.
- **Versioning:** GitHub Repositories (one per book). All Git operations are owned by `engine.py` (OpenClaw side) and SilverBullet's git library (editor side).
- **Automation:** [OpenClaw](https://docs.openclaw.ai/) — commercial AI agent gateway. One cron job per book, each configured with the book's GitHub repo URL passed as the agent message. OpenClaw uses its existing GitHub token and `gh` CLI for all git operations.
- **Agent:** `ink-engine` — a named OpenClaw agent with workspace at `/data/ink-gateway`. Its `AGENTS.md` contains the writing engine system prompt (see Phase 3 in roadmap).

## 3. Repository Structure (Per Book)
Each book repository follows this folder structure. The GitHub repository is the single source of truth. SilverBullet clones it into its own container space; the `ink-engine` agent clones it into `/data/ink-gateway/books/<book-name>/`.

- **`/Global Material/`**: Core reference files. Loaded as permanent context every session.
    - `Lore.md`: World-building, rules, and history.
    - `Characters.md`: Detailed character profiles and arcs.
    - `Outline.md`: Global plot arc and structure. Completion is detected when all arcs here are fulfilled.
    - `Style_guide.md`: Voice, tone, and specific prose instructions.
    - `Config.yml`: Book-specific parameters — `model`, `target_length`, `chapter_count`, `chapter_structure`, `nightly_output_target`.
    - `Summary.md`: Append-only narrative summary. After each session the engine appends a delta summary of the new ~5 pages. Also used as a progress anchor alongside `/Review/current.md`.
- **`/Chapters material/`**: Skeletal structure — outlines only, no prose.
    - One `.md` file per chapter with plot goals and status. Human edits here trigger AI re-evaluation of the specific chapter.
- **`/Review/`**: The active work zone.
    - `current.md`: Single rolling file containing the last ~5 pages (~1500 words). This is the engine's active context window. Overwritten each session with the new output.
- **`/Changelog/`**: Daily structured logs.
    - One entry per session: date header, AI word count generated, list of human edits detected, brief narrative summary.
- **`/Current version/`**: The compiled manuscript and persistent source of truth for all prose.
    - `Full_Book_[YYYY-MM-DD].md`: The engine reads the previous session's file, appends the new ~5 pages, and writes a new dated file.

## 4. Operational Workflow

### 4.1 Nightly Engine Session Sequence
1. **Pre-flight (Human Override Detection):** Check file timestamps on the shared volume. Any file modified during the current calendar day → `git add . && git commit -m 'chore: human updates' && git push origin main`. Engine adapts its plan based on detected changes.
2. **Snapshot:** `git tag pre-nightly-$(date +%F)`.
3. **Branch:** `git checkout draft && git rebase main`.
4. **Context Loading:** Read all files in `/Global Material/`, `/Chapters material/`, and `/Review/current.md`.
5. **Progress Anchoring:** Determine continuation point from `/Review/current.md` content + last entry in `Summary.md`.
6. **Instruction Processing:** Scan `/Review/current.md` for `<!-- Claw: [Instruction] -->` comments → rewrite targeted text → delete comment.
7. **Narrative Generation:** Generate `nightly_output_target` words adhering to `Style_guide.md`.
8. **Maintenance (in order):** Overwrite `/Review/current.md` with new pages → append delta to `Summary.md` → write structured `/Changelog/` entry → append to `Full_Book_[YYYY-MM-DD].md`.
9. **Completion Check:** If `Outline.md` arcs are fulfilled and total word count is within ±10% buffer → write `COMPLETE` marker to repo root → call `openclaw cron delete <job-id>`.
10. **Sync:** `git push origin main` (human updates + maintenance) and `git push origin draft` (AI prose).

**Implicit approval model:** No explicit approval from the human author is required. Human edits are the signal — they are committed to `main` before generation begins. If no edits are made, the engine treats the previous draft as accepted and continues writing.

**Human authoring flow:** Edit `/Review/current.md` or `/Chapters material/` files in SilverBullet. Insert `<!-- Claw: [Instruction] -->` anywhere in `current.md` to trigger a targeted rewrite on the next nightly run. No merge action required.

### 4.2 Completion & Buffer Logic
- **Standard:** 1 page = 300 words.
- **Buffer:** +/- 10% on chapter and total book length to ensure narrative closure.
- **Trigger:** Completion is reached when `Outline.md` arcs are fulfilled and total word count is within the buffer.

## 5. Multi-Book Management
- Each book has its own GitHub repository and its own OpenClaw cron job.
- Cron jobs are registered manually: `openclaw cron add --session isolated --agent ink-engine --thinking high --cron "0 2 * * *" --name "Ink: <Title>" --message "Process book: <github-repo-url>"`.
- The `ink-engine` agent parses the repo URL from its incoming message and passes it to `engine.py`.
- No central book registry is needed — book registration = cron job creation.

## 6. Security & Reliability
- **Git Snapshots:** Mandatory `pre-nightly-YYYY-MM-DD` tags before every session.
- **Priority:** Human writer's changes always override AI content. Human-modified files are committed to `main` before any AI generation begins.
- **Completion:** Engine auto-disables its own cron job and writes a `COMPLETE` marker when the book is finished.
