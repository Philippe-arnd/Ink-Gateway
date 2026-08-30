# Ink Gateway — Book Repository

This repository is a book managed by the **Ink Gateway** framework. `ink-cli` is
the sole interface to its filesystem and Git state — never write these files
or run git commands directly; always go through the commands below.

Writing itself (continuing the story, corrections, rewrites) happens through
the [Ink-Gateway-App](https://github.com/Philippe-arnd/Ink-Gateway-App) web
editor, not through this repository directly. What follows is the scaffolding
and maintenance surface `ink-cli` still owns.

---

## Prerequisites — Install ink-cli

```bash
ink-cli --version
```

If not found:

```bash
curl -fsSL https://raw.githubusercontent.com/Philippe-arnd/Ink-Gateway/main/install.sh | bash
```

---

## Getting Started

Check whether `Global Material/Config.yml` exists.

| State | Action |
|---|---|
| **Absent** | Repository not initialized — run `ink-cli init` (below) |
| **Present** | Ready to write — open it in the Ink-Gateway-App web editor |

**If absent — initialize:**

```bash
ink-cli init <repo-path> --title "<Book Title>" --author "<Author Name>" --agent
```

The command outputs JSON with a `questions` array (`question`, `hint`,
`target_file` per entry). Ask the author each question in order. Once you
have all 13 answers, extrapolate each brief answer into rich, detailed
content — do not copy answers verbatim — then fill in the template files
`ink-cli init` already scaffolded:

| File | Derived from |
|---|---|
| `Global Material/Config.yml` | Q1 (`language:`), Q3 (`target_length: <pages × 250>`, `chapter_count: ceil(target_length / 3000)`), Q4 (`words_per_session: <pages × 250>`) — update these fields only, preserve the rest |
| `Global Material/Soul.md` | Q5–Q6: narrator voice, tone, prose style |
| `Global Material/Characters.md` | Q7–Q8: character sheets |
| `Global Material/Outline.md` | Q9–Q11: plot arc |
| `Global Material/Lore.md` | Q12: world-building |
| `Chapters material/Chapter_01.md` | Q13: chapter 1 scene beats |

Preserve each template's section headings exactly — replace only the `[...]`
placeholders. Then commit and push:

```bash
git -C <repo-path> add -A
git -C <repo-path> commit -m "init: populate global material from author Q&A"
git -C <repo-path> push origin main
```

Stop. Notify the author the book is ready to write in the Ink-Gateway-App
web editor.

---

## Available Commands

```
Tool: advance_chapter
Description: Advance to the next chapter. Updates .ink-state.yml (increments
  current_chapter, resets chapter word count to 0) and commits. Does NOT push.
Shell: ink-cli advance-chapter $repo_path
```

```
Tool: apply_format
Description: Apply format corrections to Full_Book.md — insert title, author,
  and missing chapter headings at the right positions. Commits and pushes.
Shell: echo "$patch_json" | ink-cli apply-format $repo_path
Stdin: JSON patch object, e.g. {"prepend": "# Title\n*by Author*\n\n---",
  "insert_headings": [{"before_anchor": "...", "heading": "# Chapter Two"}]}
```

```
Tool: status
Description: Read-only snapshot — chapter, word counts, lock status, completion flags.
Shell: ink-cli status $repo_path
```

```
Tool: doctor
Description: Validate repo structure, Config.yml, git remote, and session lock state.
Shell: ink-cli doctor $repo_path
```

```
Tool: update_agents
Description: Refresh this file (and CLAUDE.md/GEMINI.md) from the latest
  embedded template. Commits and pushes. Idempotent.
Shell: ink-cli update-agents $repo_path
```

`repo_path` is the local clone of this book repository.
