# `/mstop` Synthesis and Execution — Implementation Plan

## Overview

Build the Claude Code plugin that turns the existing Rust recorder into the product. When the user runs `/mstop`, the plugin drives local transcription (already working), synthesizes a codebase-grounded implementation plan from the transcript, presents the plan, and — on explicit user approval — autonomously executes it on a new git branch. This milestone delivers the product's actual value: a meeting becomes a working branch of code, with the user's only touch being the `/mstart` and `/mstop` slash commands plus one "execute" confirmation.

The recorder and transcription are out of scope for this plan — they work (`so_far.md` §1–§12). Everything here is plugin-side: three slash commands (`/mstart`, `/mstop`, `/mstat`), the synthesis prompt that produces `plan.md`, and the execution contract that the plugin hands Claude for the follow-up turn.

## Current State Analysis

### What exists and works

- **Rust recorder CLI** (`recorder/`) — `domino-recorder {start,stop,status,doctor}` works end to end on macOS. `start` daemonizes and captures stereo audio; `stop` sends SIGTERM, waits for encoder flush, and runs the full transcription pipeline inline.
- **`cmd_stop()` at `recorder/src/main.rs:143–181`** — the exact hook the plugin shells out to. On success it prints:
  ```
  Saved:
    /Users/nitin/.domino/recordings/<session>/meeting.opus (0.7 MB)
    /Users/nitin/.domino/recordings/<session>/transcript.json (33 segments, 80s audio, 23s wall, metal)
  ```
  On transcription failure it writes to stderr (`eprintln!`), preserves the audio, and exits with status `2`. On missing audio it prints `Session stopped: <dir> (no audio file produced)` and exits 0.
- **`cmd_start()` at `recorder/src/main.rs:38–141`** — prints session JSON on stdout: `{"pid":…, "session_dir":"…", "started_at":"…"}`. The parent returns immediately; the daemon runs detached.
- **`cmd_status()` at `recorder/src/main.rs:183–194`** — prints the same JSON if a session is active, or `{}` if not.
- **`transcript.json` schema (v1)** — documented in `so_far.md` §8, stable. Segments carry `start`, `end`, `speaker` (`"You"` or `"Meeting"`), `text`. This is the sole structured input to synthesis.
- **Session directory convention** — `~/.domino/recordings/<YYYY-MM-DD-HHMM>/` contains `meeting.opus`, `recorder.log`, `transcript.json`, `transcription.log`.

### What is missing

- **`plugin/`** contains only `.gitkeep`. No manifest, no commands, no scripts.
- **`scripts/`** is empty. If we decide to wrap the recorder with a shell helper (e.g. to hide `DYLD_FALLBACK_LIBRARY_PATH`), it would live here. Out of scope for this plan — the plugin assumes `domino-recorder` runs cleanly.
- **No synthesis prompt** has been authored anywhere. No plan template has been pinned as a file.
- **No execution contract** — no documented rules telling Claude what it may and may not do when the user approves a plan.

### Constraints and non-negotiables

- Transcription stays local (Whisper via `whisper-rs`, already implemented).
- Synthesis and execution both run on the user's Claude Code subscription. No separate LLM, no API key handling.
- macOS only in this milestone (matches recorder's reality).
- No push, no PR, no publishing as part of autonomous execution (chosen guardrail).

## Desired End State

After this plan lands, a full user-visible meeting-to-branch flow works end to end on this machine:

```
$ <launch Claude Code in the target repo>
> /mstart
{"pid": 42017, "session_dir": "/Users/nitin/.domino/recordings/2026-04-16-1930", …}

  … (meeting happens) …

> /mstop
Stopping recording…
Decoding audio…
Resampling channels to 16 kHz…
Transcribing (You)  [████████████████]  100%
Transcribing (Meeting) [████████████████]  100%

Reading ~/.domino/recordings/2026-04-16-1930/transcript.json…
Exploring this repo for mentioned files and symbols…
Drafting plan…

Plan written: ~/.domino/recordings/2026-04-16-1930/plan.md

  • Move auth to JWT-based sessions (src/auth.ts, src/middleware/session.ts)
  • Deprecate /v1 endpoints under src/api/v1/
  • Open question for you: JWT or opaque bearer?

Reply `execute` to apply this plan on a new branch, or tell me what to change.

> execute
Creating branch domino/meeting-2026-04-16-1930-jwt-migration…
[edits + tests + commits per phase, no push, no PR]
Done. Branch domino/meeting-2026-04-16-1930-jwt-migration has 4 commits and passes `npm test`.
Review with `git log -p domino/meeting-2026-04-16-1930-jwt-migration`.
```

### Verification of the end state

Concrete acceptance checks:

1. **Plugin is installable and slash commands register.** In a Claude Code session with the plugin loaded, `/mstart`, `/mstop`, and `/mstat` appear in the slash-command picker with the descriptions from their frontmatter.
2. **`/mstart` starts a session.** Running it prints the session JSON; `~/.domino/current.pid` appears; a daemon is running (`ps` shows `domino-recorder`).
3. **`/mstat` reports the session.** Prints the JSON from `domino-recorder status` while recording; prints `{}` when idle.
4. **`/mstop` stops, transcribes, and writes `plan.md`.** After a short (≥15s) recording of the user saying "we should rename the `foo` function to `bar` in `src/lib.rs`", `plan.md` exists in the session dir, references `src/lib.rs`, and proposes a rename. Inline summary prints with a path and up to 3 bullets and ends with an "execute / iterate" cue.
5. **Iteration works without a new command.** The user replies "actually, keep the name but add a doc comment" in plain chat. Claude rewrites `plan.md` accordingly, re-prints the summary. No `/mrevise` needed.
6. **`execute` triggers autonomous work on a new branch.** Typing `execute` creates `domino/meeting-<date>-<slug>`, makes the edits, runs tests if the repo has a detectable test runner, and commits per phase. No `git push`, no `gh pr create` runs. The previous branch the user was on is restored or the user is told to check it out.
7. **Empty-meeting bailout works.** A 10-second recording of silence or off-topic chatter results in no `plan.md`; the terminal prints `No actionable technical content found in this meeting.`; audio and `transcript.json` are preserved.
8. **Synthesis-failure path works.** If the Claude Code subscription is unreachable (simulated by running in a sandbox without network), synthesis aborts cleanly: no `plan.md`, a clear error is printed, audio and transcript are preserved.

### Key Discoveries

- **`cmd_stop()` in the recorder already does the full transcription loop** (`recorder/src/main.rs:157–180`). The plugin does not need to call any separate `transcribe` subcommand — it just runs `domino-recorder stop` and parses the `Saved:` block. This keeps the plugin thin.
- **Session directory is the only coordination artifact.** `cmd_stop()` prints the session dir inside the `Saved:` block. The plugin can extract it with a regex (or by globbing `~/.domino/recordings/` for the newest directory as a fallback). Once the plugin knows the session dir, `transcript.json` is at a fixed relative path.
- **No plugin manifest exists yet.** Claude Code plugin manifest naming (`plugin.json` vs `.claude-plugin.json`, etc.) should be verified against current Claude Code plugin docs before writing — listed as a small research task in Phase 1. Until confirmed, use the conservative assumption: a top-level manifest file and a `commands/` subdirectory with one `.md` per slash command.
- **Claude Code slash commands are markdown prompts.** The body of a command `.md` becomes context in the conversation when the user invokes it. It can contain shell-command instructions that Claude then runs via its Bash tool. This means the entire `/mstop` contract — transcription, synthesis, presentation, iteration handling, execution — can live as prose instructions in one file.
- **The slash command body persists into the next turn's context.** That is the mechanism we rely on for natural-chat iteration and for the `execute` follow-up: by the time the user replies, Claude already has the `/mstop` instructions and the freshly-written `plan.md` in its context, so it knows how to respond to "execute" vs "add Y" vs "cancel" without a second slash command.
- **The existing transcription progress UI is already minimal.** `cmd_stop()` emits `Preparing offline transcription…`, `Decoding audio…`, `Resampling…`, and per-channel progress bars (`so_far.md` §6.4). We pass these through to the user verbatim; the plugin does not wrap or replace them.
- **`~/.domino/recordings/<session>/` is writeable by the current user.** The recorder creates it during `start`. Writing `plan.md` alongside the other session artifacts is the natural home. No permission dance.

## What We're NOT Doing

Listed explicitly to prevent scope creep during implementation. Each of these has been considered and deferred.

- **No `/meeting retry-plan` command.** If synthesis fails, user re-runs `/mstop` manually (audio and transcript are preserved, but `stop` is idempotent only in the sense that it cleans up an already-stopped session; a clean re-invocation against the preserved session dir is not automated here). Transient-failure recovery is deferred.
- **No push, no PR, no publishing.** Execution stops at local commits on a new branch. The user runs `git push` / `gh pr create` themselves. This is the single non-negotiable guardrail the plan enforces.
- **No ExitPlanMode handoff.** The user explicitly chose `plan.md + inline summary + explicit 'execute' turn` over Claude Code's native plan-mode UI. The plugin does not call `ExitPlanMode`.
- **No discard command.** `/meeting discard` is deferred to v1.1.
- **No mic-only flag.** `/mstart` always tries both system audio and mic; recorder already falls back to mic-only if ScreenCaptureKit fails.
- **No multi-repo awareness.** Synthesis maps against Claude Code's current working directory. If the user records a meeting about a different repo, they launch Claude Code there.
- **No history / search across meetings.** Every meeting is standalone. Out of scope.
- **No MCP server.** See `so_far.md` §13.4 for rationale.
- **No install automation.** Plugin assumes `domino-recorder` is on `PATH`. Manual install docs (`so_far.md` §13.10) cover the one-time user setup.
- **No changes to the Rust recorder.** Zero edits under `recorder/`. If a recorder change turns out to be required (e.g. a new flag), that is a separate plan.
- **No destructive-op confirmation prompts inside execution.** The user chose "Never push / no PR without explicit command" as the sole guardrail. Execution is otherwise allowed to delete files, run migrations, etc. if the plan calls for it. Risk accepted.
- **No hard cap on execution size.** No stop-after-N-files or stop-after-N-tool-calls circuit breaker. Claude runs until done or until it hits a real error.
- **No in-scope enforcement.** Claude may touch files beyond those named in `plan.md` if execution reveals the need. No automatic halt-and-ask.
- **No doctor work.** `domino-recorder doctor` stays a stub. The plugin does not invoke it and does not add its own health check.
- **No Codex port.** Claude Code only in this milestone.
- **No Linux / Windows plugin.** macOS only.

## Implementation Approach

**Strategy: the plugin is three markdown files plus a manifest.** No compiled code, no daemons, no background processes. Each slash command is a prompt that directs Claude to run the recorder via Bash and then either finish (for `mstart` / `mstat`) or chain into synthesis and execution (for `mstop`). All the product intelligence lives in the prose of `mstop.md`.

**Why this is right for this milestone:**

- The recorder is already a working CLI. Wrapping it in a plugin requires no glue code — just a prompt that invokes the binary and interprets its output.
- Claude Code's slash-command bodies become part of conversation context, so a single command can govern behavior across multiple turns (the initial `/mstop` turn, the iteration turns, and the `execute` turn) without needing additional commands or hooks.
- Keeping the plugin all-markdown means the install story is small and the iteration speed on the synthesis/execution prompt is high (edit a file, reload, try again — no recompile).

**Plugin directory layout:**

```
plugin/
├── .claude-plugin/                       # plugin metadata (Claude Code convention — verify in Phase 1)
│   └── plugin.json                       # name, description, version, author
├── commands/
│   ├── mstart.md                         # /mstart slash command
│   ├── mstop.md                          # /mstop — the product
│   └── mstat.md                          # /mstat slash command
└── README.md                             # one-page install + usage (short)
```

**The three slash commands at a glance:**

- `/mstart` — one-shot. Runs `domino-recorder start`, prints the session JSON, done.
- `/mstat` — one-shot. Runs `domino-recorder status`, prints the JSON, done.
- `/mstop` — multi-turn. Runs `domino-recorder stop`, reads the transcript, explores the repo, writes `plan.md`, prints the summary, then governs the follow-up turns (iterate / execute / reject) using rules embedded in its own body.

**Synthesis and execution, conceptually:**

- Synthesis = Claude reads `transcript.json` + explores the repo (medium depth: files mentioned by name, greps for referenced symbols) + writes `<session>/plan.md` using the Rich template from `so_far.md` §13.11.1 + prints an inline summary.
- Iteration = normal conversation. Because `plan.md` and the `/mstop` instructions are in Claude's context, the user saying "change X" causes Claude to rewrite `plan.md` and re-present.
- Execution = on the `execute` signal, Claude creates `domino/meeting-<YYYY-MM-DD-HHMM>-<slug>`, walks the plan phase by phase, runs the repo's test command if one is detectable, commits per phase with clear messages, and reports back. Never pushes, never opens PRs.

---

## Phase 1: Plugin Scaffolding and Simple Commands

### Overview

Stand up the plugin directory, the manifest, and the two one-shot commands (`/mstart`, `/mstat`). This phase proves the plugin loads and that Claude can shell out to `domino-recorder`. No synthesis yet.

### Changes Required

#### 1. Verify Claude Code plugin manifest conventions

**Task**: Before writing any files, confirm (a) the exact manifest filename/location Claude Code expects today (e.g. `plugin.json`, `.claude-plugin/plugin.json`, `manifest.json`) and (b) the slash-command frontmatter fields that are actually respected (`description`, `argument-hint`, model, permissions).

**How**: Check Claude Code plugin documentation or an existing community plugin. Record the findings in a one-paragraph note at the top of `plugin/README.md` so future readers don't re-research.

**Why it matters**: If we guess wrong, the plugin silently fails to register. Cheap to verify once, expensive to debug later.

#### 2. `plugin/.claude-plugin/plugin.json`

**File**: `plugin/.claude-plugin/plugin.json` (exact path TBD per task above)

**Content shape** (illustrative — adjust to match the verified schema):

```json
{
  "name": "domino",
  "description": "Record a meeting, get an implementation plan, optionally execute it.",
  "version": "0.1.0",
  "author": "Nitin"
}
```

#### 3. `plugin/commands/mstart.md`

**File**: `plugin/commands/mstart.md`

**Body**:

```markdown
---
description: Start a Domino recording session (mic + system audio, macOS).
---

Run `domino-recorder start` via Bash. Print its stdout verbatim (it's session JSON: pid, session_dir, started_at). If the command exits non-zero, surface the error text clearly and do nothing else.

Do not read files. Do not explore the repo. Do not offer further commentary — this command exists only to start the recorder and get out of the way.
```

#### 4. `plugin/commands/mstat.md`

**File**: `plugin/commands/mstat.md`

**Body**:

```markdown
---
description: Show the current Domino recording session, or {} if idle.
---

Run `domino-recorder status` via Bash. Print its stdout verbatim. Do nothing else.
```

#### 5. `plugin/README.md`

**File**: `plugin/README.md`

**Content** (short, ~40 lines): what the plugin does; prerequisites (`domino-recorder` on PATH — link to `so_far.md` §13.10); how to load the plugin into Claude Code; the three slash commands and what they do. No install-automation instructions.

### Success Criteria

#### Automated Verification

- [ ] `plugin/.claude-plugin/plugin.json` (or verified equivalent) is valid JSON: `jq empty plugin/.claude-plugin/plugin.json`.
- [ ] All `plugin/commands/*.md` files exist and contain YAML frontmatter with a `description` field: `grep -l "^description:" plugin/commands/*.md` returns all three.
- [ ] `plugin/commands/mstart.md` and `plugin/commands/mstat.md` reference `domino-recorder` and nothing else.

#### Manual Verification

- [ ] Load the plugin in Claude Code. `/mstart`, `/mstop`, `/mstat` all appear in the slash-command picker with accurate descriptions.
- [ ] Running `/mstart` in a Claude Code session prints valid session JSON and leaves a daemon running (`pgrep domino-recorder` succeeds, `~/.domino/current.pid` exists).
- [ ] Running `/mstat` while active prints the JSON matching `current.pid`; running it idle prints `{}`.
- [ ] Running `/mstart` twice in a row (without stopping in between) surfaces the recorder's "session already active" error clearly, without the plugin swallowing it.

**Implementation Note**: After Phase 1 passes automated checks, pause for the human to run the manual verification steps in a fresh Claude Code session before proceeding to Phase 2. Phase 2 onward depends on the plugin actually being loadable.

---

## Phase 2: `/mstop` — Recorder Handoff and Session Discovery

### Overview

Write the first half of `/mstop.md`: the part that runs `domino-recorder stop`, surfaces the existing transcription progress, parses the session directory out of the `Saved:` output, and fails cleanly on the three recorder exit conditions (success, no audio, transcription failure). No synthesis yet — this phase ends with the command identifying the session dir and announcing "now synthesizing…" as a placeholder.

### Changes Required

#### 1. `plugin/commands/mstop.md` — scaffold + recorder handoff

**File**: `plugin/commands/mstop.md`

**Body (this phase — synthesis and execution sections will be added in Phases 3–5)**:

```markdown
---
description: Stop the Domino recording, transcribe, propose a plan, and (on approval) execute it.
---

You are running the `/mstop` command. This command has three jobs that unfold across multiple conversation turns:

1. This turn: stop the recorder, read the transcript, write `plan.md`, and present a summary.
2. Next turn(s): either execute the plan on approval, or iterate on the plan based on the user's feedback, or acknowledge rejection.

## Step 1 — Stop the recorder

Run `domino-recorder stop` via Bash.

Interpret the result:

- **Success**: the command exits 0 and stdout contains a line `Saved:` followed by a `meeting.opus` path and a `transcript.json` path. Extract the session directory (the parent directory of those files). Continue to Step 2.
- **No audio**: the command exits 0 and stdout is `Session stopped: <dir> (no audio file produced)`. Stop here. Tell the user the recording produced no audio and there is nothing to synthesize.
- **Transcription failure**: the command exits 2. stderr describes the failure. Stop here. Tell the user transcription failed, point them at the `transcription.log` inside the session dir, and preserve the session (do not delete anything). Do not attempt synthesis.

## Step 2 — (placeholder, filled in Phase 3)

Say: "Transcription complete. Synthesis will be implemented in Phase 3." and stop. Do not write any files.
```

#### 2. Test harness for the session-dir parser

**Task**: informal test — run `/mstop` in three manually engineered scenarios (real session, empty session, simulated transcription failure) and verify each branch is taken. No automated test infrastructure is added; this is verified by manual steps below.

### Success Criteria

#### Automated Verification

- [ ] `plugin/commands/mstop.md` exists and contains both "Step 1" and a "Step 2" placeholder section.
- [ ] The frontmatter has a non-empty `description`.

#### Manual Verification

- [ ] Record a ≥15 s session, run `/mstop`. The existing recorder progress (`Decoding audio…`, progress bars, `Saved:` block) is passed through to the terminal without the plugin wrapping or hiding it.
- [ ] After success, the plugin correctly identifies the session dir (confirmed by the Phase 3 scaffold message referencing the right path, once added; in this phase, verify by having Claude echo the extracted path).
- [ ] Start a session, immediately stop it before any audio accumulates, run `/mstop`. The plugin prints the "no audio produced" path and does not proceed.
- [ ] Break the Whisper model (`mv ~/.domino/models/ggml-small.en.bin{,.bak}`), run a recording + `/mstop`. The plugin reports transcription failure, points at `transcription.log`, and does not proceed. Restore the model.

**Implementation Note**: Pause for manual verification before starting Phase 3.

---

## Phase 3: Synthesis — `plan.md` from Transcript + Codebase

### Overview

Fill in Step 2 of `mstop.md`: have Claude read `transcript.json`, explore the current repo at medium depth, and write a Rich-template `plan.md` into the session directory. Print the inline summary (path + up to 3 bullets). End with the "execute / iterate / reject" cue. No execution yet.

This is where the product's value appears for the first time. The prose must be precise — every ambiguity becomes a failure mode at run time.

### Changes Required

#### 1. Append synthesis instructions to `mstop.md`

**File**: `plugin/commands/mstop.md`

**Append after Step 1, replacing the Step 2 placeholder**:

```markdown
## Step 2 — Read the transcript and explore the repo

Read `<session-dir>/transcript.json`. It has the shape documented in Domino's transcript schema: a top-level object with a `segments` array where each segment has `start`, `end`, `speaker` ("You" or "Meeting"), and `text`.

Now explore the current working directory at **medium depth**:

- For every file path mentioned in any segment's `text` (e.g. `src/auth.ts`, `api/v1/users.py`), read the file if it exists.
- For every symbol mentioned (function names, class names, config keys) use Grep to find where it lives in the repo. Read the top 1–3 files that contain each symbol, enough to ground the plan.
- Do not spawn a full Explore sweep. Do not recursively follow every import. Stay targeted — the goal is to ground the plan in real code, not to map the whole repo.

Budget roughly 30–120 seconds of tool calls for this step. If you find nothing relevant (no mentioned paths, no grep hits), fall through to the empty-meeting branch in Step 3.

## Step 3 — Decide: plan or bailout

Look at the transcript and your exploration together. Ask yourself: **is there actionable technical content in this meeting that ties to this codebase?** Use your judgment.

- **Yes** → continue to Step 4 (write the plan).
- **No** → do not write `plan.md`. Print exactly: `No actionable technical content found in this meeting.` Then stop. The audio and transcript are preserved; that is intentional.

## Step 4 — Write `plan.md`

Write a plan to `<session-dir>/plan.md` using the following Rich template. **Drop any section that has no real content rather than fabricating entries.** Only attribute decisions ("raised by Meeting") where the transcript makes the attribution explicit.

    # Meeting Plan — <date> <time>

    ## Speakers
    - <who spoke, from the transcript's "You" / "Meeting" channels>

    ## Decisions
    - <decision — attribution only if explicit in transcript>

    ## Action items
    - [ ] <concrete task> — owner: <if known; else "unclear">

    ## Proposed changes
    ### `<path/to/file>`
    - Why: "<short quote from transcript>"
    - Change: <what to do in this file>

    ## Risks
    - <risk, tied to a real file or module you just read>

    ## Open questions
    - <only questions the transcript genuinely left unresolved>

Write the file using the Write tool, path `<session-dir>/plan.md`.

## Step 5 — Print the inline summary

Print exactly this shape to the terminal:

    Plan written: <session-dir>/plan.md

      • <top decision or action item>
      • <second>
      • <third, if one exists>

    Reply `execute` to apply this plan on a new branch, or tell me what to change.

Use up to three bullets — fewer if the plan has fewer headline items. The three bullets should be the most decision-carrying items (prefer Decisions and Action items over Risks and Open questions).

## Step 6 — Stop this turn

After printing the summary, stop. Do not start executing. Wait for the user's next message. Steps 7+ handle the follow-up turns.

## Step 7 — (placeholder, filled in Phases 4 and 5)

Iteration and execution handling will be appended here.
```

#### 2. Privacy boundary note in `plugin/README.md`

**File**: `plugin/README.md`

**Add a "Privacy" section**: audio stays on the device, transcription runs locally via Whisper, transcript text is sent to Anthropic via your Claude Code session for synthesis. Mirror the language in `so_far.md` §13.8. This is not a first-run banner yet (that is a separate follow-up); it is the baseline documented statement.

### Success Criteria

#### Automated Verification

- [ ] `plugin/commands/mstop.md` contains all of Step 2, Step 3, Step 4, Step 5, and Step 6.
- [ ] `plugin/README.md` includes a "Privacy" section mentioning local transcription and remote synthesis.
- [ ] The template inside Step 4 uses the six section headings from `so_far.md` §13.11.1 (`Speakers`, `Decisions`, `Action items`, `Proposed changes`, `Risks`, `Open questions`).

#### Manual Verification

- [ ] Record a ≥30 s session saying something like "let's rename `foo` to `bar` in the top-level `lib.rs`, and add a short doc comment to `main.rs`". Run `/mstop`. After synthesis, `<session-dir>/plan.md` exists, references `lib.rs` and `main.rs` accurately, and contains a `Proposed changes` section with concrete file-level instructions.
- [ ] The inline summary prints a path and ≤3 bullets. The bullets are decisions or action items, not headings.
- [ ] Record a ≥30 s session of silence or unrelated chatter ("I had pasta for lunch. The weather is terrible."). Run `/mstop`. No `plan.md` is written. The terminal prints `No actionable technical content found in this meeting.` Audio and `transcript.json` are preserved in the session dir.
- [ ] Inspect `plan.md` for hallucinated attribution. If the transcript said "we should rename foo" without naming who said it, the plan must not say "raised by Meeting" (or similar) next to that decision.

**Implementation Note**: Pause for manual verification before Phase 4. The plan template's quality is the product's quality — any hallucination issues found here should be fixed by tightening the prompt language in Step 4 before proceeding.

---

## Phase 4: Iteration — Natural Chat on the Plan

### Overview

Handle the user's follow-up turn when they want to change the plan rather than execute it. No new slash command — this is entirely handled by instructions in `mstop.md` that Claude carries forward into the next turn's context.

### Changes Required

#### 1. Append iteration rules to `mstop.md`

**File**: `plugin/commands/mstop.md`

**Replace the Step 7 placeholder with**:

```markdown
## Step 7 — Handle the user's next message

The user has seen the plan. Their next message will be one of three things:

- **`execute`** (or clearly synonymous: "go ahead", "do it", "apply it") → jump to Step 8 (Execution).
- **Iteration feedback** — anything suggesting a change to the plan, e.g. "don't touch `src/auth.ts`", "also add a regression test", "use a feature flag instead", "rename the branch", "do only the first phase". → Revise `plan.md` as described below, then repeat Step 5 (print the updated summary) and return to waiting.
- **Rejection** — "cancel", "never mind", "don't do it". → Acknowledge briefly. Leave `plan.md` in place (it is valuable as a record even if not executed). Stop.

If the intent is ambiguous, ask a single clarifying question. Do not guess.

### How to revise `plan.md`

- Read the current `<session-dir>/plan.md`.
- Apply the user's feedback conservatively: change only what they asked for. Do not rewrite sections that were not mentioned.
- Write the updated file with the Write tool.
- Re-print the inline summary (Step 5 shape). The summary reflects the revised plan.
- Return to waiting for the user's next message.

Iteration may repeat any number of times. Keep revising, keep re-presenting. Do not execute until the user explicitly approves.
```

### Success Criteria

#### Automated Verification

- [ ] `mstop.md` contains a "Step 7" section with the three branches (execute / iterate / reject).
- [ ] The iteration branch includes explicit instructions to read the existing `plan.md` before rewriting.

#### Manual Verification

- [ ] Produce a plan (any content). Reply in plain chat: "don't touch `src/lib.rs`". `plan.md` is rewritten with the `src/lib.rs` references removed; the summary reprints. No `/mrevise` command was needed.
- [ ] Produce a plan. Reply "nah, cancel". Claude acknowledges briefly; `plan.md` is preserved; no execution happens.
- [ ] Produce a plan. Reply with something ambiguous ("hmm"). Claude asks a single clarifying question instead of guessing.
- [ ] Iterate twice in a row: first reply "remove the doc comment change", then reply "actually put it back". `plan.md` converges. No divergence or duplication.

**Implementation Note**: Pause for manual verification before Phase 5.

---

## Phase 5: Execution — Branch, Edits, Tests, Commits

### Overview

The payoff. On `execute`, Claude creates a new git branch named after the session, walks the plan phase by phase, makes the edits, runs the repo's tests if detectable, and commits per phase. Never pushes, never opens PRs. Reports back with a summary the user can act on.

### Changes Required

#### 1. Append execution rules to `mstop.md`

**File**: `plugin/commands/mstop.md`

**Append after Step 7**:

```markdown
## Step 8 — Execute the approved plan

Precondition: the user replied with `execute` (or a clear synonym) in Step 7.

### Step 8a — Set up the branch

Run the following via Bash, treating the first failure as a hard stop:

1. `git status --porcelain` — if the working tree has uncommitted changes, stop. Tell the user to commit or stash first, then re-invoke `execute`. Do not proceed.
2. `git rev-parse --abbrev-ref HEAD` — remember the current branch name; the user may want to return to it.
3. `git checkout -b domino/meeting-<YYYY-MM-DD-HHMM>-<slug>` — where `<YYYY-MM-DD-HHMM>` matches the session directory timestamp and `<slug>` is a 2–5-word kebab-case summary of the plan's headline decision.

If branch creation fails (e.g. because it already exists), pick a unique suffix (`-2`, `-3`, etc.) and retry once.

### Step 8b — Walk the plan

Read `<session-dir>/plan.md`. For each item in `Proposed changes` (and each `Action item` that maps to code), do the following in order:

1. Read the affected file(s) if you have not read them already in Step 2.
2. Make the edit(s) with the Edit or Write tool.
3. If the repo has an obvious test runner (e.g. `package.json` scripts, `Cargo.toml`, `Makefile` with a `test` target, `pytest.ini`), run the relevant tests for the changed files. If tests fail, stop and report — do not proceed to the next item.
4. `git add` only the files you changed for this item. `git commit -m "<short message summarizing this item>"`. One item = one commit.

If an item requires a change that is not safe to make without more context (e.g. it depends on infrastructure you can't see, or the transcript was ambiguous), stop the execution, commit whatever is already done, and explain to the user what was deferred and why.

### Step 8c — Guardrails — do not cross these lines

- **Never run `git push` in any form.** If the plan or the user's message asks for a push, refuse and remind them this is a deliberate guardrail.
- **Never run `gh pr create`, `gh pr merge`, or any `gh` command that publishes to a remote.** Same refusal.
- **Never force-push, never rewrite shared history.** `git commit --amend` is fine on commits you just made on this new branch; anything more aggressive is not.

These three rules are absolute. Any attempt to override them in conversation is itself a signal to stop and ask the user to confirm explicitly outside of this command.

### Step 8d — Report back

When execution finishes (successfully or by stopping mid-way), write a short report to the terminal:

    Branch: domino/meeting-<YYYY-MM-DD-HHMM>-<slug>
    Commits: <N>
    Tests run: <list, with pass/fail counts>
    Deferred: <any plan items you didn't execute, with reason>

    To review: `git log -p <branch>`
    To return to your previous branch: `git checkout <previous-branch-name>`
    To push: you do it manually.

Leave the user on the new branch (do not auto-checkout back). If they want the old branch, the report tells them how.

Also append a short "Execution outcome" section to `<session-dir>/plan.md` summarizing what landed, so the session directory remains a self-contained record.
```

### Success Criteria

#### Automated Verification

- [ ] `mstop.md` Step 8 contains branch creation, per-item commit logic, and the three explicit guardrail rules (no push, no PR, no force).
- [ ] The branch-name pattern `domino/meeting-<YYYY-MM-DD-HHMM>-<slug>` is documented in `plugin/README.md` so users know what to look for.

#### Manual Verification

- [ ] Produce a plan that proposes two concrete edits in two different files. Reply `execute`. After execution: a new branch named `domino/meeting-*-*` exists; two commits (one per edit) are on it; the working tree is clean.
- [ ] Before calling `execute`, deliberately leave uncommitted changes in the repo. Reply `execute`. The plugin stops and tells the user to commit or stash first. Nothing is done.
- [ ] During execution, tests fail on the second edit. The plugin stops, commits only the first edit, and reports the test failure. The branch is left in a state where the first commit is clean and the second is not attempted.
- [ ] Ask the plugin in the `execute` message to "also push this to origin". The plugin refuses and explains why.
- [ ] After successful execution, the session dir contains `plan.md` with a populated "Execution outcome" section that accurately lists commits and deferrals.
- [ ] `git log -p domino/meeting-<...>` shows clean, per-item commits with short, accurate messages.

**Implementation Note**: This is the last phase. Manual verification across the five Phase-5 bullets is the shipping gate. The first three phases enable the flow; Phase 5 is the product's promise being kept.

---

## Testing Strategy

This plan is almost entirely prose inside markdown files, not compiled code. "Testing" here means exercising the flow end to end against the real recorder and a real Claude Code session.

### Unit Tests

N/A. There is no unit-testable code in this milestone. The Rust recorder already has its own test suite (`recorder/tests/`) and is not being modified.

### Integration Tests

Informal, manual, and repeated per phase. Each phase's "Manual Verification" list is the integration test for that phase. Running them in order against a clean Claude Code session is the full acceptance run.

### Manual Testing Steps

The shipping acceptance run (expected to take ≥45 minutes including waiting for transcription):

1. **Plugin loads.** Claude Code session in the target repo shows `/mstart`, `/mstop`, `/mstat` in the picker.
2. **Happy path end to end.** Record a 2–3 minute meeting about a small, real change in the repo ("rename `foo` to `bar` in `src/lib.rs` and add a doc comment"). `/mstart`, meeting, `/mstop`. Synthesis produces a plan that cites `src/lib.rs` accurately. Reply `execute`. A branch is created, the rename happens, the doc comment is added, tests pass, two commits exist, working tree is clean.
3. **Iteration loop.** Repeat step 2 but after seeing the plan, reply "actually only do the rename — leave the doc comment alone". Plan updates. Reply `execute`. Only the rename lands. One commit.
4. **Empty-meeting bailout.** Record 30 seconds of silence or unrelated talk. `/mstop`. No `plan.md`. Terminal prints the bailout message.
5. **Transcription failure.** Rename the whisper model temporarily. Record + `/mstop`. Plugin surfaces the failure cleanly; no `plan.md`; audio and transcript preserved.
6. **Dirty working tree blocks execute.** Leave an uncommitted change. `/mstop`, get a plan, reply `execute`. Plugin refuses and asks the user to commit or stash first.
7. **Push / PR refusal.** In the `execute` reply, add "and push to origin". Plugin refuses the push, still executes the local commits.

### Edge Cases to Verify Manually

- Meeting references files that do not exist in the repo. Plan should either say so in `Open questions` or omit the phantom files, not fabricate.
- Meeting mixes unrelated topics (half about the codebase, half about lunch). Plan should cover the code half cleanly; the lunch half should not appear.
- Very long meeting (≥30 minutes audio, so ≥8 minutes transcription). The existing progress bar remains legible throughout. The synthesis step after transcription does not hang or timeout silently.
- Multiple consecutive `/mstart` attempts without stopping (recorder returns "session already active" — the plugin must not hide this).

---

## Performance Considerations

- **Transcription** is the wall-time floor: roughly 25–30% of meeting duration on this Apple-silicon machine (observed 80 s audio → 23 s transcribe). Synthesis adds 30–120 s on top depending on repo exploration depth. For a 1-hour meeting, the user should expect a plan in ≈15–20 minutes.
- **Execution** time is unbounded in principle and determined by the scope of the plan. Guidance for the user: plans that touch 1–3 files usually execute in under a minute; large plans can take several minutes. No progress bar is added for execution — Claude's normal tool-call output carries the signal.
- **Synthesis depth is capped by prose**, not by code. The "read mentioned files, grep mentioned symbols, 30–120 s budget" language in Step 2 of `mstop.md` is the control. If synthesis starts feeling too deep or too shallow, the fix is prompt-side, not code-side.
- **Context size**. Long transcripts (≥1 hour, ≥10k words) plus a codebase exploration can approach Claude Code's context limits. If this becomes a problem in practice, the mitigation is to trim the transcript to segments with code-related keywords before synthesis, or to summarize the transcript first and synthesize from the summary. Not done preemptively in this plan.

---

## Migration Notes

No migration. The plugin is additive. Existing recorder behavior, session directories, and `transcript.json` schema are all untouched.

Users on this machine who have an active recording session (unlikely) should `/record stop` / `domino-recorder stop` it before loading the plugin for the first time, purely to avoid mixing pre- and post-plugin sessions. No data migration or format change is required.

---

## References

- Product thesis and architecture decisions: `so_far.md` §13.1–§13.11.
- Transcript schema (v1): `so_far.md` §8.
- Rich plan.md template: `so_far.md` §13.11.1.
- Empty-meeting bailout semantics: `so_far.md` §13.11.2.
- Synthesis-failure semantics: `so_far.md` §13.11.3.
- Privacy boundary wording: `so_far.md` §13.8.
- Recorder `cmd_stop()` implementation: `recorder/src/main.rs:143–181`.
- Recorder `cmd_start()` and session JSON shape: `recorder/src/main.rs:38–141`.
- Recorder `cmd_status()`: `recorder/src/main.rs:183–194`.
- Manual install and runtime env: `so_far.md` §6.1, §13.10.
- Prior plan (transcription): `thoughts/shared/plans/2026-04-16-local-transcription.md`.
- Prior plan (macOS audio capture): `thoughts/shared/plans/2026-04-15-domino-v1-macos-audio-capture.md`.
