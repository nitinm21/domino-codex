# Domino Codex Plugin

Domino for Codex packages the existing Domino meeting workflow as a Codex plugin.

The canonical repository for this Codex plugin is `https://github.com/nitinm21/domino-codex`.

This plugin is the Codex counterpart to the Claude Code plugin in `plugin/`. It does not replace the Claude implementation. Both plugin surfaces are intended to coexist in this repository.

## What This Plugin Does

The final Codex workflow matches the existing Claude workflow:

- Start a meeting recording.
- Check active meeting status.
- Stop the meeting, transcribe locally, write `plan.md`, and optionally execute the approved plan on a new branch.

The Codex port now includes all three bundled skills: `$mstart`, `$mstat`, and `$mstop`.

## Prerequisites

- macOS
- `domino-recorder` available on your `PATH`
- A Codex session opened in the git repository the meeting is about

The bundled `$mstart`, `$mstat`, and `$mstop` skills already inject the required Swift runtime fallback path when they call `domino-recorder`.

If you run `domino-recorder` manually in your shell on this machine, it may still require:

```bash
export DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
```

## Local Install

This repository now exposes a repo-scoped plugin marketplace at `.agents/plugins/marketplace.json`.

To install the local Domino plugin:

1. Launch Codex from the repository root that contains the marketplace file:

```bash
cd /Users/nitin/domino-codex/domino
codex
```

2. Restart Codex so it reloads the repo marketplace.
3. Open `/plugins`.
4. Select the `Domino Local Plugins` marketplace.
5. Install `Domino`.

If you launch Codex from `/Users/nitin/domino-codex` instead, it will not see the repo marketplace because that directory does not contain `.agents/plugins/marketplace.json`.

After plugin or marketplace changes, restart Codex again so the local install picks up the new files.

## Manual Verification

Use this sequence after reinstalling the plugin:

1. Build the release recorder binary if you have not already:

```bash
cd /Users/nitin/domino-codex/domino
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk cargo build --release --manifest-path recorder/Cargo.toml
```

2. Put the release binary on your shell `PATH` for the current terminal:

```bash
cd /Users/nitin/domino-codex/domino
export PATH="$PWD/recorder/target/release:$PATH"
which domino-recorder
```

Expected result:

- `which domino-recorder` prints `/Users/nitin/domino-codex/domino/recorder/target/release/domino-recorder`

3. Launch Codex from the actual repo root:

```bash
cd /Users/nitin/domino-codex/domino
codex
```

4. In Codex, open `/plugins`.
5. Reinstall `Domino` so Codex refreshes the local plugin files.
6. Start a new thread in that same Codex session.
7. Invoke `$mstart`.

Expected result:

- Codex runs the command with `DYLD_FALLBACK_LIBRARY_PATH=...`
- The command succeeds instead of failing with `libswift_Concurrency.dylib`
- Codex returns JSON containing `pid`, `session_dir`, and `started_at`
- If you see `objc[...]` duplicate Swift runtime warnings but also get valid JSON, treat that as a pass for this phase

8. Invoke `$mstat`.

Expected result:

- Codex returns the current session JSON
- The `session_dir` matches the one from `$mstart`
- If warnings appear above valid JSON, treat the JSON as authoritative

9. In a normal terminal, stop the recorder manually:

```bash
cd /Users/nitin/domino-codex/domino
export PATH="$PWD/recorder/target/release:$PATH"
export DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
domino-recorder stop
```

Expected result:

- The recorder stops cleanly
- The command emits the stop/transcription JSON

10. Go back to Codex and invoke `$mstat` again.

Expected result:

- Codex returns `{}` or equivalent empty session output
- There is no active recorder session anymore

11. Start another short recording with `$mstart`, speak a short repo-grounded test meeting, then invoke `$mstop` in the same thread.

Expected result:

- Codex streams the recorder stop and transcription output through unchanged
- Codex writes `<session-dir>/plan.md`
- Codex prints `Plan written: <session-dir>/plan.md` with up to three summary bullets

12. Reply in the same thread with a plan revision, such as "also add a regression test" or "do not touch `plugin/README.md`".

Expected result:

- Codex reads the existing `plan.md`
- Codex updates only the requested parts of the plan
- Codex reprints the inline summary without executing

13. Reply `execute` in that same thread.

Expected result:

- Codex checks for a clean working tree before making changes
- Codex creates `domino/meeting-<timestamp>-<slug>`
- Codex makes one commit per plan item it executes
- Codex runs relevant tests before advancing to the next item
- Codex does not push and does not open a pull request

14. If Codex loses context after `$mstop`, point it back at the saved `<session-dir>/plan.md` explicitly and continue in the same thread if possible.

If step 7 still shows the old bare `domino-recorder start` command, or if `$mstop` still behaves like an older cached copy, Codex is using a cached plugin install. Restart Codex, reinstall `Domino`, and repeat from step 6.

If `domino-recorder start` required out-of-sandbox approval, approve the same retry pattern for `$mstat` or `$mstop` if Codex asks. A sandboxed `status` or `stop` can misreport `stale PID file detected` even while the recorder daemon is still running outside the sandbox.

## Skill Invocations

Available now:

- `$mstart`
- `$mstat`
- `$mstop`

You can also invoke the plugin explicitly via `@domino`.

## After `$mstop`

Stay in the same thread for plan revisions and `execute`. `$mstop` is intentionally a multi-turn workflow, not a one-shot command.

The plugin writes planning artifacts into the Domino session directory, not into repo-scoped temp files. If the thread loses context, point Codex back at the saved `plan.md` before resuming execution or revisions.

The saved session `plan.md` is the source of truth if thread context is lost.

`execute` never pushes and never opens a pull request.

## Relationship To The Claude Plugin

The existing Claude Code implementation remains at:

- `plugin/.claude-plugin/plugin.json`
- `plugin/commands/mstart.md`
- `plugin/commands/mstat.md`
- `plugin/commands/mstop.md`

The Codex port lives separately under:

- `plugins/domino/.codex-plugin/plugin.json`
- `plugins/domino/skills/`
- `.agents/plugins/marketplace.json`

## Runtime Contract References

The Codex plugin relies on the existing recorder behavior defined in:

- `recorder/src/main.rs`
- `recorder/src/session.rs`
- `recorder/src/transcription/mod.rs`

Those files remain the source of truth for session lifecycle, local transcription, and session artifacts.
