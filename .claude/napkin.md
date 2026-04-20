# Napkin Runbook

## Curation Rules
- Re-prioritize on every read.
- Keep recurring, high-value notes only.
- Max 10 items per category.
- Each item includes date + "Do instead".

## Execution & Validation (Highest Priority)
1. **[2026-04-16] Verify Domino from the Rust recorder binary first**
   Do instead: use `recorder/target/release/domino-codex-recorder` for Codex manual checks before assuming any plugin workflow exists; keep `domino-recorder` only as the retained Claude compatibility alias.
2. **[2026-04-16] Match recorder artifacts by session directory, not bare filename**
   Do instead: compare `meeting.opus`, `transcript.json`, and logs inside the same `~/.domino/recordings/<YYYY-MM-DD-HHMM>/` folder because every session reuses the same filenames.
3. **[2026-04-19] Domino has parallel Claude and Codex plugin surfaces**
   Do instead: when changing meeting workflow behavior, update both `plugin/commands/` and `plugins/domino/skills/` plus their READMEs so the two plugin entry points stay aligned.
4. **[2026-04-19] Codex plugin commands cannot rely on your interactive shell `PATH`**
   Do instead: resolve repo-local binaries like `./recorder/target/release/domino-codex-recorder` explicitly inside plugin skill instructions, then fall back to `domino-codex-recorder` on `PATH` only as a backup.
5. **[2026-04-19] Standalone Codex installs use repo marketplaces, not Claude-style plugin marketplace add flows**
   Do instead: expose Codex plugins through `$REPO_ROOT/.agents/plugins/marketplace.json`, instruct users to open Codex in the cloned repo and install from `/plugins`, and keep public docs aligned with Codex marketplace behavior.
6. **[2026-04-16] Treat phase coverage as partial until the plan says otherwise**
   Do instead: check the relevant phase section in `thoughts/shared/plans/2026-04-15-domino-v1-macos-audio-capture.md` and then confirm the matching code paths in `recorder/src/`.
7. **[2026-04-16] Concurrency automation is not green yet**
   Do instead: if `cargo test` fails in `tests/concurrent_start.rs`, rely on direct manual `start`/`status`/`stop` verification for lifecycle behavior and call out the gap explicitly.
8. **[2026-04-16] Phase 3 system-audio verification depends on logs, not `status`**
   Do instead: inspect `<session>/recorder.log` for `starting system audio capture via ScreenCaptureKit` or the mic-only fallback warning because `status` still only prints PID/session metadata.
9. **[2026-04-16] CI exists, but plugin release automation does not**
   Do instead: treat `.github/workflows/ci.yml` as Rust quality gating only and plan a separate release workflow before assuming prebuilt plugin binaries can be shipped.

## Shell & Command Reliability
1. **[2026-04-16] `starter_pack` must stay as plain files in the top-level repo**
   Do instead: keep `starter_pack/.git` out of the workspace before staging so Git tracks the folder contents instead of an embedded repo link.
2. **[2026-04-16] Build commands are anchored on the recorder manifest**
   Do instead: run `cargo build --release --manifest-path recorder/Cargo.toml` or `cargo test --manifest-path recorder/Cargo.toml` from repo root.
3. **[2026-04-16] Release recorder builds on this machine need the 15.4 macOS SDK**
   Do instead: run `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk cargo build --release --manifest-path recorder/Cargo.toml` when `screencapturekit` is in the build.
4. **[2026-04-16] Recorder CLI invocations on this machine need an explicit Swift runtime fallback**
   Do instead: prefix manual recorder runs and `cargo test` with `DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx` until the binaries embed a working rpath.
5. **[2026-04-16] Phase 3 whisper smoke tests need explicit SDK and Swift runtime env on macOS**
   Do instead: export `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk` and `DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx` before running the targeted `cargo test` commands.
6. **[2026-04-16] `whisper-rs` rebuilds on this machine miss libc++ headers by default**
   Do instead: when `cargo build` or `cargo test` compiles `whisper-rs-sys`, export `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.2.sdk`, `CXXFLAGS='-I /Library/Developer/CommandLineTools/SDKs/MacOSX26.2.sdk/usr/include/c++/v1'`, and `DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx`.
7. **[2026-04-16] Terminal commands for this user must be copy-safe**
   Do instead: prefer single-line commands; if a command truly needs multiple lines, use explicit continuations or fenced blocks that paste cleanly into `zsh`.
8. **[2026-04-16] Sandboxed release builds can fail inside `screencapturekit`'s Swift bridge**
   Do instead: if `cargo build --release` dies with `sandbox-exec: sandbox_apply: Operation not permitted`, rerun the release build outside the workspace sandbox instead of changing Rust code.

## Domain Behavior Guardrails
1. **[2026-04-16] `doctor` is still a stub**
   Do instead: do not route users through `domino-recorder doctor` for permissions until Phase 4 lands; use direct macOS permission steps instead.
2. **[2026-04-16] Current manual verification is terminal-driven, not browser-driven**
   Do instead: treat the browser as an optional sound source or meeting simulator; the authoritative checks are the saved Opus file, `status`, `ps`, and `ffprobe`.
3. **[2026-04-16] Keep transcription downstream of the saved session artifact**
   Do instead: treat `~/.domino/recordings/<session>/meeting.opus` as the stable handoff and write transcript outputs beside it rather than adding model work into the live capture loop.
