# Roadmap: Ink Gateway Implementation

## Phase 1: Git Sync Setup & Rust Scaffold
**Goal:** Wire the editor and the agent gateway through GitHub. Build `ink-cli` with the `session-open` subcommand.

- [ ] **Editor Git Sync:** Configure the markdown editor with git auto-sync (commit + pull + push on a timer).
- [ ] **`ink-engine` Agent Setup:** Register the agent in the agent gateway with workspace at `/data/ink-gateway`.
- [ ] **Book Repository Setup:** Initialize the first book repo on GitHub; clone into the editor's space and into the agent gateway's workspace.
- [ ] **`Config.yml` Template:** Define schema with all fields:
    - `target_length`, `chapter_count`, `chapter_structure`
    - `words_per_session` — words to generate per session
    - `summary_context_entries` — recent `Summary.md` paragraphs to include in context (default: `5`)
    - `current_chapter` — active chapter index; author increments to advance
    - `session_timeout_minutes` — stale lock threshold in minutes (default: `60`)
    - **Note:** The AI model is NOT a book-level setting. It is specified when registering the cron job at the agent gateway level.
- [ ] **`ink-cli` — `session-open` subcommand:**
    - **Git-setup phase:**
        - Clone repo if absent; fetch if present.
        - **Kill signal check (first):** if `.ink-kill` exists → delete it, delete `.ink-running` if present, return `{ "kill_requested": true }` and exit 0. No further processing.
        - Timestamp-based detection of human edits → `git add . && git commit -m 'chore: human updates' && git push origin main`. Collect list of modified files.
        - Create snapshot tag `ink-YYYY-MM-DD-HH-MM` (current timestamp). Each session gets its own tag — multiple sessions per day are supported.
        - **Lock file check with stale detection:** if `.ink-running` exists:
            - Read the ISO timestamp from its content.
            - If age ≤ `session_timeout_minutes` → set `session_already_run: true`, exit 0.
            - If age > `session_timeout_minutes` → delete stale lock, set `stale_lock_recovered: true`, continue.
        - Create `.ink-running` with current ISO timestamp as content.
        - Ensure `draft` branch exists, checkout, rebase onto `main`.
    - **Context phase:**
        - Parse `Config.yml` via `serde_yaml`.
        - Load all files in `Global Material/`. For `Summary.md`, include only the last `summary_context_entries` paragraphs.
        - Load only `Chapters material/Chapter_<current_chapter>.md` and `Chapter_<current_chapter+1>.md` (skip if files don't exist).
        - Load `Review/current.md`. Extract `<!-- INK: [text] -->` comments into a typed `instructions` array (`anchor` + `instruction`). Strip comments from `content`.
        - Compute `word_count: { total, target, remaining }` from `Current version/Full_Book.md`.
    - Output full JSON payload (schema defined in `agentic-architecture.md`).

## Phase 2: Full Automation
**Goal:** Complete the session loop end-to-end.

- [ ] **Cron Job (per book):**
    ```bash
    agent-gateway cron add \
      --name "Ink: <Book Title>" \
      --cron "<cron-schedule>" \
      --session isolated \
      --agent "ink-engine" \
      --model <model-id> \
      --thinking high \
      --thinking-budget 10000 \
      --max-turns 5 \
      --timeout 1800 \
      --message "Process book: https://github.com/<github-username>/<book-repo>"
    ```
    > `--thinking-budget 10000` caps thinking tokens (the only unbounded cost element). `--max-turns 5` caps tool call loops. `--timeout 1800` (30 min) triggers gateway kill on hung sessions; stale lock recovery cleans up on the next trigger. Flag names are illustrative — use your gateway's equivalents.
- [ ] **`ink-cli` — `session-close` subcommand:**
    - Guard: if `.ink-running` does not exist, emit `{ "error": "no active session", "status": "error" }` and exit non-zero. Prevents double-write if called twice.
    - Read generated prose from **stdin**.
    - Execute in strict order:
        1. Overwrite `Review/current.md` with the new prose.
        2. Append a delta paragraph to `Summary.md` (this session only — never rewrite existing entries).
        3. Write `Changelog/[YYYY-MM-DD-HH-MM].md`: date header, session word count, human edits list, one-paragraph narrative summary.
        4. Append prose to `Current version/Full_Book.md`.
        5. `git push origin main` + `git push origin draft`.
    - Delete `.ink-running` lock file from repo root.
    - Output JSON: `{ "session_word_count": N, "total_word_count": N, "target_length": N, "completion_ready": bool, "status": "closed" }`.
- [ ] **`ink-cli` — `complete` subcommand:**
    - Guard: if `COMPLETE` already exists, emit `{ "error": "book already complete", "status": "error" }` and exit non-zero.
    - Write `COMPLETE` marker to repo root.
    - Delete `.ink-running` lock file from repo root (if present — may already be gone if `session-close` ran).
    - `git add COMPLETE && git commit -m "book: complete" && git push origin main`.
    - Output JSON: `{ "status": "complete", "total_word_count": N }`.
    - The agent handles book-completion notification (via the gateway's notification channel) and signals the gateway to delete the cron job after receiving this response.
- [ ] **Tool definitions in `AGENTS.md`:** Define `session_open` and `session_close` as inline shell tools. No separate SKILL.md required.

## Phase 3: The Writing Engine System Prompt
**Goal:** Author the `AGENTS.md` for the `ink-engine` agent workspace.
(See `writing_engine.md` for the full technical mandate.)

- [ ] Write global `AGENTS.md`:
    - Inline tool definitions: `session_open` → `ink-cli session-open`, `session_close` → `ink-cli session-close`.
    - Strict session flow (open → check idempotency → plan → generate → close → optional complete).
    - Abort rules (in order): `kill_requested` → log and stop; `session_already_run` → log and stop; `stale_lock_recovered` → log warning and continue.
    - INK instruction processing from `current.instructions` array.
    - Completion rule: `completion_ready` flag + narrative judgment.
    - Human override handling from `human_edits` list.
    - Guardrail rules (must be explicit):
        - Call `session_open` exactly once. Call `session_close` exactly once. Never call either again in the same session — the lock is deleted on close, so a second `session_open` would not be blocked by the binary.
        - If any tool returns `"status": "error"`, stop immediately. Do not retry.
        - Call `complete` at most once, only when `completion_ready` is `true` and narrative closure is confirmed. When in doubt, skip — the cron job runs again.
        - After a successful `complete` response: notify the user, signal cron deletion, stop. No further tool calls.

