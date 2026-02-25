#!/usr/bin/env bash
set -euo pipefail

REPO="Philippe-arnd/Ink-Gateway"
DEST="${1:-$HOME/.local/bin}"

LATEST=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

mkdir -p "${DEST}"

for BIN in ink-cli ink-gateway-mcp; do
  TMP=$(mktemp)
  curl -sSfL "https://github.com/${REPO}/releases/download/${LATEST}/${BIN}" -o "$TMP"
  chmod +x "$TMP"
  mv "$TMP" "${DEST}/${BIN}"
  echo "${BIN} ${LATEST} installed → ${DEST}/${BIN}"
done

echo ""
echo "Register the MCP server with your AI client:"
echo "  claude mcp add ink-gateway -- ${DEST}/ink-gateway-mcp"
echo "  # or for Gemini CLI:"
echo "  gemini mcp add ink-gateway -- ${DEST}/ink-gateway-mcp"
