# Ink Gateway — VPS Infrastructure Setup

All steps run on the self-hosted VPS, unless noted otherwise.

---

## Architecture: Git as the Sync Layer

No shared volume between the editor and the agent gateway. GitHub is the single source of truth.

```
Human edits in markdown editor
  → Editor git auto-sync commits + pushes to GitHub

At scheduled time, agent gateway runs ink-engine
  → ink-cli session-open: fetch from GitHub, commit human edits, load context
  → agent generates prose
  → ink-cli session-close: write + maintain + push to GitHub

Editor next auto-sync
  → git pull from GitHub (human sees AI output)
```

---

## 1. Markdown Editor — Git Sync Setup

### 1a. Git credentials

```bash
git config --global credential.helper store
git config --global user.name "Ink Editor"
git config --global user.email "ink-engine@noreply"
echo "https://<github-username>:<github-token>@github.com" > ~/.git-credentials
```

### 1b. Enable git auto-sync in the editor

Configure the editor to commit and push changes automatically on a timer (e.g., every 5 minutes if changes exist). Refer to your editor's git integration documentation.

### 1c. Clone the book repo

```bash
cd /path/to/editor/space
git clone https://<github-username>:<token>@github.com/<github-username>/<book-repo>.git
```

---

## 2. Agent Gateway — GitHub Access

Verify access:

```bash
gh auth status
git ls-remote https://github.com/<github-username>/<book-repo>.git
```

`ink-cli` uses the HTTPS repo URL — the configured token handles authentication automatically.

---

## 3. Register the `ink-engine` Agent

```bash
agent-gateway agents add ink-engine --workspace /data/ink-gateway
```

Verify:

```bash
agent-gateway agents list
# ink-engine should appear with workspace /data/ink-gateway
```

The agent's `AGENTS.md` system prompt lives at `/data/ink-gateway/AGENTS.md` (authored in Phase 3). It contains the tool definitions for `session-open` and `session-close` inline — no separate skill file needed.

---

## 4. Deploy `ink-cli`

Build the Rust binary and install it on the VPS:

```bash
# In the Ink Gateway repo
cargo build --release
cp target/release/ink-cli /usr/local/bin/ink-cli
```

Verify:

```bash
ink-cli --version
ink-cli --help
```

---

## 5. Book Repository Setup (per book)

### 5a. Initialize the repo on GitHub

Create a new private GitHub repo, then initialize the book structure:

```bash
git clone https://github.com/<github-username>/<book-repo>.git
cd <book-repo>

mkdir -p "Global Material" "Chapters material" "Review" "Changelog" "Current version"

# Copy and fill in the Config.yml template
cp /path/to/Ink-Gateway/templates/Config.yml "Global Material/Config.yml"
# Edit current_chapter, target_length, and other book settings as appropriate
# NOTE: The AI model is NOT set here — configure it via --model when registering the cron job

# Create the Global Material files
touch "Global Material/Soul.md"
touch "Global Material/Outline.md"
touch "Global Material/Characters.md"
touch "Global Material/Lore.md"
touch "Global Material/Summary.md"

# Create working files
touch "Review/current.md"
touch "Current version/Full_Book.md"

# Create at least the first chapter outline
touch "Chapters material/Chapter_01.md"

git add .
git commit -m "chore: initialize book structure"
git push origin main
```

### 5b. Create the `draft` branch

```bash
git checkout -b draft main
git push -u origin draft
```

> `ink-cli session-open` handles this automatically on first run. Doing it manually confirms repo access is working.

### 5c. Clone into the editor

Follow step 1c above so the book appears in the editor.

### 5d. Fill in the content files

Before the first session, the author should add content to:
- `Global Material/Soul.md` — define the book's narrative voice
- `Global Material/Outline.md` — write the plot arc
- `Global Material/Characters.md` — describe main characters
- `Chapters material/Chapter_01.md` — write the first chapter outline

---

## 6. Register the Cron Job (per book)

```bash
agent-gateway cron add \
  --name "Ink: <Book Title>" \
  --cron "<cron-schedule>" \
  --session isolated \
  --agent "ink-engine" \
  --model <model-id> \
  --thinking high \
  --message "Process book: https://github.com/<github-username>/<book-repo>"
```

Verify:

```bash
agent-gateway cron list
# "Ink: <Book Title>" should appear with the configured schedule
```

---

## Quick Reference

| Component | GitHub auth | Method |
|---|---|---|
| Markdown editor | Fine-grained PAT | HTTPS credential store or SSH key |
| Agent gateway | Existing token + `gh` CLI | Already configured |

| Path | Purpose |
|---|---|
| `/data/ink-gateway/` | Agent workspace root |
| `/data/ink-gateway/AGENTS.md` | `ink-engine` system prompt + tool definitions (Phase 3) |
| `/data/ink-gateway/books/<book-name>/` | Cloned book repo |
| `/usr/local/bin/ink-cli` | Compiled Rust binary |

| Config.yml field | Purpose |
|---|---|
| `current_chapter` | Active chapter — **increment via editor to advance** |
| `words_per_session` | Words per session (also the size of `current.md`) |
| `summary_context_entries` | How many recent `Summary.md` paragraphs go into context |
