# Contributing to Domino for Codex

Notes for contributors working on the Codex plugin surface or the recorder.

## Repo Layout

```text
domino/
├── plugins/domino/                # Codex plugin: manifest, skills, and user-facing README
├── .agents/plugins/marketplace.json
│                                 # Repo marketplace that exposes the Codex plugin
├── recorder/                      # Rust crate: audio capture + local Whisper transcription
├── plugin/                        # Retained Claude Code plugin files for reference
├── install.sh                     # Public installer for the recorder binary
└── .github/workflows/             # CI and release automation
```

## Codex Plugin Development

Codex uses repo or personal marketplaces rather than Claude-style `/plugin marketplace add` flows.

For this repo, the authoritative install surface is:

- plugin files under `plugins/domino/`
- repo marketplace metadata at `.agents/plugins/marketplace.json`

To test locally:

1. Start Codex from the repo root:

   ```bash
   cd /Users/nitin/domino-codex/domino
   codex
   ```

2. Open `/plugins`.
3. Select the marketplace exposed by `.agents/plugins/marketplace.json`.
4. Install or reinstall `Domino`.
5. Restart Codex after plugin changes so the cached install picks up the new files.

## Recorder Builds

Base release build:

```bash
cargo build --release --manifest-path recorder/Cargo.toml
```

That build now emits both `recorder/target/release/domino-codex-recorder` for the Codex distribution and `recorder/target/release/domino-recorder` as a retained compatibility alias for the Claude-facing files kept in this repo.

If the ScreenCaptureKit or Swift link step fails on this machine, retry with the explicit SDK path:

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk cargo build --release --manifest-path recorder/Cargo.toml
```

### Running the Recorder During Development

On machines where the embedded Swift runtime rpath does not resolve cleanly, prefix manual recorder invocations with:

```bash
export DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx
```

End users should not need this in the intended install flow. If they do, capture the exact `dyld` error and treat it as a packaging or release issue.

## Tests

```bash
cargo fmt --manifest-path recorder/Cargo.toml --check
cargo clippy --manifest-path recorder/Cargo.toml -- -D warnings
cargo test --manifest-path recorder/Cargo.toml
```

CI runs these on every push and pull request.

## Releases

Releases are cut by pushing a `v*` tag in this repository. The release workflow builds both recorder binaries, packages the Codex-facing `domino-codex-recorder` darwin-arm64 artifact, computes a SHA256 checksum, and uploads both release assets to GitHub.

The installer downloads from `nitinm21/domino-codex`, not the old `nitinm21/domino` repository.

Use `-rcN` or `-betaN` suffixes for prereleases while validating the public install flow.

## Claude Files

The `plugin/` directory remains in this repository for transition and reference only. Do not treat it as the primary public install surface for `domino-codex`.

If you change shared workflow behavior, keep the Codex plugin files under `plugins/domino/` and the Claude files under `plugin/` aligned where intentional.
