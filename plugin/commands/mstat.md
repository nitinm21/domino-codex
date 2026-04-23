---
description: Show the current Domino recording session, or {} if idle.
---

Resolve the recorder binary in this order: `./recorder/target/release/domino-recorder` from the current repo root, then `domino-recorder` from `PATH`. Run the resolved binary with `status` via Bash. Print its stdout verbatim. Do nothing else.
