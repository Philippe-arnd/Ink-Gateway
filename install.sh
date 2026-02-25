#!/usr/bin/env bash
set -euo pipefail

REPO="Philippe-arnd/Ink-Gateway"
DEST="${1:-$HOME/.local/bin}"

# ── Detect platform ───────────────────────────────────────────────────────────

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "${OS}-${ARCH}" in
  linux-x86_64)          TARGET="x86_64-unknown-linux-musl" ;;
  linux-aarch64|linux-arm64) TARGET="aarch64-unknown-linux-musl" ;;
  darwin-x86_64)         TARGET="x86_64-apple-darwin" ;;
  darwin-arm64)          TARGET="aarch64-apple-darwin" ;;
  *)
    echo "Unsupported platform: ${OS}-${ARCH}"
    echo "Build from source: https://github.com/${REPO}"
    exit 1
    ;;
esac

# ── Resolve latest release ────────────────────────────────────────────────────

LATEST=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

BASE_URL="https://github.com/${REPO}/releases/download/${LATEST}"

mkdir -p "${DEST}"
TMP=$(mktemp -d)
trap 'rm -rf "${TMP}"' EXIT

# ── Download checksums ────────────────────────────────────────────────────────

echo "Fetching checksums for ${LATEST} (${TARGET})..."
curl -sSfL "${BASE_URL}/checksums.sha256" -o "${TMP}/checksums.sha256"

# ── Download and verify each binary ──────────────────────────────────────────

verify_checksum() {
  local file="$1"
  local dir="$2"
  if command -v sha256sum > /dev/null 2>&1; then
    grep "${file}" "${TMP}/checksums.sha256" | (cd "${dir}" && sha256sum --check --status)
  elif command -v shasum > /dev/null 2>&1; then
    grep "${file}" "${TMP}/checksums.sha256" | (cd "${dir}" && shasum -a 256 --check --status)
  else
    echo "Warning: sha256sum/shasum not found — skipping checksum verification"
    return 0
  fi
}

for BIN in ink-cli ink-gateway-mcp; do
  REMOTE="${BIN}-${TARGET}"
  echo "Downloading ${BIN}..."
  curl -sSfL "${BASE_URL}/${REMOTE}" -o "${TMP}/${REMOTE}"

  echo "Verifying ${BIN}..."
  if ! verify_checksum "${REMOTE}" "${TMP}"; then
    echo "Checksum mismatch for ${REMOTE} — aborting"
    exit 1
  fi

  chmod +x "${TMP}/${REMOTE}"
  mv "${TMP}/${REMOTE}" "${DEST}/${BIN}"
  echo "  ${BIN} ${LATEST} → ${DEST}/${BIN}"
done

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
echo "Register the MCP server with your AI client:"
echo "  claude mcp add ink-gateway -- ${DEST}/ink-gateway-mcp"
echo "  gemini mcp add ink-gateway -- ${DEST}/ink-gateway-mcp"
