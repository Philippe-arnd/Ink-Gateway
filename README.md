# Ink Gateway

A collaborative AI-driven framework for writing Science Fiction and Fantasy novels. The engine runs nightly writing sessions autonomously, while the human author edits in a browser-based markdown editor — no Git knowledge required.

## How It Works

Three components sync through GitHub on a self-hosted VPS:

| Component | Role |
|---|---|
| **SilverBullet** (`write.philapps.com`) | Human author's markdown editor. Uses the built-in Git library to auto-commit and push edits to GitHub. |
| **GitHub** | Single source of truth. The sync layer between SilverBullet and the engine. |
| **OpenClaw `ink-engine` agent** | Runs nightly at 02:00 UTC. Pulls from GitHub, reads context, generates ~5 pages of prose, pushes all Git operations. |

Each book is an independent GitHub repository. SilverBullet auto-syncs human edits throughout the day; the engine pulls them at 02:00 UTC, generates new content, and pushes back. SilverBullet's next sync makes the AI output visible in the editor.

**Implicit approval:** If no files were modified today, the previous draft is treated as accepted and the engine keeps writing. Human edits are the only signal needed.

## Per-Book Repository Structure

```
/Global Material/
  Config.yml           # model, target_length, chapter_count, chapter_structure, nightly_output_target
  Outline.md           # Plot arc; completion is detected when all arcs are fulfilled
  Summary.md           # Append-only delta log; used as a progress anchor each session
  Lore.md
  Characters.md
  Style_guide.md

/Chapters material/    # Chapter outlines only (no prose). Human edits trigger AI re-evaluation.
/Review/
  current.md           # Rolling ~5-page context window (~1500 words). Overwritten each session.
/Changelog/
  YYYY-MM-DD.md        # Date, word count, human edits detected, narrative summary
/Current version/
  Full_Book_[date].md  # Source of truth for all prose. Engine appends new pages each session.
COMPLETE               # Written by engine when book is finished (triggers cron self-deletion)
```

## Nightly Engine Session

1. **Pre-flight** — Detect files modified today → `git add . && git commit -m 'chore: human updates' && git push origin main`
2. **Snapshot** — `git tag pre-nightly-$(date +%F)`
3. **Branch** — `git checkout draft && git rebase main`
4. **Load context** — `/Global Material/` + `/Chapters material/` + `/Review/current.md`
5. **Anchor** — Determine continuation from `current.md` + last `Summary.md` entry
6. **Instructions** — Process `<!-- Claw: [Instruction] -->` comments in `current.md` → rewrite + delete
7. **Generate** — `nightly_output_target` words following `Style_guide.md`
8. **Maintain** — Overwrite `current.md` → append delta to `Summary.md` → write `/Changelog/[date].md` → append to `Full_Book_[date].md`
9. **Completion check** — Arcs fulfilled + word count within ±10% → write `COMPLETE` → `openclaw cron delete <job-id>`
10. **Sync** — `git push origin main` then `git push origin draft`

## Registering a New Book

```bash
openclaw cron add \
  --name "Ink: <Book Title>" \
  --cron "0 2 * * *" \
  --session isolated \
  --agent "ink-engine" \
  --thinking high \
  --message "Process book: https://github.com/Philippe-arnd/<book-repo>"
```

No central book registry — each book is simply a cron job. Deleting the cron job removes the book from the engine.

## Human Authoring Flow

- Edit `/Review/current.md` or `/Chapters material/` files directly in SilverBullet.
- Insert `<!-- Claw: [Instruction] -->` anywhere in `current.md` to request a targeted rewrite on the next nightly run.
- SilverBullet's git auto-sync commits and pushes your edits automatically (every few minutes). No manual Git action required.

## API Cost Reference

| Model | Per 200-page book |
|---|---|
| Gemini Flash | ~$0.04 |
| Claude Sonnet | ~$1.93 |
| Claude Opus | ~$6.92 |

See `Requirements/cost-analysis.md` for the full breakdown.

## Implementation Status

| Phase | Status | Description |
|---|---|---|
| **Phase 1** | Planned | SilverBullet git sync setup, `ink-engine` agent registration, `engine.py` scaffold |
| **Phase 2** | Planned | Full nightly automation (cron, change detection, manuscript compiler, completion handler) |
| **Phase 3** | Planned | Author the `ink-engine` `AGENTS.md` system prompt |
| **Phase 4** | Planned | Static site (`books.philapps.com`), validation layer |

See `Requirements/roadmap.md` for detailed task checklists and `Requirements/framework.md` for full architecture documentation.
