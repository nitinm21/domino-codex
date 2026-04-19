---
name: mstart
description: Start a Domino meeting recording session explicitly when the user invokes $mstart or asks to begin recording a meeting with Domino.
---

Run this exact command via Bash:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx domino-recorder start
```

Print stdout verbatim because it contains the session JSON: `pid`, `session_dir`, and `started_at`.

If the command exits zero but macOS prints duplicate Swift runtime or `objc[...]` warnings on stderr, ignore those warnings and still use stdout as the result.

If the command exits non-zero, surface stderr clearly and stop.

Do not read files. Do not explore the repo. This skill exists only to start the recorder and get out of the way.
