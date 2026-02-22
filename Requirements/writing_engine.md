# Writing Engine Brain - Technical Specification

This document serves as the technical mandate for the `ink-engine` agent running an Ink Gateway writing session.

## 1. Environment & Permissions

- **Clone location:** `/data/ink-gateway/books/<book-name>/`
- **Git auth:** The agent gateway's configured GitHub token handles all push/pull.
- **File I/O:** The agent never writes files directly. All operations go through `ink-cli` subcommands.
- **Tools:** Four shell tools — `session_open`, `session_close`, `complete`, `advance_chapter` — defined inline in `AGENTS.md`.

---

## 2. Context Assembly (The Input)

The agent calls `ink-cli session-open <repo-path>`, which performs git-setup and context aggregation in a single step and returns one JSON payload.

**What the payload contains:**

| Field | Source | Notes |
|---|---|---|
| `config` | `Global Material/Config.yml` | All settings; `current_chapter` sourced from `.ink-state.yml` |
| `kill_requested` | `.ink-kill` file existence | `true` → abort immediately, no generation |
| `session_already_run` | Lock file existence | `true` if `.ink-running` is current → abort |
| `stale_lock_recovered` | Lock file age | `true` if stale lock auto-removed → proceed normally |
| `chapter_close_suggested` | `.ink-state.yml` vs `words_per_chapter` | `true` when chapter word count ≥ 90% of target → engine may call `advance_chapter` |
| `current_chapter_word_count` | `.ink-state.yml` | Words written to `Full_Book.md` in current chapter so far |
| `global_material[]` | All files in `Global Material/` | `Summary.md` truncated to last `summary_context_entries` substantive paragraphs |
| `chapters.current` | `Chapters material/Chapter_<N>.md` | Active chapter outline |
| `chapters.next` | `Chapters material/Chapter_<N+1>.md` | Only populated when `chapter_close_suggested` is `true` |
| `current_review.content` | `Review/current.md` | Author instructions stripped; engine markers preserved |
| `current_review.instructions` | Extracted from `current.md` | `<!-- INK: ... -->` comments as typed `{ anchor, instruction }` array |
| `word_count` | `Full_Book.md` word count | `{ total, target, remaining }` (prose only, comment lines excluded) |
| `human_edits` | Git timestamp check | Files modified today, committed to `main` |

**`Global Material/` contains:**
- `Soul.md` — narrator voice, tone, prose style
- `Outline.md` — full plot arc and story goal
- `Characters.md` — character profiles and arcs
- `Lore.md` — world-building and consistency rules
- `Summary.md` — append-only session log (truncated in context)
- `Config.yml` — book parameters

---

## 3. Core Logic & Prioritization

### 3.1 Abort Checks
**First checks after `session-open` — evaluate in this order before any other action:**

1. **Kill requested:** if `kill_requested` is `true`, log "Kill signal received — session cancelled by author." Stop immediately. No prose generation, no further tool calls.
2. **Concurrent session:** if `session_already_run` is `true`, log "Session already in progress — lock file is current. Aborting to avoid conflict." Stop immediately.
3. **Stale lock recovered:** if `stale_lock_recovered` is `true`, log "Stale lock removed (previous session exceeded timeout or was killed externally). Proceeding." Continue with the session normally.

> **How to cancel a session:** Create an `.ink-kill` file in the repo root via the markdown editor. The editor auto-commits and pushes it to GitHub. The engine detects it on the next scheduled trigger and cancels that run. This cancels the *next* session only — it does not interrupt a session that is already running. To stop a running session, use the gateway's native cancel/stop control; the stale lock recovery mechanism will handle cleanup on the next trigger.

### 3.2 Human Override
Read `human_edits` from the payload. Adapt the session plan:
- Chapter outline changed → re-evaluate that chapter's direction.
- `current.md` was edited → treat the human's version as authoritative.
- `Soul.md` changed → the human has adjusted the narrative voice; honor it.
- `Outline.md` changed → re-read the updated plot arc before generating.

### 3.3 Instruction Processing
Read `current_review.instructions` from the payload. For each entry:
- Locate the `anchor` passage in `current_review.content`.
- Apply the `instruction` as a targeted rewrite of that specific passage.
- Author instructions are already stripped from `current_review.content` — no cleanup needed.
- Wrap each rewrite in `<!-- INK:REWORKED:START -->` ... `<!-- INK:REWORKED:END -->` blocks.

### 3.4 Narrative Generation
- Anchor to the continuation point in `current.content` and last `Summary.md` entry.
- Reference `Outline.md` arc and `chapters.current` for the active chapter's goals.
- Consult `chapters.next` for look-ahead coherence at chapter boundaries.
- Generate exactly `config.words_per_session` words.
- Voice and style must adhere to `Soul.md`.

**Implicit Approval:** If `human_edits` is empty, the previous draft is accepted — continue writing.

---

## 4. Output & Sync

The agent calls `ink-cli session-close <repo-path>` with generated prose piped via stdin. The binary executes in strict order:

1. Overwrite `Review/current.md` with the new prose (becomes next session's context window).
2. Append a delta paragraph to `Summary.md` — this session's events only, never rewrite existing entries.
3. Write a structured entry to `Changelog/[YYYY-MM-DD].md`:
   - Date header
   - Session word count
   - List of human edits (filenames + one-line description)
   - One-paragraph narrative summary
4. Append new prose to `Current version/Full_Book.md`.
5. `git push origin main` + `git push origin draft`.

Returns JSON:
```json
{
  "session_word_count": 1487,
  "total_word_count": 43210,
  "target_length": 90000,
  "current_chapter_word_count": 2341,
  "completion_ready": false,
  "status": "closed"
}
```

### Completion (conditional)
`completion_ready` is `true` when `total_word_count` is within ±10% of `target_length`. This is a necessary but not sufficient condition. The agent must also confirm that the narrative arcs in `Outline.md` are genuinely fulfilled.

If both are satisfied, the agent calls `ink-cli complete <repo-path>`, which:
1. Writes a `COMPLETE` file to the repo root.
2. Commits and pushes it.

Returns JSON: `{ "status": "complete", "total_word_count": N }`.

After receiving this response, the agent:
- Notifies the user via the gateway's configured notification channel.
- Signals the gateway to delete this cron job.

These are the final actions of the session. No further tool calls are made.

---

## 5. Automation, Bounds & Observability

### 5.1 Session Token Budget

Every cost component is bounded except extended thinking, which MUST be capped at the gateway level.

| Component | Bound mechanism | ~Tokens (default config) |
|---|---|---|
| Context payload (input) | Config.yml params | ~4,800 input |
| Generated prose (output) | `words_per_session` | ~2,250 output |
| Tool call overhead | Fixed (2–3 calls/session) | ~300 |
| Extended thinking | **Gateway `--thinking-budget`** | **Unbounded if not set** |

**Context payload breakdown** (with default Config.yml values):
- `current.md` (1,500 words): ~2,250 tokens — bounded by `words_per_session`
- Soul + Outline + Characters + Lore: ~1,500 tokens — bounded by how much the author writes
- `Summary.md` (last 5 paragraphs): ~500 tokens — bounded by `summary_context_entries`
- 2 chapter outlines: ~300 tokens — bounded by outline length
- Agent system prompt (AGENTS.md): ~250 tokens — fixed

The payload does not grow as the book grows. `Summary.md` truncation and chapter-only loading keep it constant across all 40+ sessions.

**The only runaway risk is extended thinking.** Without a thinking budget cap, a single session with `--thinking high` can consume 10K–32K tokens of thinking before generating a single word of prose. Multiply that by an agent that loops (even just once), and costs spike. The `--max-turns` gateway setting is the hard ceiling on looping.

### 5.2 Recommended Gateway Configuration

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

| Flag | Value | Rationale |
|---|---|---|
| `--thinking-budget` | 10000 | Caps thinking tokens. Normal session: 2–3 tool calls leaves ample room. |
| `--max-turns` | 5 | Allows: open (1) + close (2) + complete (3) + 2 buffer for error handling. Hard ceiling on loops. |
| `--timeout` | 1800 | 30-minute wall-clock limit. Triggers gateway kill; stale lock recovery cleans up on next run. |

> Flag names are illustrative — use your gateway's equivalent parameters. The intent is: cap thinking tokens, cap tool call turns, cap wall time.

### 5.3 Session Observability

**What the user can see and where:**

| Signal | Where | Latency |
|---|---|---|
| Session running now | `.ink-running` exists in repo (visible in editor) | Live (file is in repo) |
| Session start time | Content of `.ink-running` (ISO timestamp) | Live |
| Session result | `Changelog/YYYY-MM-DD-HH-MM.md` (auto-pushed by `session-close`) | After session ends |
| Live agent output | Gateway logs / UI | Real-time |
| Book progress | `word_count` field in `session-close` JSON (logged by gateway) | After session ends |

**To cancel the next scheduled session:** Create `.ink-kill` in the repo root via the editor. The editor commits and pushes it. `session-open` detects it on the next trigger, cancels cleanly, and removes the file.

**To stop a running session:** Use the gateway's native cancel/stop control. The session process is terminated. `.ink-running` remains on disk. On the next trigger, `session-open` detects the stale lock (age > `session_timeout_minutes`), removes it, logs the recovery, and proceeds from the last committed state.

- **Repo path:** The agent gateway clones (or pulls) the repo from the URL in the message, then passes the local path to `ink-cli`.

---

## 6. Guardrails

Two layers of protection against runaway execution and double-writes.

### 6.1 Binary-Level (`ink-cli`)

Enforced mechanically regardless of agent behavior:

- **`session-close` lock check:** Before executing, verifies that `.ink-running` exists in the repo root. If absent → outputs `{ "error": "no active session", "status": "error" }` and exits non-zero. Prevents prose from being double-written if `session-close` is called twice in one session.
- **`complete` idempotency:** Before executing, verifies that `COMPLETE` does not already exist. If it does → outputs `{ "error": "book already complete", "status": "error" }` and exits non-zero.

### 6.2 Agent-Level (`AGENTS.md`)

These rules MUST be stated explicitly in the writing engine's `AGENTS.md`:

- **One open, one close:** Call `session_open` exactly once at session start. Call `session_close` exactly once after prose is ready. Never call either tool again in the same session — the lock file is deleted on close, so a second `session_open` would succeed but produce a duplicate session.
- **Abort on lock:** If `session_already_run` is `true`, stop immediately. Output a brief explanation. Do not call any other tools.
- **No retries:** If any tool call returns a non-zero exit or an `"error"` status in the JSON, stop immediately. Do not retry. The next cron trigger will handle recovery.
- **Generate before close:** Do not call `session_close` speculatively or as a mid-session checkpoint. Only call it when the complete prose output is ready.
- **Completion discipline:** Call `complete` at most once, only when `completion_ready` is `true` AND you have verified narrative closure against `Outline.md`. When in doubt, do not call `complete` — the cron job will run again next session.
- **Stop after complete:** After a successful `complete` response, notify the user and signal the cron job deletion. These are the final actions — no further tool calls.
