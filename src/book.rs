use anyhow::{anyhow, Context, Result};
use std::path::Path;

use crate::git;

// ─── Prose utilities ───────────────────────────────────────────────────────────

/// Count prose words, ignoring HTML comment lines (e.g. `<!-- PAGE N -->`).
pub fn count_prose_words(content: &str) -> u32 {
    content
        .lines()
        .filter(|l| !l.trim_start().starts_with("<!--"))
        .flat_map(|l| l.split_whitespace())
        .count() as u32
}

// ─── apply-format ──────────────────────────────────────────────────────────────

/// Apply structural format patches to `Full_Book.md`:
/// - `prepend`: text inserted after the managed-file header comment
/// - `insert_headings`: each entry `{ before_anchor, heading }` inserts a heading line
///   before the first line containing `before_anchor` as a substring
///
/// Commits and pushes `Full_Book.md` on success.
pub fn apply_format_patch(repo: &Path, patch: serde_json::Value) -> Result<serde_json::Value> {
    // Guard: cannot patch a sealed book
    if repo.join("COMPLETE").exists() {
        return Err(anyhow!(
            "book already complete — format patches cannot be applied after sealing"
        ));
    }

    let book_path = repo.join("Current version").join("Full_Book.md");
    if !book_path.exists() {
        return Err(anyhow!("Full_Book.md does not exist — nothing to patch"));
    }

    let mut content = std::fs::read_to_string(&book_path)
        .with_context(|| "Failed to read Full_Book.md for format patch")?;

    let mut patches_applied: u32 = 0;
    let mut warnings: Vec<String> = Vec::new();

    // ── Apply prepend ─────────────────────────────────────────────────────────
    if let Some(prepend) = patch.get("prepend").and_then(|v| v.as_str()) {
        if !prepend.is_empty() {
            // Find the end of the first `-->` (managed-file header closing tag)
            let after_header = content
                .find("-->")
                .map(|pos| pos + "-->".len())
                .unwrap_or(0);
            // Skip one trailing newline if present so our insertion follows naturally
            let insert_pos = if content[after_header..].starts_with('\n') {
                after_header + 1
            } else {
                after_header
            };
            content.insert_str(insert_pos, &format!("\n{}", prepend));
            patches_applied += 1;
        }
    }

    // ── Apply insert_headings ─────────────────────────────────────────────────
    if let Some(inserts) = patch.get("insert_headings").and_then(|v| v.as_array()) {
        for entry in inserts {
            let before_anchor = match entry.get("before_anchor").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => {
                    warnings.push("insert_headings entry missing 'before_anchor'".to_string());
                    continue;
                }
            };
            let heading = match entry.get("heading").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => {
                    warnings.push("insert_headings entry missing 'heading'".to_string());
                    continue;
                }
            };

            // Find the byte position of the anchor, then walk back to the line start
            if let Some(anchor_pos) = content.find(before_anchor) {
                let line_start = content[..anchor_pos]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let heading_with_nl = if heading.ends_with('\n') {
                    heading.to_string()
                } else {
                    format!("{}\n\n", heading)
                };
                content.insert_str(line_start, &heading_with_nl);
                patches_applied += 1;
            } else {
                warnings.push(format!("before_anchor not found: '{before_anchor}'"));
            }
        }
    }

    // Write the modified file
    std::fs::write(&book_path, &content).with_context(|| "Failed to write patched Full_Book.md")?;

    // Commit and push
    git::run_git(repo, &["add", "Current version/Full_Book.md"])
        .with_context(|| "Failed to git add Full_Book.md")?;
    git::run_git(repo, &["commit", "-m", "fmt: apply format corrections"])
        .with_context(|| "Failed to commit format corrections")?;
    git::run_git(repo, &["push", "origin", "main"])
        .with_context(|| "Failed to push format corrections")?;

    Ok(serde_json::json!({
        "status": "applied",
        "patches_applied": patches_applied,
        "warnings": warnings,
    }))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_words_ignores_html_comment_lines() {
        let content = "Hello world\n<!-- PAGE 1 -->\nFoo bar baz";
        assert_eq!(count_prose_words(content), 5);
    }

    #[test]
    fn count_words_empty_input() {
        assert_eq!(count_prose_words(""), 0);
    }
}
