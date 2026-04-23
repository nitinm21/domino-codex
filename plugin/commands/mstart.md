---
description: Start a Domino recording session (mic + system audio, macOS).
---

Resolve the recorder binary in this order: `./recorder/target/release/domino-recorder` from the current repo root, then `domino-recorder` from `PATH`. Run the resolved binary with `start` via Bash. Print its stdout verbatim (it's session JSON: pid, session_dir, started_at). If the command exits non-zero, surface the error text clearly and do nothing else.

Do not read files. Do not explore the repo. Do not offer further commentary — this command exists only to start the recorder and get out of the way.
