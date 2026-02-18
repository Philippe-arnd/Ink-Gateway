# Agentic Architecture for Ink Gateway

This document outlines the modular transition from a script-based engine to an **Agent + Skill** architecture. This decoupling ensures that the writing logic remains portable while the creative personality is specific to each book.

## 1. Overview

The architecture is split into three layers:
1.  **Core Logic (Generic):** A Python-based CLI tool (`ink-cli`) that handles file operations and Git sync.
2.  **Capabilities (Skill):** An OpenClaw Skill that maps the CLI tools to AI-invocable functions.
3.  **Creative Brain (Agent):** An OpenClaw Agent that uses the skill, guided by book-specific `SOUL.md` and `MEMORY.md`.

---

## 2. The Generic Layer: `ink-cli`

To ensure portability, all "heavy lifting" is handled by a standalone Python CLI.

**File:** `core/ink_cli.py`
**Responsibilities:**
- `read-context`: Aggregates `/Global Material/`, `/Chapters material/`, and `/Review/current.md`.
- `apply-edit`: Injects generated prose into the correct chapter/current file.
- `maintenance`: Updates `Summary.md`, `Changelog/`, and compiles the `Full_Book.md`.
- `git-sync`: Executes the snapshot/branch/push lifecycle.

---

## 3. The OpenClaw Skill: `ink-manuscript`

This layer translates the `ink-cli` capabilities for the AI.

**File:** `/opt/openclaw/app/skills/ink-manuscript/SKILL.md`
**Tools provided:**
- `get_book_context(repo_url)`: Calls `ink-cli read-context`.
- `write_nightly_draft(content)`: Calls `ink-cli apply-edit`.
- `finalize_session()`: Triggers maintenance and Git sync via `ink-cli git-sync`.
- `analyze_instructions()`: Scans for `<!-- Claw: ... -->` tags.

---

## 4. The Agent Layer: `ink-engine`

This is the "Writer". It doesn't know *how* to Git push; it knows *how* to write and *when* to call its tools.

### Global Agent Definition
Defined in OpenClaw as the `ink-engine` agent. Its system prompt focuses on:
- Narrative structure.
- Adherence to the writing process.
- Tool usage sequence.

### Per-Book Specifics (The "Soul")
Each book repository contains its own identity files which are loaded by the agent:
- `SOUL.md`: Defines the narrator's voice, tone, and prose style (e.g., "Hard SF, cynical, technical").
- `GOAL.md`: The high-level plot arc and current objective.
- `MEMORY.md`: Long-term consistency (character traits, past events).

---

## 5. Execution Flow (The Nightly Run)

1.  **Trigger:** Cron job starts the `ink-engine` agent.
2.  **Bootstrap:** Agent loads the target repo and reads `SOUL.md` + `MEMORY.md`.
3.  **Contextualization:** Agent calls `get_book_context()` to see where it left off.
4.  **Planning:** Agent calls `analyze_instructions()` to see if the human (Phil) left notes.
5.  **Creation:** Agent generates prose, mimicking the `SOUL.md` style.
6.  **Action:** Agent calls `write_nightly_draft()` to save progress.
7.  **Closure:** Agent calls `finalize_session()` to sync everything back to GitHub.

## 6. Benefits

- **Model Agnostic:** You can switch from Gemini to Claude without changing a single line of Python code.
- **Scalable:** To start a new book, you just create a new repo with a new `SOUL.md` and add a cron job.
- **Portability:** If you move away from OpenClaw, the `ink-cli` and your book repos remain fully functional.
