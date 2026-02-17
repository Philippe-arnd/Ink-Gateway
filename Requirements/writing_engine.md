# Writing Engine Brain - Technical Specification

This document serves as the technical mandate for the OpenClaw sub-agent running the Ink Gateway engine.

## 1. Environment & Permissions
The engine operates in a **Shared Volume environment** (e.g., `/data/ink-gateway/books/`) mapped between the SilverBullet Docker container and the OpenClaw container.
- **Permissions:** OpenClaw must have Read/Write access to this volume.
- **Git Auth:** OpenClaw uses the configured SSH keys to push/pull from GitHub.

## 2. Context Assembly (The Input)
For every session, the engine must load:
- **Permanent Context:** All files in `/Global Material/`.
- **Structural Context:** All files in `/Chapters material/` (to detect changes by the Human writer).
- **Active Context:** All files in `/Review/` (the "living" prose).
- **Change Detection (Smart Sync):** 
    - The engine must check file timestamps on the shared volume. 
    - **Logic:** Any file modified during the current day (date of the run) is prioritized.
    - These files represent "Human Overrides" and must be committed to `main` immediately to preserve the writer's work before any AI generation begins.

## 3. Core Logic & Prioritization
1.  **Human Writer Override:** 
    - **Step 1:** Commit any local changes in the Shared Volume to `main`.
    - **Step 2:** Adapt the AI plan to these changes immediately.
2.  **Instruction Processing:**
    - Scan for `<!-- Claw: [Instruction] -->` in `/Review/`.
    - **Action:** Replace targeted text + Delete comment.
3.  **Narrative Generation:**
    - Reference `Outline.md` and `/Chapters material/`.
    - Generate ~1500 words of prose.
    - Style must strictly adhere to `/Global Material/Style_guide.md`.

## 4. Maintenance & Sync Tasks (The Output)
After generating prose, the engine MUST:
1.  **Update `Summary.md`**: Summarize the new events.
2.  **Generate Changelog**: Create a new entry in `/Changelog/`.
3.  **Compile Manuscript**: Re-assemble prose into `/Current version/Full_Book_[YYYY-MM-DD].md`.
4.  **Sync to GitHub:** 
    - Push `main` (Human updates).
    - Push `draft` (AI updates).

## 5. Automation (The Cron Job)
- **Trigger:** Scheduled via OpenClaw Cron.
- **Schedule:** 02:00 UTC (Nightly).
- **Command:** 
  `openclaw sessions_spawn --task "Ink-Gateway Engine Run: Syncing main with daily human modifications and generating draft prose" --agent "ink-engine" --thinking "high"`
- **Pre-run check:** Verify if any files have been modified today; if so, perform a mandatory `git add . && git commit -m 'chore: human updates' && git push origin main` before proceeding with the `draft` branch logic.
