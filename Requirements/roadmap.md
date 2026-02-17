# Roadmap: Ink Gateway Implementation

This roadmap outlines the technical path for a fully automated multi-book writing engine using a Shared Volume architecture between SilverBullet and OpenClaw.

## Phase 1: Shared Volume Setup & MVP
**Goal:** Create the physical link between the Editor and the Engine.
- [ ] **Volume Definition:** Configure a host directory (e.g., `/data/ink-gateway/books`) on the VPS.
- [ ] **SilverBullet Config:** Update Coolify service to mount the host directory.
- [ ] **OpenClaw Config:** Update Coolify/Docker config to mount the same host directory into the agent's workspace.
- [ ] **Permissions Check:** Ensure both containers can read/write to the shared volume.
- [ ] **Repository Setup:** Initialize `Philippe-arnd/Ink-gateway`.
- [ ] **Orchestrator Script (`engine.py`):**
    - **Pre-flight:** `git add . && git commit` any local changes from SilverBullet.
    - **Sync:** `git push origin main`.
    - **Branching:** `git checkout draft`, `git rebase main`.
    - **Snapshotting:** `git tag pre-nightly-$(date +%F)`.
- [ ] **Payload Builder:** Recursive reader for the shared volume folders.

## Phase 2: V1 - Full Automation
- [ ] **Nightly Cron Job:**
    - Schedule: `0 2 * * *`.
    - Task: Trigger `engine.py` via `sessions_spawn`.
- [ ] **Intelligent Change Detection:** Script logic to compare file timestamps and notify the AI of specific Human edits made during the day.
- [ ] **Summary Loop:** Automated update of `Summary.md` via Gemini Flash.
- [ ] **Manuscript Compiler:** Daily auto-generation of the `Full_Book_[Date].md` file.

## Phase 3: The Writing Engine System Prompt
(See writing_engine.md for the full system prompt instructions)

## Phase 4: V2 - Ecosystem
- [ ] **Static Site Generator:** Hook into `git push` to rebuild `books.philapps.com`.
- [ ] **Validation Layer:** Automated check for consistency across chapters.
