#!/usr/bin/env bash
set -euo pipefail

REPO="Philippe-arnd/Ink-Gateway"
BIN="ink-cli"
DEST="${1:-/usr/local/bin}"

LATEST=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

TMP=$(mktemp)
curl -sSfL "https://github.com/${REPO}/releases/download/${LATEST}/${BIN}" -o "$TMP"
chmod +x "$TMP"
mv "$TMP" "${DEST}/${BIN}"

echo "ink-cli ${LATEST} installed → ${DEST}/${BIN}"
