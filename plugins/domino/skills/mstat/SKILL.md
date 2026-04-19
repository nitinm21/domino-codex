---
name: mstat
description: Show the active Domino meeting recording status explicitly when the user invokes $mstat or asks for Domino recording status.
---

Run this exact command via Bash:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx domino-recorder status
```

Print stdout verbatim. Do nothing else.

If the command exits zero but macOS prints duplicate Swift runtime or `objc[...]` warnings on stderr, ignore those warnings and still use stdout as the result.

If the sandboxed command reports `stale PID file detected` and then returns `{}` immediately after a recording was started with an escalated or out-of-sandbox `domino-recorder start`, do not trust that result yet. Retry the same `domino-recorder status` command outside the sandbox and treat the out-of-sandbox result as authoritative.
