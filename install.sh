#!/usr/bin/env bash
set -euo pipefail

REPO="nitinm21/domino-codex"
BIN_NAME="domino-codex-recorder"
MARKETPLACE_REF="stable"
MARKETPLACE_LABEL="Domino"
INSTALL_DIR_PRIMARY="/usr/local/bin"
INSTALL_DIR_HOMEBREW="/opt/homebrew/bin"
INSTALL_DIR_FALLBACK="${HOME}/.local/bin"
PATH_BLOCK_BEGIN="# >>> domino-codex-recorder >>>"
PATH_BLOCK_END="# <<< domino-codex-recorder <<<"
PATH_BLOCK=$'# >>> domino-codex-recorder >>>\nif [ -d "$HOME/.local/bin" ]; then\n  case ":$PATH:" in\n    *":$HOME/.local/bin:"*) ;;\n    *) export PATH="$HOME/.local/bin:$PATH" ;;\n  esac\nfi\n# <<< domino-codex-recorder <<<'

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarn:\033[0m %s\n' "$*" >&2; }
err() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
dir_on_path() { case ":${PATH}:" in *":$1:"*) return 0 ;; *) return 1 ;; esac; }

ensure_path_block() {
  local file="$1"
  mkdir -p "$(dirname "${file}")"
  touch "${file}"
  if grep -Fq "${PATH_BLOCK_BEGIN}" "${file}"; then
    return 1
  fi
  printf '\n%s\n' "${PATH_BLOCK}" >> "${file}"
  return 0
}

configure_shell_path_for_fallback() {
  local files=("${HOME}/.zprofile" "${HOME}/.zshrc" "${HOME}/.profile")
  local file
  PATH_SETUP_FILES=()
  for file in "${files[@]}"; do
    if ensure_path_block "${file}"; then
      PATH_SETUP_FILES+=("${file}")
    fi
  done
}

[[ "$(uname -s)" == "Darwin" ]] || err "Domino currently supports macOS only."
[[ "$(uname -m)" == "arm64" ]] || err "Domino currently ships an arm64 binary only. Intel Mac users should build from source (see README)."

if ! xcode-select -p >/dev/null 2>&1; then
  err "Xcode Command Line Tools are required. Install with: xcode-select --install"
fi

PATH_SETUP_FILES=()
PATH_SETUP_FILES_DISPLAY=""
INSTALL_DIR_PATH_SETUP="none"

if dir_on_path "${INSTALL_DIR_PRIMARY}" && [[ -w "${INSTALL_DIR_PRIMARY}" ]]; then
  INSTALL_DIR="${INSTALL_DIR_PRIMARY}"
  SUDO=""
elif dir_on_path "${INSTALL_DIR_HOMEBREW}" && [[ -w "${INSTALL_DIR_HOMEBREW}" ]]; then
  INSTALL_DIR="${INSTALL_DIR_HOMEBREW}"
  SUDO=""
elif dir_on_path "${INSTALL_DIR_PRIMARY}" && sudo -n true 2>/dev/null; then
  INSTALL_DIR="${INSTALL_DIR_PRIMARY}"
  SUDO="sudo"
elif dir_on_path "${INSTALL_DIR_FALLBACK}"; then
  INSTALL_DIR="${INSTALL_DIR_FALLBACK}"
  SUDO=""
  mkdir -p "${INSTALL_DIR}"
else
  INSTALL_DIR="${INSTALL_DIR_FALLBACK}"
  SUDO=""
  mkdir -p "${INSTALL_DIR}"
  configure_shell_path_for_fallback
  if [[ ${#PATH_SETUP_FILES[@]} -gt 0 ]]; then
    INSTALL_DIR_PATH_SETUP="updated"
    PATH_SETUP_FILES_DISPLAY="$(printf '    %s\n' "${PATH_SETUP_FILES[@]}")"
  else
    INSTALL_DIR_PATH_SETUP="already-configured"
  fi
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
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx \
  "${INSTALL_DIR}/${BIN_NAME}" --help >/dev/null 2>&1 \
  || err "Installed binary failed to run. See README troubleshooting."

CODEX_MARKETPLACE_CMD=()
CODEX_MARKETPLACE_CMD_DISPLAY=""
MARKETPLACE_STATUS="skipped"
MARKETPLACE_NOTE=""

if command -v codex >/dev/null 2>&1; then
  if codex marketplace add --help >/dev/null 2>&1; then
    CODEX_MARKETPLACE_CMD=(
      codex marketplace add "${REPO}" --ref "${MARKETPLACE_REF}"
      --sparse .agents/plugins --sparse plugins/domino
    )
    CODEX_MARKETPLACE_CMD_DISPLAY="codex marketplace add ${REPO} --ref ${MARKETPLACE_REF} --sparse .agents/plugins --sparse plugins/domino"
  elif codex plugin marketplace add --help >/dev/null 2>&1; then
    CODEX_MARKETPLACE_CMD=(
      codex plugin marketplace add "${REPO}" --ref "${MARKETPLACE_REF}"
      --sparse .agents/plugins --sparse plugins/domino
    )
    CODEX_MARKETPLACE_CMD_DISPLAY="codex plugin marketplace add ${REPO} --ref ${MARKETPLACE_REF} --sparse .agents/plugins --sparse plugins/domino"
  else
    MARKETPLACE_STATUS="unsupported-codex"
    MARKETPLACE_NOTE="Codex is installed, but this Codex version does not expose marketplace registration from the terminal."
  fi
else
  MARKETPLACE_STATUS="codex-missing"
  CODEX_MARKETPLACE_CMD_DISPLAY="codex marketplace add ${REPO} --ref ${MARKETPLACE_REF} --sparse .agents/plugins --sparse plugins/domino"
  MARKETPLACE_NOTE="Codex is not installed yet, so marketplace registration was skipped."
fi

if [[ ${#CODEX_MARKETPLACE_CMD[@]} -gt 0 ]]; then
  log "Registering ${MARKETPLACE_LABEL} marketplace with Codex..."
  set +e
  MARKETPLACE_OUTPUT="$("${CODEX_MARKETPLACE_CMD[@]}" 2>&1)"
  MARKETPLACE_EXIT=$?
  set -e

  if [[ ${MARKETPLACE_EXIT} -eq 0 ]]; then
    if [[ "${MARKETPLACE_OUTPUT}" == *"already added"* ]]; then
      MARKETPLACE_STATUS="already-configured"
    else
      MARKETPLACE_STATUS="registered"
    fi
  elif [[ "${MARKETPLACE_OUTPUT}" == *"already added from a different source"* ]]; then
    MARKETPLACE_STATUS="conflict"
    MARKETPLACE_NOTE="Codex already has a Domino marketplace configured from a different source or ref."
  else
    MARKETPLACE_STATUS="failed"
    MARKETPLACE_NOTE="${MARKETPLACE_OUTPUT}"
  fi
fi

cat <<EOF

$(printf '\033[1;32m')Installed ${BIN_NAME} ${VERSION} to ${INSTALL_DIR}/${BIN_NAME}$(printf '\033[0m')

This Codex install intentionally uses ${BIN_NAME} so it can coexist with any
Claude-side domino-recorder already on your PATH.
EOF

if [[ "${INSTALL_DIR}" == "${INSTALL_DIR_FALLBACK}" ]]; then
  if [[ "${INSTALL_DIR_PATH_SETUP}" == "updated" ]]; then
    cat <<EOF

Shell PATH:
  Added ${INSTALL_DIR} to future shells via:
${PATH_SETUP_FILES_DISPLAY}

  Open a new terminal before running bare \`${BIN_NAME}\` commands from the shell.
  Codex itself also checks ${INSTALL_DIR}/${BIN_NAME} directly, so plugin commands
  do not depend on a PATH reload.
EOF
  else
    cat <<EOF

Shell PATH:
  ${INSTALL_DIR} was already present on this machine's PATH or shell setup.
  If your current shell still reports \`${BIN_NAME}: command not found\`, open a new
  terminal and retry. Codex itself also checks ${INSTALL_DIR}/${BIN_NAME} directly.
EOF
  fi
fi

case "${MARKETPLACE_STATUS}" in
  registered)
    cat <<EOF

Codex marketplace:
  Registered ${MARKETPLACE_LABEL} from ${REPO}#${MARKETPLACE_REF}.

Next step in Codex:
    Open /plugins
    Install Domino
EOF
    ;;
  already-configured)
    cat <<EOF

Codex marketplace:
  ${MARKETPLACE_LABEL} is already registered with Codex from ${REPO}#${MARKETPLACE_REF}.

Next step in Codex:
    Open /plugins
    Install or upgrade Domino
EOF
    ;;
  codex-missing|unsupported-codex)
    cat <<EOF

Codex marketplace:
  ${MARKETPLACE_NOTE}

After you install Codex, run:
    ${CODEX_MARKETPLACE_CMD_DISPLAY}

Then in Codex:
    Open /plugins
    Install Domino
EOF
    ;;
  conflict)
    cat <<EOF

Codex marketplace:
  ${MARKETPLACE_NOTE}

To switch this machine to the production Domino marketplace, remove the
[marketplaces.domino-codex] block from ~/.codex/config.toml, then run:
    ${CODEX_MARKETPLACE_CMD_DISPLAY}

Then in Codex:
    Open /plugins
    Install Domino
EOF
    ;;
  failed)
    warn "Automatic Codex marketplace registration failed."
    cat <<EOF

Codex marketplace:
  Automatic registration failed. Re-run this command manually:
    ${CODEX_MARKETPLACE_CMD_DISPLAY}

Registration output:
${MARKETPLACE_NOTE}

Then in Codex:
    Open /plugins
    Install Domino
EOF
    ;;
esac

cat <<EOF

Then record a meeting:

    \$domino:mstart
    ... hold the meeting ...
    \$domino:mstop

First-run notes:
  - No repo clone is required for the normal install flow.
  - macOS will prompt for Microphone and Screen Recording permissions on first use.
  - The Whisper model (~466 MB) downloads once to ~/.domino/models/ on first stop-and-transcribe run.
EOF
