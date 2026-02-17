# Project: The Ink Gateway - Collaborative AI Book Writing Framework

## 1. Overview
A collaborative AI-driven writing framework hosted on a private VPS. It enables human-AI collaboration on Science Fiction and Fantasy novels through a "Reusable Workflow" model. The framework handles multi-book management, automated nocturnal writing sessions, and bidirectional Git synchronization.

## 2. Technical Stack & Architecture
- **Infrastructure:** Self-hosted on OVH VPS via Coolify.
- **Framework Model:** "The Ink Gateway" acts as the engine/workflow. Each book has its own dedicated GitHub repository.
- **Editor:** [SilverBullet](https://silverbullet.md/) (Docker-based) at `write.philapps.com`.
- **Versioning:** GitHub Repositories.
- **Automation:** OpenClaw Cron + Writing Engine Sub-agents.

## 3. Repository Structure (Per Book)
Each book repository follows this folder structure to organize the collaboration:

- **`/Global Material/`**: Core reference files.
    - `Lore.md`: World-building, rules, and history.
    - `Characters.md`: Detailed character profiles and arcs.
    - `Outline.md`: Global plot arc and structure.
    - `Style_guide.md`: Voice, tone, and specific prose instructions.
    - `Config.yml`: Book-specific parameters (model, target lengths).
    - `Summary.md`: Auto-updated narrative summary to combat context decay.
- **`/Chapters material/`**: The skeletal structure.
    - One `.md` file per chapter containing the specific chapter's plot goals and status. 
    - Note: These files are stable; changes from the Human writer here trigger AI re-evaluation of the specific chapter.
- **`/Review/`**: The active work zone.
    - Contains the latest drafted pages (IA) and polished prose (the Human writer).
    - This folder is the primary context for narrative continuity.
- **`/Changelog/`**: Daily logs.
    - One page per day detailing AI and human contributions.
- **`/Current version/`**: The compiled manuscript.
    - `Full_Book_[Date].md`: A read-only reassembly of all chapters for global navigation.

## 4. Operational Workflow

### 4.1 Git Flow "Draft-to-Main"
1.  **AI Session (Draft Branch):**
    - Pulls `main`, rebases `draft`.
    - Reads `/Global Material/` and `/Chapters material/`.
    - Processes `/Review/` (Drafts new content or executes instructions from the Human writer).
    - Updates `/Summary/`, `/Changelog/`, and compiles the `/Current version/`.
    - Pushes to `draft`.
2.  **Human Session (Review & Merge):**
    - The Human writer edits in `/Review/` or updates `/Chapters material/`.
    - The Human writer merges `draft` into `main` to validate the work.

### 4.2 Completion & Buffer Logic
- **Standard:** 1 page = 300 words.
- **Buffer:** +/- 10% on chapter and total book length to ensure narrative closure.
- **Trigger:** Completion is reached when `Outline.md` arcs are fulfilled and length is within the buffer.

## 5. Security & Reliability
- **Git Snapshots:** Mandatory `pre-nightly-YYYY-MM-DD` tags.
- **Priority:** The Human writer's changes always override AI content in case of merge conflicts.
