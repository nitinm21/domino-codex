#!/usr/bin/env bash
set -euo pipefail

REPO="nitinm21/domino-codex"
BIN_NAME="domino-recorder"
INSTALL_DIR_PRIMARY="/usr/local/bin"
INSTALL_DIR_FALLBACK="${HOME}/.local/bin"

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || err "Domino currently supports macOS only."
[[ "$(uname -m)" == "arm64" ]] || err "Domino currently ships an arm64 binary only. Intel Mac users should build from source (see README)."

if ! xcode-select -p >/dev/null 2>&1; then
  err "Xcode Command Line Tools are required. Install with: xcode-select --install"
fi

if [[ -w "${INSTALL_DIR_PRIMARY}" ]]; then
  INSTALL_DIR="${INSTALL_DIR_PRIMARY}"
  SUDO=""
elif sudo -n true 2>/dev/null; then
  INSTALL_DIR="${INSTALL_DIR_PRIMARY}"
  SUDO="sudo"
else
  INSTALL_DIR="${INSTALL_DIR_FALLBACK}"
  SUDO=""
  mkdir -p "${INSTALL_DIR}"
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      log "Note: ${INSTALL_DIR} is not on your PATH. Add it to your shell profile:"
      log "    export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac
fi

if [[ -n "${DOMINO_VERSION:-}" ]]; then
  VERSION="${DOMINO_VERSION}"
  log "Using pinned version ${VERSION} (DOMINO_VERSION override)"
else
  log "Resolving latest release from github.com/${REPO}..."
  LATEST_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
  VERSION="$(printf '%s' "${LATEST_JSON}" | awk -F'"' '/"tag_name":/ {print $4; exit}')"
  [[ -n "${VERSION}" ]] || err "Could not resolve latest release tag. If only a pre-release is available, set DOMINO_VERSION=vX.Y.Z-rcN and re-run."
  log "Latest release: ${VERSION}"
fi

ASSET="${BIN_NAME}-${VERSION}-darwin-arm64.tar.gz"
SHA_ASSET="${ASSET}.sha256"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

log "Downloading ${ASSET}..."
curl -fsSL -o "${TMP}/${ASSET}" "${BASE_URL}/${ASSET}"
curl -fsSL -o "${TMP}/${SHA_ASSET}" "${BASE_URL}/${SHA_ASSET}"

log "Verifying SHA256..."
(cd "${TMP}" && shasum -a 256 -c "${SHA_ASSET}" >/dev/null) || err "SHA256 verification failed. Aborting."

log "Extracting..."
tar -xzf "${TMP}/${ASSET}" -C "${TMP}"
[[ -x "${TMP}/${BIN_NAME}" ]] || err "Extracted archive did not contain an executable ${BIN_NAME}."

xattr -d com.apple.quarantine "${TMP}/${BIN_NAME}" 2>/dev/null || true

log "Installing to ${INSTALL_DIR}/${BIN_NAME}..."
${SUDO} install -m 0755 "${TMP}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"

log "Verifying install..."
"${INSTALL_DIR}/${BIN_NAME}" --help >/dev/null 2>&1 || err "Installed binary failed to run. See README troubleshooting."

cat <<EOF

$(printf '\033[1;32m')Installed ${BIN_NAME} ${VERSION} to ${INSTALL_DIR}/${BIN_NAME}$(printf '\033[0m')

Next step:

    git clone https://github.com/${REPO}.git
    cd domino-codex
    codex

Inside Codex:

    Open /plugins
    Select the repo marketplace from .agents/plugins/marketplace.json
    Install Domino

Then record a meeting:

    \$domino:mstart
    ... hold the meeting ...
    \$domino:mstop

First-run notes:
  - macOS will prompt for Microphone and Screen Recording permissions on first use.
  - The Whisper model (~466 MB) downloads once to ~/.domino/models/ on first stop-and-transcribe run.
EOF
