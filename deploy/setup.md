# Ink Gateway — VPS Infrastructure Setup

All steps run on the OVH VPS managed via Coolify, unless noted otherwise.

---

## Architecture: Git as the Sync Layer

No shared volume is needed between SilverBullet and OpenClaw. GitHub is the single source of truth. Each container clones the book repo independently and syncs through git.

```
Human edits in SilverBullet
  → SilverBullet git auto-sync commits + pushes to GitHub (every N min)

At 02:00 UTC, OpenClaw runs engine.py
  → git fetch / pull from GitHub (picks up human edits)
  → generates AI prose
  → git push to GitHub

SilverBullet next auto-sync
  → git pull from GitHub (human sees AI output in editor)
```

---

## 1. SilverBullet — Git Sync Setup

SilverBullet's built-in Git library handles the human author's sync. It runs shell-based git commands, so git must be available in the container and credentials must be configured.

### 1a. Enable shell and install git

In Coolify → SilverBullet service → **"Docker Compose Override"**:

```yaml
services:
  silverbullet:
    environment:
      - SB_SHELL_BACKEND=local     # enables shell commands (default, but make it explicit)
      - GITHUB_TOKEN=ghp_your_token_here
```

Then add a startup script or custom Dockerfile to ensure git is installed and the credential helper is configured:

```bash
# Inside the SilverBullet container (or baked into a custom image)
apt-get install -y git
git config --global credential.helper store
git config --global user.name "SilverBullet"
git config --global user.email "ink-engine@noreply"
echo "https://Philippe-arnd:${GITHUB_TOKEN}@github.com" > ~/.git-credentials
```

> If you prefer, an SSH key works here too — the credential lives in the SilverBullet container and is separate from OpenClaw.

### 1b. Install and configure the Git library in SilverBullet

Open SilverBullet → run the command `Plugs: Add` → paste:

```
github:silverbulletmd/silverbullet-libraries/Git.md
```

In your SilverBullet `SETTINGS` page, add:

```yaml
git:
  autoSync: 5        # commit + pull + push every 5 minutes if changes exist
```

### 1c. Clone the book repo into SilverBullet's space

From inside the SilverBullet container (Coolify → Terminal):

```bash
cd /space
git clone https://Philippe-arnd:<token>@github.com/Philippe-arnd/<book-repo>.git
```

The book will appear as a top-level folder in SilverBullet's file tree.

---

## 2. OpenClaw — GitHub Access

OpenClaw already has a GitHub token and the `gh` CLI configured. No additional setup is needed for git authentication.

Verify `engine.py` can reach GitHub:

```bash
# Inside the OpenClaw container (Coolify → OpenClaw → Terminal)
gh auth status
git ls-remote https://github.com/Philippe-arnd/<book-repo>.git
```

`engine.py` uses the HTTPS repo URL passed by the cron job message — the existing token handles authentication automatically.

---

## 3. Register the `ink-engine` Agent

Inside the OpenClaw container:

```bash
openclaw agents add ink-engine --workspace /data/ink-gateway
```

Verify:

```bash
openclaw agents list
# ink-engine should appear with workspace /data/ink-gateway
```

The agent's `AGENTS.md` system prompt lives at `/data/ink-gateway/AGENTS.md` (authored in Phase 3).

---

## 4. Book Repository Setup (per book)

Perform these steps once per new book.

### 4a. Initialize the repo on GitHub

Create a new GitHub repo under `Philippe-arnd/<book-repo>` (private). Then initialize the book structure locally and push:

```bash
git clone https://github.com/Philippe-arnd/<book-repo>.git
cd <book-repo>

mkdir -p "Global Material" "Chapters material" "Review" "Changelog" "Current version"

# Fill in book-specific values before committing
cp /path/to/Ink-Gateway/templates/Config.yml "Global Material/Config.yml"

touch "Global Material/Outline.md"
touch "Global Material/Summary.md"
touch "Global Material/Lore.md"
touch "Global Material/Characters.md"
touch "Global Material/Style_guide.md"
touch "Review/current.md"

git add .
git commit -m "chore: initialize book structure"
git push origin main
```

### 4b. Create the `draft` branch

```bash
git checkout -b draft main
git push -u origin draft
```

> `engine.py` handles this automatically on first run. Doing it manually here confirms repo access is working.

### 4c. Clone into SilverBullet

Follow step 1c above so the book appears in the editor.

---

## 5. Register the OpenClaw Cron Job (per book)

```bash
openclaw cron add \
  --name "Ink: <Book Title>" \
  --cron "0 2 * * *" \
  --session isolated \
  --agent "ink-engine" \
  --thinking high \
  --message "Process book: https://github.com/Philippe-arnd/<book-repo>"
```

Verify:

```bash
openclaw cron list
# "Ink: <Book Title>" should appear, scheduled at 02:00 UTC
```

---

## Quick Reference

| Component | GitHub auth | How |
|---|---|---|
| SilverBullet | Fine-grained PAT | HTTPS credential store or SSH key in SilverBullet container |
| OpenClaw | Existing token + `gh` CLI | Already configured — no changes needed |

| Path | Purpose |
|---|---|
| `/data/ink-gateway/` | OpenClaw agent workspace root |
| `/data/ink-gateway/AGENTS.md` | `ink-engine` system prompt (Phase 3) |
| `/space/<book-repo>/` | Book as seen inside SilverBullet |
