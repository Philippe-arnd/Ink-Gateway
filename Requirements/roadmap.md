# Roadmap: Ink Gateway Implementation

This roadmap outlines the technical path for a fully automated multi-book writing engine using a Shared Volume architecture between SilverBullet and OpenClaw.

## Phase 1: Shared Volume Setup & MVP
**Goal:** Create the physical link between the Editor and the Engine.
- [ ] **Volume Definition:** Configure a host directory (`/data/ink-gateway/books`) on the VPS.
- [ ] **SilverBullet Config:** Update Coolify service to mount the host directory.
- [ ] **OpenClaw Config:** Update Coolify/Docker config to mount the same host directory into the `ink-engine` agent's workspace.
- [ ] **Permissions Check:** Ensure both containers can read/write to the shared volume.
- [ ] **`ink-engine` Agent Setup:** Register the agent via `openclaw agents add ink-engine --workspace /data/ink-gateway`.
- [ ] **Book Repository Setup:** Initialize the first book repo; check it out to `/data/ink-gateway/books/<book-name>/`.
- [ ] **`Config.yml` Template:** Define schema with fields: `model`, `target_length`, `chapter_count`, `chapter_structure`, `nightly_output_target`.
- [ ] **Orchestrator Script (`engine.py`):**
    - Accept GitHub repo URL as argument (passed from OpenClaw cron message).
    - **Pre-flight:** Timestamp-based detection of human edits → `git add . && git commit -m 'chore: human updates' && git push origin main`.
    - **Snapshotting:** `git tag pre-nightly-$(date +%F)`.
    - **Branching:** `git checkout draft && git rebase main`.
- [ ] **Payload Builder:** Recursive reader for `/Global Material/`, `/Chapters material/`, `/Review/current.md`.

## Phase 2: V1 - Full Automation
- [ ] **Nightly Cron Job (per book):**
    ```bash
    openclaw cron add \
      --name "Ink: <Book Title>" \
      --cron "0 2 * * *" \
      --session isolated \
      --agent "ink-engine" \
      --thinking high \
      --message "Process book: https://github.com/Philippe-arnd/<book-repo>"
    ```
- [ ] **Intelligent Change Detection:** File timestamp logic to detect human edits and commit them to `main` before generation.
- [ ] **Rolling `/Review/current.md`:** Engine overwrites with new ~5 pages each session.
- [ ] **Append-only `Summary.md`:** Engine appends a delta paragraph after each session (via Gemini Flash for cost efficiency).
- [ ] **Structured Changelog:** Daily `/Changelog/[YYYY-MM-DD].md` entries with date, word count, human edits detected, narrative summary.
- [ ] **Manuscript Compiler:** Read previous `Full_Book_[date].md`, append new pages, write new `Full_Book_[YYYY-MM-DD].md`.
- [ ] **Book Completion Handler:** Detect finished book → write `COMPLETE` marker → `openclaw cron delete <job-id>`.

## Phase 3: The Writing Engine System Prompt
**Goal:** Author the `AGENTS.md` for the `ink-engine` agent workspace.
(See `writing_engine.md` for the full technical mandate that this system prompt must implement)

## Phase 4: V2 - Ecosystem
- [ ] **Static Site Generator:** Hook into `git push` to rebuild `books.philapps.com`.
- [ ] **Validation Layer:** Automated check for consistency across chapters.
