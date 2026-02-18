"""
engine.py — Ink Gateway Orchestrator (Phase 1)

Invocation:
    python engine.py https://github.com/Philippe-arnd/<book-repo>

Phase 1 scope: Git operations + payload builder.
AI generation is wired in Phase 2.
"""

import argparse
import logging
import subprocess
import sys
from datetime import date
from pathlib import Path

import yaml

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

BOOKS_ROOT = Path("/data/ink-gateway/books")

DEFAULTS = {
    "model": "claude-opus-4-6",
    "target_length": 90000,
    "chapter_count": 30,
    "chapter_structure": "linear",
    "nightly_output_target": 1500,
}

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s  %(levelname)-8s  %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger("ink-engine")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _run(cmd: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess:
    """Run a subprocess, streaming output to the log."""
    log.info("$ %s", " ".join(str(c) for c in cmd))
    result = subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        log.info(result.stdout.strip())
    if result.returncode != 0:
        if result.stderr.strip():
            log.error(result.stderr.strip())
        if check:
            raise RuntimeError(
                f"Command failed (exit {result.returncode}): {' '.join(str(c) for c in cmd)}"
            )
    return result


def _count_words(text: str) -> int:
    return len(text.split())


# ---------------------------------------------------------------------------
# Core functions
# ---------------------------------------------------------------------------


def parse_args() -> argparse.Namespace:
    """Parse CLI arguments. Accepts the GitHub repo URL passed by OpenClaw."""
    parser = argparse.ArgumentParser(
        description="Ink Gateway nightly orchestrator.",
        epilog="Example: python engine.py https://github.com/Philippe-arnd/my-novel",
    )
    parser.add_argument(
        "repo_url",
        help="GitHub repository URL for the book to process.",
    )
    return parser.parse_args()


def book_name_from_url(url: str) -> str:
    """Extract the repository name from the tail of a GitHub URL.

    >>> book_name_from_url("https://github.com/Philippe-arnd/my-novel")
    'my-novel'
    """
    return url.rstrip("/").split("/")[-1]


def ensure_repo(url: str, book_dir: Path) -> None:
    """Clone the repo if it doesn't exist; fetch latest refs if it does."""
    if book_dir.exists():
        log.info("Repo already cloned at %s — fetching.", book_dir)
        _run(["git", "fetch", "--all", "--tags"], cwd=book_dir)
    else:
        log.info("Cloning %s → %s", url, book_dir)
        book_dir.parent.mkdir(parents=True, exist_ok=True)
        _run(["git", "clone", url, str(book_dir)], cwd=book_dir.parent)


def load_config(book_dir: Path) -> dict:
    """Load Global Material/Config.yml and merge with defaults."""
    config_path = book_dir / "Global Material" / "Config.yml"
    config = dict(DEFAULTS)
    if config_path.exists():
        with config_path.open() as fh:
            overrides = yaml.safe_load(fh) or {}
        config.update({k: v for k, v in overrides.items() if k in DEFAULTS})
        log.info("Config loaded from %s", config_path)
    else:
        log.warning("Config.yml not found — using defaults.")
    log.info("Config: %s", config)
    return config


def detect_human_edits(book_dir: Path) -> list[Path]:
    """Return list of files modified today (by mtime), excluding .git."""
    today = date.today()
    modified = []
    for path in book_dir.rglob("*"):
        if ".git" in path.parts:
            continue
        if not path.is_file():
            continue
        if date.fromtimestamp(path.stat().st_mtime) == today:
            modified.append(path)
    if modified:
        log.info("Human edits detected today (%d file(s)):", len(modified))
        for p in modified:
            log.info("  %s", p.relative_to(book_dir))
    else:
        log.info("No human edits detected today.")
    return modified


def commit_human_edits(book_dir: Path, files: list[Path]) -> None:
    """Stage, commit, and push human edits to main."""
    _run(["git", "add", "."], cwd=book_dir)
    _run(
        ["git", "commit", "-m", "chore: human updates"],
        cwd=book_dir,
    )
    _run(["git", "push", "origin", "main"], cwd=book_dir)
    log.info("Human edits committed and pushed to main.")


def snapshot(book_dir: Path) -> str:
    """Create a pre-nightly git tag and push it."""
    tag = f"pre-nightly-{date.today().isoformat()}"
    result = _run(
        ["git", "tag", tag],
        cwd=book_dir,
        check=False,
    )
    if result.returncode != 0 and "already exists" in (result.stderr or ""):
        log.warning("Tag %s already exists — skipping.", tag)
    else:
        _run(["git", "push", "origin", tag], cwd=book_dir)
        log.info("Snapshot tag created: %s", tag)
    return tag


def branch_for_draft(book_dir: Path) -> None:
    """Ensure the draft branch exists, then check it out and rebase onto main."""
    # Check if draft branch exists remotely or locally
    result = _run(
        ["git", "branch", "--list", "draft"],
        cwd=book_dir,
        check=False,
    )
    draft_exists_locally = bool(result.stdout.strip())

    remote_result = _run(
        ["git", "ls-remote", "--heads", "origin", "draft"],
        cwd=book_dir,
        check=False,
    )
    draft_exists_remotely = bool(remote_result.stdout.strip())

    if not draft_exists_locally and not draft_exists_remotely:
        log.info("Creating draft branch from main.")
        _run(["git", "checkout", "-b", "draft", "main"], cwd=book_dir)
        _run(["git", "push", "-u", "origin", "draft"], cwd=book_dir)
    else:
        _run(["git", "checkout", "draft"], cwd=book_dir)
        _run(["git", "rebase", "main"], cwd=book_dir)

    log.info("On draft branch, rebased onto main.")


def build_payload(book_dir: Path, config: dict) -> dict:
    """Read context files and assemble the payload for the AI generation step.

    Returns a dict with:
        - context_sections: list of {"path": str, "content": str}
        - word_count: int  (total words in latest Full_Book_*.md, or 0)
        - config: dict
    """
    context_sections = []

    # --- Global Material (all .md files) ---
    global_dir = book_dir / "Global Material"
    if global_dir.exists():
        for md_file in sorted(global_dir.glob("*.md")):
            text = md_file.read_text(encoding="utf-8")
            context_sections.append(
                {"path": str(md_file.relative_to(book_dir)), "content": text}
            )
            log.info("Loaded: %s (%d words)", md_file.name, _count_words(text))

    # --- Chapters material (all .md files, recursively) ---
    chapters_dir = book_dir / "Chapters material"
    if chapters_dir.exists():
        for md_file in sorted(chapters_dir.rglob("*.md")):
            text = md_file.read_text(encoding="utf-8")
            context_sections.append(
                {"path": str(md_file.relative_to(book_dir)), "content": text}
            )
            log.info("Loaded: %s (%d words)", md_file.relative_to(book_dir), _count_words(text))

    # --- Review/current.md ---
    current_md = book_dir / "Review" / "current.md"
    if current_md.exists():
        text = current_md.read_text(encoding="utf-8")
        context_sections.append(
            {"path": str(current_md.relative_to(book_dir)), "content": text}
        )
        log.info("Loaded: Review/current.md (%d words)", _count_words(text))
    else:
        log.warning("Review/current.md not found — starting fresh.")

    # --- Word count from latest Full_Book_*.md ---
    word_count = 0
    current_version_dir = book_dir / "Current version"
    if current_version_dir.exists():
        candidates = sorted(current_version_dir.glob("Full_Book_*.md"))
        if candidates:
            latest = candidates[-1]
            text = latest.read_text(encoding="utf-8")
            word_count = _count_words(text)
            log.info(
                "Manuscript word count: %d (from %s)", word_count, latest.name
            )

    payload = {
        "context_sections": context_sections,
        "word_count": word_count,
        "config": config,
    }
    log.info(
        "Payload built: %d context section(s), %d manuscript words.",
        len(context_sections),
        word_count,
    )
    return payload


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    args = parse_args()
    repo_url = args.repo_url

    book_name = book_name_from_url(repo_url)
    book_dir = BOOKS_ROOT / book_name

    log.info("=" * 60)
    log.info("Ink Gateway — nightly session starting")
    log.info("Book: %s", book_name)
    log.info("Dir:  %s", book_dir)
    log.info("=" * 60)

    # Step 1: Ensure the repo is present and up to date
    ensure_repo(repo_url, book_dir)

    # Step 2: Load config (needed for payload later)
    config = load_config(book_dir)

    # Step 3: Pre-flight — detect and commit human edits
    edited_files = detect_human_edits(book_dir)
    if edited_files:
        commit_human_edits(book_dir, edited_files)

    # Step 4: Snapshot
    snapshot(book_dir)

    # Step 5: Branch for draft
    branch_for_draft(book_dir)

    # Step 6: Build context payload (Phase 2 will pass this to the AI)
    payload = build_payload(book_dir, config)

    log.info("=" * 60)
    log.info("Phase 1 complete. Payload ready for AI generation (Phase 2).")
    log.info(
        "  Sections: %d | Manuscript words: %d | Nightly target: %d",
        len(payload["context_sections"]),
        payload["word_count"],
        config["nightly_output_target"],
    )
    log.info("=" * 60)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        log.error("Fatal error: %s", exc)
        sys.exit(1)
