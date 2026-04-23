# Domino — Claude Code plugin

Record a meeting, get a codebase-grounded implementation plan, optionally execute it on a new branch. Audio and transcription stay on the device; synthesis and execution run on your Claude Code session.

## Plugin conventions (verified 2026-04-16)

- Manifest path: `.claude-plugin/plugin.json` (only the manifest lives inside `.claude-plugin/`; commands and other components sit at the plugin root).
- Only `name` is strictly required in `plugin.json`; `description`, `version`, `author` are recommended.
- Slash commands: flat `.md` files under `commands/`. YAML frontmatter supports `description`, `argument-hint`, `allowed-tools`, `model`, and a few others.
- Load locally with `claude --plugin-dir ./plugin`. Reload during a session with `/reload-plugins`.

## Prerequisites

- macOS (Apple Silicon recommended).
- The release recorder binary built at `./recorder/target/release/domino-recorder` from the repo root, or `domino-recorder` otherwise available on `PATH`. See `so_far.md` §13.10 for the one-time manual install (binary location, Whisper model, runtime env vars).
- A Claude Code session running in the git repository the meeting is about.

## Commands

- `/mstart` — start a recording session. The command resolves `./recorder/target/release/domino-recorder` before falling back to `PATH`. Prints session JSON (pid, session_dir, started_at). The recorder daemonizes; your terminal is free again immediately.
- `/mstat` — show the current session JSON, or `{}` if idle. The command resolves `./recorder/target/release/domino-recorder` before falling back to `PATH`.
- `/mstop` — stop the recording, run local transcription, synthesize an implementation plan grounded in this repo, present a summary, and — on explicit `execute` — apply the plan on a new git branch named `domino/meeting-<YYYY-MM-DD-HHMM>-<slug>` (timestamp matches the session dir, slug is a 2–5-word kebab-case summary of the headline decision). One plan item = one commit. The command resolves `./recorder/target/release/domino-recorder` before falling back to `PATH`. The plugin never pushes and never opens a PR.

## Installation / loading

For local development and testing:

```
claude --plugin-dir /path/to/domino/plugin
```

Inside a Claude Code session you can reload after edits with `/reload-plugins`.

## After `/mstop`

Stay in the same thread for plan revisions and `execute`. `/mstop` is intentionally a multi-turn workflow, not a one-shot command.

On `execute`, the plugin first requires a clean working tree, then creates `domino/meeting-<YYYY-MM-DD-HHMM>-<slug>`, runs relevant tests as it walks the approved plan, and makes one commit per executed plan item.

If a test fails or a later item turns out to be unsafe or underspecified, execution stops on that branch, keeps the earlier passing commits, skips committing the failing or deferred item, and appends an `Execution outcome` section to the saved session `plan.md`.

Requests to push, force-push, or open a PR are refused. Review and any push remain manual.

## Session artifacts

Every meeting gets its own directory under `~/.domino/recordings/<YYYY-MM-DD-HHMM>/` containing the Opus audio, the recorder log, the transcript JSON, and (after synthesis) `plan.md`. Nothing is sent off-device except the transcript text that your Claude Code session carries to Anthropic during synthesis.

## Privacy

- **Audio stays on the device.** The Opus file lives under `~/.domino/recordings/` and is never uploaded anywhere by this plugin.
- **Transcription is local.** Whisper runs on your machine via the bundled model; no audio leaves the device during transcription.
- **Synthesis uses your Claude Code session.** During `/mstop`, the transcript text (and any repo files Claude reads to ground the plan) is sent to Anthropic via your existing Claude Code subscription. Treat the transcript the same way you treat anything you paste into Claude Code.
- **Execution is local.** Branch creation, edits, tests, and commits all happen in your working copy. The plugin never runs `git push` and never opens a PR.
