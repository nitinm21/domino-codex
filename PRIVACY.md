# Domino for Codex Privacy

Domino for Codex is designed to keep meeting data local.

- Audio recordings are written to `~/.domino/recordings/` on the local machine.
- Transcription runs locally with Whisper. Audio is not uploaded by the recorder.
- Recorder state and downloaded models are stored under `~/.domino/`.
- Plan generation and optional execution happen inside the user's existing Codex session.
- Domino does not run `git push` and does not open pull requests.

If you choose to share transcript files, recordings, or repository output yourself, that sharing happens outside Domino's control.
