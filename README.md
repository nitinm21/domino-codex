# Domino for Codex

Domino turns recorded meetings into codebase-grounded implementation plans inside Codex. Audio and transcription stay local; planning and optional execution happen in your Codex session.

## Install

1. Install the recorder binary on macOS Apple Silicon:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/nitinm21/domino-codex/main/install.sh | sh
   ```

   The Codex distribution installs `domino-codex-recorder`. That is intentional so it can coexist with a Claude-side `domino-recorder` already on the same machine.
   If the installer has to use `~/.local/bin`, it now updates your shell startup files automatically for future shells.

2. Register the Domino marketplace with Codex.

   If Codex is already installed, the installer will try to do this automatically. If it did not, run:

   ```bash
   codex marketplace add nitinm21/domino-codex --ref stable --sparse .agents/plugins --sparse plugins/domino
   ```

3. Open Codex, then open `/plugins` and install `Domino`.

   No repo clone is required for the normal install flow. The marketplace pulls the production plugin directly from GitHub.

4. Record a meeting:

   ```text
   $domino:mstart
   ... hold the meeting ...
   $domino:mstop
   ```

Depending on your Codex UI version, the installed commands may also appear as `$mstart`, `$mstat`, and `$mstop`.

## What It Does

Domino is built for working conversations that should turn into code, not notes. You talk through changes while the recorder captures your microphone and system audio, then Domino transcribes the session locally and grounds the resulting plan in the repository you opened in Codex.

The flow is intentionally narrow:

1. Start recording with `mstart`.
2. Stop with `mstop`.
3. Domino reads the saved transcript, explores the repo just enough to ground the discussion, and writes `plan.md` into the session directory.
4. You either revise the plan in plain chat or approve it for execution on a new branch.

The value is not the recording by itself. The value is getting from a spoken technical conversation to a structured, grounded starting point without manually reconstructing what was said.

## Requirements

- macOS 14+ on Apple Silicon.
- Xcode Command Line Tools installed via `xcode-select --install`.
- Codex installed and available as `codex`.
- Roughly 500 MB of free disk for the first-run Whisper model download.

## Privacy

- **Audio stays on the device.** The Opus recording lives under `~/.domino/recordings/` and is never uploaded by Domino.
- **Transcription is local.** Whisper runs on your machine; no audio leaves the device during transcription.
- **Planning uses your Codex session.** Transcript text and any repo files Codex reads to ground the plan are handled through your existing Codex session.
- **Execution is local.** Branch creation, edits, tests, and commits happen in your working copy. Domino does not push and does not open pull requests.

## Commands

- `$domino:mstart` or `$mstart` — start recording.
- `$domino:mstat` or `$mstat` — show the active session, or `{}` if idle.
- `$domino:mstop` or `$mstop` — stop, transcribe locally, write `plan.md`, and optionally execute the approved plan on a branch.

## Where Recordings Live

Each meeting gets its own directory under `~/.domino/recordings/<YYYY-MM-DD-HHMM>/` containing:

- `meeting.opus`
- `transcript.json`
- `recorder.log`
- `transcription.log`
- `plan.md` after synthesis succeeds

## Troubleshooting

- **`domino-codex-recorder: command not found` in your shell right after install.** If the installer used `~/.local/bin`, open a new terminal so your shell reloads the PATH block the installer wrote. Codex itself now checks `~/.local/bin`, `/opt/homebrew/bin`, and `/usr/local/bin` directly, so plugin commands do not depend on that shell refresh.
- **Domino does not appear in `/plugins`.** Run `codex marketplace add nitinm21/domino-codex --ref stable --sparse .agents/plugins --sparse plugins/domino`, restart Codex, then reopen `/plugins`.
- **`xcrun: error: invalid active developer path`** or missing Swift runtime libraries. Run `xcode-select --install`.
- **Claude already installed `domino-recorder`.** That is expected. Codex now uses `domino-codex-recorder`, so the two installs can coexist without sharing a PATH entry.
- **Gatekeeper blocks the binary.** The installer strips the quarantine attribute automatically. If you installed manually, run `xattr -d com.apple.quarantine /usr/local/bin/domino-codex-recorder` or the equivalent path under `~/.local/bin` or `/opt/homebrew/bin`.
- **Intel Mac.** This repo currently ships an arm64 release binary only. Intel users should build from source with `cargo build --release --manifest-path recorder/Cargo.toml`.
- **Codex marketplace conflict.** If Codex says `domino-codex` is already registered from a different source or ref, remove the `[marketplaces.domino-codex]` block from `~/.codex/config.toml`, rerun the marketplace add command above, then reopen Codex.

## Repo Layout

- `plugins/domino/` — canonical Codex plugin surface for this repository
- `.agents/plugins/marketplace.json` — marketplace metadata used for local development and git-backed Codex installs
- `recorder/` — Rust recorder and local transcription pipeline
- `plugin/` — retained Claude Code plugin files for reference and transition; not the primary install surface for this repo

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for local development, release, and plugin iteration notes.

## License

MIT — see [LICENSE](./LICENSE).
