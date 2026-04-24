# Domino for Codex

Domino turns recorded meetings into codebase-grounded implementation plans inside Codex. Audio and transcription stay local; planning and optional execution happen in your Codex session.
If you use Claude Code, go here: https://github.com/nitinm21/domino

## Install

1. Install the recorder binary on macOS Apple Silicon:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/nitinm21/domino-codex/main/install.sh | sh
   ```

   The Codex distribution installs `domino-codex-recorder`. That is intentional so it does not collide with any existing Domino recorder binary already on the same machine.
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

More information: https://domino-meet.vercel.app/

The flow is intentionally narrow:

1. Start recording with `$mstart`.
2. Stop with `$mstop`.
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
- **Execution is local.** Branch creation, edits, tests, and commits happen in your working copy. Domino will never open pull requests or push to a remote branch.



## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for local development, release, and plugin iteration notes.

## License

MIT — see [LICENSE](./LICENSE).
