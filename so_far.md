# Domino - Project Context and Current Repo State

This document is the current source of truth for Domino's product direction and, more importantly, the actual state of the repository as it exists today.

If this file disagrees with older planning docs under `thoughts/`, trust this file plus the code in `recorder/`.

Last updated: 2026-04-16

---

## 1. Executive Summary

Domino is currently a standalone Rust CLI recorder. It is not yet a Claude Code plugin or a Codex plugin, even though that is still the product direction.

What works today on this machine:

- A detached background recorder can be started from the terminal.
- Microphone capture works.
- macOS system-audio capture via ScreenCaptureKit is wired up and was exercised in a real session.
- Audio is encoded to a single stereo `meeting.opus` file.
- `stop` automatically runs offline transcription.
- The transcript is written to `transcript.json`.
- Channel-based speaker labeling is working: left channel becomes `"You"`, right channel becomes `"Meeting"`.

What is not fully done or not fully green:

- `plugin/` is still empty, so there is no slash-command wrapper yet.
- `scripts/` is still empty.
- `doctor` is a placeholder and does not actually diagnose permissions or devices yet.
- `MODEL_SHA256_HEX` is still empty in the code, so model integrity verification is not enforced yet.
- Recorder commands on this machine currently need `DYLD_FALLBACK_LIBRARY_PATH` set to find the Swift runtime.
- Full test automation is not green because `recorder/tests/concurrent_start.rs` still fails.

The current state is best described as:

- Core macOS recorder path: working
- Automatic transcription: working
- Channel-based diarization: working
- Plugin/product shell around it: not built yet
- Automation and operational polish: partial

---

## 2. Product Direction That Still Holds

The high-level product goal is unchanged:

1. Start recording from the assistant environment.
2. Attend the meeting normally.
3. Stop recording.
4. Get structured meeting output that downstream tooling can use.

The key design choices that still hold:

- Terminal-first experience.
- Local capture, not a meeting bot.
- Local/offline transcription.
- Single stereo Opus file per session.
- Channel-based separation for `"You"` vs. `"Meeting"`.

The important update is that the repo has moved past the purely aspirational stage. The recorder/transcription core now exists and has been manually exercised.

---

## 3. Repo Snapshot

Current top-level layout:

```text
domino/
├── recorder/                # working Rust crate
├── plugin/                  # placeholder only
├── scripts/                 # placeholder only
├── thoughts/                # planning docs
├── starter_pack/            # unrelated helper folder
└── so_far.md                # this file
```

The recorder crate already contains the core implementation:

```text
recorder/src/
├── main.rs                  # start / stop / status / doctor
├── cli.rs                   # clap CLI definitions
├── session.rs               # ~/.domino paths, PID file, session lock
├── signals.rs               # SIGTERM / SIGINT shutdown flag
├── audio/
│   ├── mic.rs               # microphone capture
│   ├── system.rs            # ScreenCaptureKit system-audio capture
│   └── encoder.rs           # stereo Opus encoder
└── transcription/
    ├── mod.rs               # end-to-end stop-time transcription pipeline
    ├── decode.rs            # decode Ogg Opus -> channel buffers
    ├── resample.rs          # 48 kHz -> 16 kHz
    ├── whisper.rs           # whisper-rs wrapper
    ├── merge.rs             # merge labeled segments by time
    ├── output.rs            # transcript.json writer
    ├── model.rs             # model lookup / download / verification
    └── progress.rs          # progress bar + log wiring
```

There is also one integration test:

- `recorder/tests/concurrent_start.rs`

That test is currently the main failing automation check.

---

## 4. What Has Been Verified

The strongest evidence is the real session directory:

- `~/.domino/recordings/2026-04-16-1853/`

Observed artifacts from that run:

- `meeting.opus`: `751273` bytes
- `recorder.log`: `1702` bytes
- `transcript.json`: `5181` bytes
- `transcription.log`: `1263` bytes

Observed audio metadata from `ffprobe`:

- codec: `opus`
- channels: `2`
- sample rate: `48000`
- duration: `79.526500` seconds

Observed transcript metadata:

- `version`: `1`
- `audio_file`: `meeting.opus`
- `model`: `ggml-small.en`
- `language`: `en`
- `accelerator`: `metal`
- `transcription_wall_sec`: `23.481138625`
- segment count: `33`
- speaker split: `20` `"You"` segments, `13` `"Meeting"` segments

Observed recorder log facts:

- the daemon started
- the mic device was `MacBook Air Microphone`
- system audio capture started via ScreenCaptureKit
- the stereo encoder started with `system_audio=true`
- the daemon exited cleanly after shutdown

Observed transcription log facts:

- transcription started automatically during `stop`
- `meeting.opus` decoded successfully
- both channels were resampled to 16 kHz
- whisper loaded with `metal`
- transcript output was written successfully

Conclusion:

- The core capture -> save -> transcribe -> labeled transcript loop is working.

---

## 5. Important Nuance About "Diarization"

What is implemented today is not general speaker diarization.

What exists today:

- Left channel of the stereo recording is treated as `"You"`.
- Right channel of the stereo recording is treated as `"Meeting"`.
- Each channel is transcribed independently.
- The resulting segments are merged by timestamp.

That means:

- Domino can currently distinguish "my side" vs. "everything coming from system audio".
- Domino cannot distinguish Alice vs. Bob inside the `"Meeting"` channel.

It also means you can still see duplicated content across both labels if the microphone physically hears the laptop speakers. That is expected with open speakers and is not a merge bug. The recent transcript shows exactly that kind of overlap in a few places.

So the accurate statement is:

- Channel-based labeling is working.
- True multi-speaker diarization is not implemented.

---

## 6. Operational Runbook

### 6.1. Build Commands

Base release build:

```bash
cargo build --release --manifest-path recorder/Cargo.toml
```

If ScreenCaptureKit or Swift build/linking is unhappy on this machine, use the explicit SDK path:

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk cargo build --release --manifest-path recorder/Cargo.toml
```

Important runtime note for this machine:

- `domino-recorder` does not currently have a working embedded Swift runtime search path.
- In practice, recorder commands need `DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx`.

Safe command prefix to use before recorder invocations:

```bash
export DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
```

After that export, the common commands are:

```bash
recorder/target/release/domino-recorder --help
recorder/target/release/domino-recorder start
recorder/target/release/domino-recorder status
recorder/target/release/domino-recorder stop
recorder/target/release/domino-recorder doctor
```

### 6.2. Start Command

Primary start command:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx recorder/target/release/domino-recorder start
```

Optional output-directory override:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx recorder/target/release/domino-recorder start --out-dir /tmp/domino-smoke
```

What `start` does:

1. Ensures `~/.domino/` exists.
2. Acquires `~/.domino/session.lock`.
3. Checks `~/.domino/current.pid` to make sure another session is not already active.
4. Creates a session directory:
   - default: `~/.domino/recordings/<YYYY-MM-DD-HHMM>/`
   - overridden: `<out-dir>/<YYYY-MM-DD-HHMM>/`
5. Forks.
6. Child process calls `setsid()` and becomes the detached recorder daemon.
7. Child redirects stdout/stderr to `<session>/recorder.log`.
8. Child starts mic capture, system capture, and the Opus encoder.
9. Parent prints session JSON to stdout and exits immediately.

Expected stdout shape from a successful `start`:

```json
{"pid":12345,"session_dir":"/Users/nitin/.domino/recordings/2026-04-16-1853","started_at":"2026-04-16T18:53:48-05:00"}
```

Important details:

- `start` is meant to return immediately.
- The actual recorder work happens in the child daemon.
- The daemon logs to `recorder.log`, not the terminal.
- If system-audio capture fails on macOS, recording continues in mic-only mode and the right channel is silent.
- That fallback is logged in `recorder.log`; `status` does not surface it.
- If a session is already active, `start` fails with a clear error telling the user to run `stop` first.

### 6.3. Status Command

Command:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx recorder/target/release/domino-recorder status
```

Observed idle output:

```json
{}
```

Expected active-session output shape:

```json
{"pid":12345,"session_dir":"/Users/nitin/.domino/recordings/2026-04-16-1853","started_at":"2026-04-16T18:53:48-05:00"}
```

Current limitations of `status`:

- it only reports PID, session path, and start timestamp
- it does not show duration
- it does not show whether transcription is running
- it does not show whether system capture fell back to mic-only
- it does not inspect artifact completeness

### 6.4. Stop Command

Command:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx recorder/target/release/domino-recorder stop
```

What `stop` does:

1. Reads `~/.domino/current.pid`.
2. Sends `SIGTERM` to the recorder daemon.
3. Waits up to 5 seconds for a clean exit.
4. Sends `SIGKILL` if the process is still alive after the timeout.
5. Removes `~/.domino/current.pid`.
6. Looks for `<session>/meeting.opus`.
7. If audio exists, automatically runs the offline transcription pipeline.
8. Writes `transcript.json` and `transcription.log`.
9. Prints saved-artifact info to stdout.

During the transcription phase, `stop` currently emits user-visible progress such as:

- `Preparing offline transcription...`
- `Checking transcription model...`
- `Decoding audio...`
- `Resampling channels to 16 kHz...`
- progress-bar output while whisper transcribes each channel

If `meeting.opus` is missing, `stop` prints:

```text
Session stopped: /path/to/session (no audio file produced)
```

If transcription succeeds, `stop` prints a summary like:

```text
Saved:
  /Users/nitin/.domino/recordings/<session>/meeting.opus (0.7 MB)
  /Users/nitin/.domino/recordings/<session>/transcript.json (33 segments, 80s audio, 23s wall, metal)
```

If transcription fails:

- the audio file is preserved
- the command exits with status code `2`
- the user is pointed at `transcription.log`

This is an important current behavior:

- transcription is automatic
- there is no separate public `transcribe` command
- `stop` is the only normal path that generates the transcript

### 6.5. Doctor Command

Command:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx recorder/target/release/domino-recorder doctor
```

Current output:

```text
Domino Recorder - Health Check
  (doctor checks will be implemented in Phase 4)
```

So today `doctor` is documentation-only theater. It is not a real health check yet.

---

## 7. Files, Paths, and Session Artifacts

### 7.1. Top-Level Runtime Paths

Domino currently stores runtime data under:

```text
~/.domino/
```

Important paths:

- `~/.domino/current.pid`
- `~/.domino/session.lock`
- `~/.domino/models/ggml-small.en.bin`
- `~/.domino/recordings/<session>/`

What each one means:

- `current.pid`
  - JSON file describing the active daemon
  - only meaningful while a recording is in progress
  - verified absent after the recent successful `stop`

- `session.lock`
  - lock-file path used to serialize `start`
  - its existence does not mean a recording is active
  - the lock state matters, not the file's mere presence

- `models/ggml-small.en.bin`
  - whisper model used during transcription
  - currently present on this machine

### 7.2. Session Directory Layout

A normal completed session looks like:

```text
~/.domino/recordings/<session>/
├── meeting.opus
├── recorder.log
├── transcript.json
└── transcription.log
```

#### `meeting.opus`

- Ogg Opus container
- stereo
- 48 kHz
- left channel = mic
- right channel = system audio

This is the canonical saved recording artifact.

#### `recorder.log`

This is the daemon log. It is the best place to inspect:

- which input device was selected
- whether ScreenCaptureKit system capture started
- whether the recorder fell back to mic-only mode
- encoder drift warnings
- dropped-sample warnings
- clean shutdown vs. crash behavior

For macOS system-audio validation, `recorder.log` is more authoritative than `status`.

#### `transcription.log`

This is the stop-time transcription log. It records:

- transcription start
- model check
- decode timing
- resample timing
- whisper accelerator selection
- segment counts
- transcript output write

It is created at the start of transcription, so it should also exist for failed transcription attempts.

#### `transcript.json`

This is the structured transcript contract. It is the main downstream artifact for anything smarter than raw audio playback.

It is written atomically:

- file is written to `transcript.json.tmp`
- file is renamed into place as `transcript.json`

That reduces the chance of leaving a half-written transcript behind.

### 7.3. Temporary / Intermediate Files

Current transient files worth knowing about:

- `transcript.json.tmp`
  - temporary file during transcript write

- `ggml-small.en.bin.part`
  - partial model download file if the model must be fetched or resumed

---

## 8. `transcript.json` Contract

Current top-level fields:

- `version`
- `audio_file`
- `duration_sec`
- `model`
- `model_sha256`
- `language`
- `transcribed_at`
- `transcription_wall_sec`
- `accelerator`
- `segments`

Current shape:

```json
{
  "version": 1,
  "audio_file": "meeting.opus",
  "duration_sec": 79.5135,
  "model": "ggml-small.en",
  "model_sha256": "",
  "language": "en",
  "transcribed_at": "2026-04-16T18:55:31.877642-05:00",
  "transcription_wall_sec": 23.481138625,
  "accelerator": "metal",
  "segments": [
    {
      "start": 0.0,
      "end": 5.0,
      "speaker": "You",
      "text": "..."
    },
    {
      "start": 0.0,
      "end": 15.56,
      "speaker": "Meeting",
      "text": "..."
    }
  ]
}
```

Important details:

- `speaker` is currently constrained to `"You"` or `"Meeting"`.
- segment timestamps are in seconds from the beginning of the recording.
- segments are merged in chronological order.
- if two segments have the exact same start time, `"You"` sorts before `"Meeting"`.
- `accelerator` is whatever whisper actually used, for example `metal` or `cpu`.

Important current caveat:

- `model_sha256` is currently the empty string because `MODEL_SHA256_HEX` has not been filled in yet.

So the transcript schema is richer than the original project notes, but the model-integrity field is not yet meaningful.

---

## 9. Current Test and Verification Status

### 9.1. Manual Verification

Manual verification is the strongest green signal right now.

Verified from the real session on 2026-04-16:

- recorder daemon started successfully
- mic capture started successfully
- ScreenCaptureKit system capture started successfully
- stereo Opus file was produced
- automatic transcription ran during `stop`
- transcript file was produced
- transcript contains both `"You"` and `"Meeting"` segments

This is enough to say the main macOS recorder/transcription loop works.

### 9.2. Automated Tests

Plain `cargo test --manifest-path recorder/Cargo.toml` is not reliable on this machine because the binary fails to load `libswift_Concurrency.dylib` without the Swift runtime fallback path.

The workable test command today is:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx cargo test --manifest-path recorder/Cargo.toml
```

Observed result from that command:

- `42` tests passed
- `1` test ignored
- `1` test failed

The failing test is:

- `recorder/tests/concurrent_start.rs`

That means:

- unit coverage for audio/transcription/session utilities is mostly healthy
- lifecycle concurrency automation is still not correct or not stable

### 9.3. Current Meaning of "Working Properly"

The honest read is:

- yes, transcription is working properly in a real run
- yes, channel-based `"You"` / `"Meeting"` labeling is working properly in a real run
- no, the repo is not fully production-ready
- no, the lifecycle/test/doctor/plugin story is not complete

---

## 10. Known Gaps, Risks, and Sharp Edges

These are the important current limitations.

### 10.1. No Plugin Wrapper Yet

`plugin/` is just `.gitkeep`.

Implication:

- there is no `/record start`
- there is no plugin install flow
- everything is currently driven by the Rust CLI directly

### 10.2. `doctor` Is Still a Stub

There is no real permission, device, or OS diagnostics yet.

Implication:

- permission troubleshooting is manual
- users must inspect logs and macOS settings themselves

### 10.3. Swift Runtime Path Is Still Fragile

On this machine, the recorder binary does not run cleanly without:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
```

Without that env var, the binary fails to load `libswift_Concurrency.dylib`.

Even with the env var, recorder commands currently emit duplicate Swift/Objective-C class warnings on startup. They are noisy but have not prevented the real session from completing.

### 10.4. Model Integrity Pinning Is Not Finished

`MODEL_SHA256_HEX` is currently empty in `recorder/src/transcription/model.rs`.

Implication:

- the code logs a warning
- the transcript's `model_sha256` field is empty
- the model is not actually being integrity-checked yet

### 10.5. Concurrency Automation Is Not Green

`recorder/tests/concurrent_start.rs` fails right now.

Implication:

- we should trust manual `start` / `status` / `stop` verification more than the race test
- concurrent start behavior should still be treated as unfinished

### 10.6. Session Naming Is Minute-Resolution

Session directories are named with:

```text
%Y-%m-%d-%H%M
```

Implication:

- two recordings started within the same minute can target the same session directory
- that is a real collision risk and should be fixed later

### 10.7. `status` Is Shallow

`status` only reports PID/session metadata.

Implication:

- it does not prove system audio is flowing
- it does not prove transcription will succeed
- it does not show dropped samples, drift, or device selection

### 10.8. macOS-Centric Reality

Current repo reality is macOS-first:

- system-audio capture implementation exists for macOS
- non-macOS builds fall back to mic-only behavior for the system channel path

The broader cross-platform vision still exists, but the implemented and manually verified path is macOS.

---

## 11. Recommended Day-to-Day Commands

If the goal is "use the recorder today and inspect its output", these are the practical commands:

Set the runtime env:

```bash
export DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
```

Build:

```bash
cargo build --release --manifest-path recorder/Cargo.toml
```

Start a recording:

```bash
recorder/target/release/domino-recorder start
```

Check if one is active:

```bash
recorder/target/release/domino-recorder status
```

Stop and trigger transcription:

```bash
recorder/target/release/domino-recorder stop
```

Inspect the newest session:

```bash
ls -lah ~/.domino/recordings
ls -lah ~/.domino/recordings/<session>
sed -n '1,200p' ~/.domino/recordings/<session>/recorder.log
sed -n '1,200p' ~/.domino/recordings/<session>/transcription.log
jq '.segments[:10]' ~/.domino/recordings/<session>/transcript.json
ffprobe -v error -show_entries stream=index,codec_name,codec_type,channels,sample_rate:format=duration,size -of json ~/.domino/recordings/<session>/meeting.opus
```

Run tests with the current runtime workaround:

```bash
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx cargo test --manifest-path recorder/Cargo.toml
```

---

## 12. Bottom Line

Domino has moved from concept to a working macOS recorder/transcriber core.

The current repo can:

- record mic + system audio into one stereo Opus file
- stop cleanly
- automatically transcribe the recording offline
- label transcript segments as `"You"` and `"Meeting"`

The current repo cannot yet claim:

- production-ready install/run ergonomics
- a real `doctor` command
- a plugin wrapper
- fully green automation
- finalized model integrity handling

That is the correct current-state summary as of 2026-04-16.

---

## 13. Product Architecture and Direction Decisions (2026-04-16)

This section captures the product thesis and the architecture decisions made on 2026-04-16. It sits on top of the current-state snapshot above (§1–§12) and defines what v1 looks like.

### 13.1. Product Thesis

Domino is not a meeting transcription tool. Transcription is plumbing. The product is:

> A meeting ends. Claude Code proposes an implementation plan for this codebase based on what was discussed. The user did not have to ask.

The core UX loop is:

1. `/meeting start` — start recording.
2. Attend the meeting normally.
3. `/meeting stop` — stop recording, transcribe locally, and produce a plan automatically.

"Automatically" is the whole product. The user is never expected to manually hand the transcript to Claude, summarize the meeting, or prompt synthesis. Removing that handoff is the differentiator. Every architectural decision below protects it.

### 13.2. v1 Scope — What We Are Building

- A Claude Code plugin wrapping the existing Rust recorder.
- Slash commands: `/meeting start`, `/meeting stop`, `/meeting status`.
- Automatic synthesis of an implementation plan immediately after `/meeting stop` completes transcription.
- Plan output:
  - `plan.md` written into the session directory.
  - A short inline summary printed in the terminal after `/meeting stop` returns.
- Plan is scoped to the codebase in Claude Code's current working directory.
- Transcription is local (Whisper, already working).
- Synthesis runs through the user's existing Claude Code subscription.

### 13.3. v1 Scope — What We Are NOT Building

- No MCP server. Not useful at this scope.
- No cross-session or cross-meeting intelligence. Every meeting is standalone.
- No history search, tagging, or meeting corpus.
- No multi-speaker diarization beyond the existing channel-based `"You"` / `"Meeting"` labels.
- No Codex plugin at launch. Claude Code first.
- No autonomous code edits. Claude proposes; the user decides whether to execute.
- No Linux/Windows build. macOS only in v1.

### 13.4. Why No MCP Server (v1)

An MCP server earns its keep when Claude needs to pull structured data from an external system mid-conversation — typically for queryable corpora (meeting history, search, multi-record lookups).

At v1 scope:

- There is exactly one relevant transcript per meeting.
- That transcript is a JSON file on disk at a known path.
- The plugin slash command already knows which file to read.

Reading a single known file does not need an MCP server. The slash command reads it directly. Adding an MCP server would add:

- A second process to install and keep alive.
- A second surface to version and distribute.
- A second failure mode the end user can hit.

If, post-v1, we decide history search is worth shipping, the `transcript.json` schema is already stable enough to layer an MCP server on top without rewriting the CLI.

### 13.5. Architecture Topology

Three components:

1. **Rust CLI recorder** (`domino-recorder`) — exists today. Captures audio, transcribes locally, writes `transcript.json`. Installed on `PATH`.
2. **Claude Code plugin** — to build. A set of slash commands implemented as markdown prompts that shell out to the recorder and then direct Claude to synthesize.
3. **Whisper model** — downloaded to `~/.domino/models/` on first transcription. Handled by the recorder today.

No MCP server. No background daemon beyond the recorder process itself. No additional installed services.

### 13.6. Slash Command Design

`/meeting start` — runs `domino-recorder start`. Prints the session JSON. Nothing else.

`/meeting stop` — runs `domino-recorder stop`. Once the recorder has produced `transcript.json`, the slash-command prompt directs Claude to:

1. Read `~/.domino/recordings/<latest>/transcript.json`.
2. Read the code in the current working directory as needed to ground the plan.
3. Produce an implementation plan mapping meeting decisions to concrete next steps in this repo.
4. Write the plan to `<session>/plan.md`.
5. Print a short inline summary of the plan in the terminal.

`/meeting status` — thin wrapper over `domino-recorder status`.

That is the complete command surface for v1.

### 13.7. Transcription UX During `/meeting stop`

- Blocking foreground. The command does not return until transcription is done and the plan is written.
- Progress UI is intentionally minimal. Its only job is to let the user know the process is alive and making progress. No fancy visuals, no extra metadata, no dashboards.
- Expected wall time: currently ~29% of audio duration on this Apple-silicon machine (observed: 80s audio → 23s transcribe). For a 1-hour meeting, budget ~15–20 minutes of wait before the plan appears.
- This latency is the explicit tradeoff for keeping transcription local. The product accepts it.

### 13.8. Codebase Scope and Privacy Boundary

- The target codebase is always Claude Code's current working directory. No config, no flags, no interactive prompt.
- If the user wants a plan against a different repo, they launch Claude Code from that repo.
- Privacy boundary, stated honestly:
  - Audio never leaves the device.
  - Transcription runs locally via Whisper.
  - Synthesis does send transcript text to Anthropic via Claude Code's normal API path. This is intentional — we reuse the user's existing Claude Code subscription instead of shipping or requiring a local LLM.
  - End-user-facing docs and install flow need to state this boundary plainly.

### 13.9. Platform Priority

- v1: Claude Code plugin on macOS only.
- Codex is a follow-up, not a parallel target.
- Linux/Windows builds are deferred because the recorder's system-audio path is macOS-specific (ScreenCaptureKit).

### 13.10. Distribution and the Manual Install Story

v1 ships the Claude Code plugin first. The Rust recorder binary is installed manually and documented clearly until a release pipeline exists.

**System requirements:**

- macOS (Apple silicon is the primary target; Intel Mac expected to work).
- Xcode Command Line Tools (needed for the Swift runtime and ScreenCaptureKit linking).
- Rust toolchain — only while we're still building from source. Goes away once prebuilt binaries ship.
- Network access for the first-time Whisper model download.

**Install steps (as of today):**

1. Clone the domino repo.
2. Build the recorder:
   ```bash
   cargo build --release --manifest-path recorder/Cargo.toml
   ```
   If the Swift / ScreenCaptureKit link step fails, retry with the explicit SDK path:
   ```bash
   SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk \
     cargo build --release --manifest-path recorder/Cargo.toml
   ```
3. Copy `recorder/target/release/domino-recorder` to a directory on `PATH` (for example `/usr/local/bin`).
4. On first `domino-recorder start`, grant macOS permissions when prompted:
   - Microphone access.
   - Screen Recording access (required for ScreenCaptureKit system-audio capture).
5. On first `domino-recorder stop`, the recorder downloads the Whisper model (~466 MB for `ggml-small.en`) to `~/.domino/models/`. One-time.

**Known sharp edges the install docs must call out:**

- The recorder currently needs `DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx` to load `libswift_Concurrency.dylib`. Until the Swift runtime search path is baked into the binary, this env var is either documented or hidden inside a shell wrapper the plugin calls.
- Duplicate Swift / Objective-C class warnings print on startup. Noisy but harmless.
- `recorder/tests/concurrent_start.rs` is failing today. Lifecycle correctness currently rests on manual verification (see §9).

**Path to nicer distribution, post-v1:**

1. Cut GitHub Releases with prebuilt `darwin-arm64` and `darwin-x64` binaries.
2. Plugin downloads and verifies a binary on first use.
3. Homebrew formula once the binary story is stable.

### 13.11. v1 Synthesis, Failure, and Scope Decisions (2026-04-16)

These are the six open questions from the first pass of §13, now resolved.

#### 13.11.1. `plan.md` Template — Rich / Full Structure

The default `plan.md` follows a full-structure template: speakers, decisions with attribution, action items with owners, per-file proposed changes with rationale quotes, risks, and open questions. The synthesis prompt should tell Claude to drop any section that has no real content rather than fabricate entries.

Worked example (the shape Claude should produce):

```markdown
# Meeting Plan — 2026-04-16 19:42

## Speakers
- You, Meeting

## Decisions
- Move auth to JWT (raised by Meeting)
- Drop /v1 endpoints (You agreed)

## Action items
- [ ] Implement JWT verify — owner: You
- [ ] Confirm /v1 sunset date — owner: unclear

## Proposed changes
### `src/auth.ts`
- Why: "we can't keep session state across the new pods"
- Change: swap `lookupSession()` for `verifyJwt()`

## Risks
- JWT move may break existing mobile clients pinned to v1.

## Open questions
- JWT or opaque bearer?
```

Sharp edge to guard against: the Rich template gives Claude room to invent attribution ("raised by Meeting") that isn't grounded in the transcript. The synthesis prompt must instruct Claude to attribute only where the transcript makes it explicit, and to omit the field otherwise.

#### 13.11.2. Empty / Off-Topic Meetings — Bail Out Cleanly

If the meeting produces no actionable technical content tied to this codebase, synthesis bails out:

- `plan.md` is not written.
- The terminal prints a short message, e.g. `No actionable technical content found in this meeting.`
- Audio and `transcript.json` are preserved in the session directory.

This keeps the absence of `plan.md` a meaningful signal: a session either has a plan worth reading, or it does not. No stub plans.

#### 13.11.3. Synthesis Failures — No plan.md, Clear Error

When synthesis fails (API rate limit, network error, malformed transcript):

- `plan.md` is not written. Its presence remains a positive signal.
- Audio and `transcript.json` are preserved.
- The terminal prints a clear error and points at `<session>/synthesis.log`.
- No retry command in this milestone. A `/meeting retry-plan` (or equivalent) is deferred to a later phase. For now the user's recovery path is to re-run `/meeting stop` on a preserved session manually, or accept the failure. Transient-failure recovery is explicitly out of scope here.

#### 13.11.4. Privacy Boundary — Docs + First-Run Banner

The local-vs-remote boundary is surfaced in two places:

- Install / README documentation states it plainly: audio stays local, transcription runs locally via Whisper, transcript text is sent to Anthropic via Claude Code for synthesis.
- The first time the user runs `/meeting start`, the plugin prints a one-time banner summarizing the boundary and asks the user to continue. The acknowledgment is persisted (likely a flag file under `~/.domino/`) so subsequent recordings are silent.

No confirmation on every `/meeting stop`. That would break the "automatic" UX thesis.

#### 13.11.5. `/meeting discard` — Deferred to v1.1

Not in v1. Users can delete a session directory manually if they need to. A proper `/meeting discard` command is deferred to v1.1 once we see whether the gap actually hurts real usage.

#### 13.11.6. System Audio vs Mic-Only — Always Both

v1 always attempts to capture both microphone and system audio. No `--mic-only` flag. Rationale from the user: keep it simple — one mental model, one command, one audio pipeline. The existing recorder behavior still falls back to mic-only if ScreenCaptureKit fails or permission is denied, and that fallback stays.
