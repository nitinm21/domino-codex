---
name: mstat
description: Show the active Domino meeting recording status explicitly when the user invokes $mstat or asks for Domino recording status.
---

Run this exact sequence via Bash:

```bash
RECORDER_BIN="$PWD/recorder/target/release/domino-codex-recorder"
if [ ! -x "$RECORDER_BIN" ]; then
  for CANDIDATE in \
    "$HOME/.local/bin/domino-codex-recorder" \
    "/opt/homebrew/bin/domino-codex-recorder" \
    "/usr/local/bin/domino-codex-recorder"
  do
    if [ -x "$CANDIDATE" ]; then
      RECORDER_BIN="$CANDIDATE"
      break
    fi
  done
fi
if [ ! -x "$RECORDER_BIN" ]; then
  RECORDER_BIN="$(command -v domino-codex-recorder || true)"
fi
if [ -z "$RECORDER_BIN" ]; then
  printf 'domino-codex-recorder not found. Checked %s, %s, %s, %s, and PATH.\n' \
    "$PWD/recorder/target/release/domino-codex-recorder" \
    "$HOME/.local/bin/domino-codex-recorder" \
    "/opt/homebrew/bin/domino-codex-recorder" \
    "/usr/local/bin/domino-codex-recorder" >&2
  exit 127
fi
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx "$RECORDER_BIN" status
```

Print stdout verbatim. Do nothing else.

If the command exits zero but macOS prints duplicate Swift runtime or `objc[...]` warnings on stderr, ignore those warnings and still use stdout as the result.

If the sandboxed command reports `stale PID file detected` and then returns `{}` immediately after a recording was started with an escalated or out-of-sandbox recorder command, do not trust that result yet. Retry the same resolved recorder `status` command outside the sandbox and treat the out-of-sandbox result as authoritative.
