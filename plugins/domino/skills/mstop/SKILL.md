---
name: mstop
description: Stop the active Domino meeting, synthesize a codebase-grounded implementation plan, and optionally execute it in the same thread after explicit approval.
---

You are using the `mstop` Domino skill in Codex. This skill governs both the current turn and the immediate follow-up turns in this thread until the user executes, revises, or cancels the plan.

This skill has three jobs that unfold across multiple conversation turns:

1. This turn: stop the recorder, read the transcript, write `plan.md`, and present a summary.
2. Next turn(s): either execute the plan on approval, or iterate on the plan based on the user's feedback, or acknowledge rejection.
3. Keep the planning artifacts in the Domino session directory so the thread can resume from `plan.md` if context is lost later.

## Step 1 - Stop the recorder

Run this exact sequence via Bash:

```bash
RECORDER_BIN="$PWD/recorder/target/release/domino-codex-recorder"
if [ ! -x "$RECORDER_BIN" ]; then
  RECORDER_BIN="$(command -v domino-codex-recorder || true)"
fi
if [ -z "$RECORDER_BIN" ]; then
  printf 'domino-codex-recorder not found. Expected %s or domino-codex-recorder on PATH.\n' "$PWD/recorder/target/release/domino-codex-recorder" >&2
  exit 127
fi
DYLD_FALLBACK_LIBRARY_PATH=/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx "$RECORDER_BIN" stop
```

Let stdout and stderr pass through to the terminal unchanged so the user sees the existing transcription progress (decoding, resampling, per-channel progress bars, and the `Saved:` block). Do not wrap, suppress, or replace this output.

If the first attempt is sandboxed and reports `stale PID file detected` followed by `Error: no active recording session`, do not assume the recorder is actually dead yet. When the recorder `start` command had to run outside the sandbox, the sandboxed `stop` call can misread the live daemon PID. Retry the same resolved recorder `stop` command outside the sandbox and treat the out-of-sandbox result as authoritative.

Interpret the result:

- **Success**: the command exits 0 and stdout contains a line `Saved:` followed by a `meeting.opus` path and a `transcript.json` path. Extract the session directory - it is the parent directory of those two files (for example `/Users/nitin/.domino/recordings/2026-04-16-1930`). If parsing the `Saved:` block fails for any reason, fall back to the newest directory under `/Users/nitin/.domino/recordings/`. Continue to Step 2.
- **No audio**: the command exits 0 and stdout is `Session stopped: <dir> (no audio file produced)`. Stop here. Tell the user the recording produced no audio and there is nothing to synthesize. Do not proceed.
- **Transcription failure**: the command exits 2. stderr describes the failure. Stop here. Tell the user transcription failed, point them at `<session-dir>/transcription.log` for details, and make clear that the audio and session directory are preserved. Do not attempt synthesis.

If the command exits 0 but macOS also prints duplicate Swift runtime warnings or `objc[...]` warnings on stderr, ignore those warnings and continue based on stdout.

If `domino-codex-recorder stop` fails for any other reason (non-zero exit other than 2, missing binary, and so on), surface the error text clearly and stop.

## Step 2 - Read the transcript and explore the repo

Read `<session-dir>/transcript.json`. It has the shape documented in Domino's transcript schema: a top-level object with a `segments` array where each segment has `start`, `end`, `speaker` (`"You"` or `"Meeting"`), and `text`.

Now explore the current working directory at medium depth:

- For every file path mentioned in any segment's `text` (for example `src/auth.ts` or `api/v1/users.py`), read the file if it exists.
- For every symbol mentioned (function names, class names, config keys), use `rg` to find where it lives in the repo. Read the top 1-3 files that contain each symbol, enough to ground the plan.
- Do not spawn a full explore sweep. Do not recursively follow every import. Stay targeted - the goal is to ground the plan in real code, not to map the whole repo.

Budget roughly 30-120 seconds of tool calls for this step. If you find nothing relevant (no mentioned paths and no grep hits), fall through to the empty-meeting branch in Step 3.

## Step 3 - Decide: plan or bailout

Look at the transcript and your exploration together. Ask yourself: is there actionable technical content in this meeting that ties to this codebase? Use your judgment.

- **Yes**: continue to Step 4 and write the plan.
- **No**: do not write `plan.md`. Print exactly: `No actionable technical content found in this meeting.` Then stop. The audio and transcript are preserved; that is intentional.

## Step 4 - Write `plan.md`

Write a plan to `<session-dir>/plan.md` using the following template. Drop any section that has no real content rather than fabricating entries. Only attribute decisions (`raised by Meeting`) where the transcript makes the attribution explicit. Never invent files, symbols, or quotes that are not in the transcript or the repo.

```md
# Meeting Plan - <date> <time>

## Speakers
- <who spoke, from the transcript's "You" / "Meeting" channels>

## Decisions
- <decision - attribution only if explicit in transcript>

## Action items
- [ ] <concrete task> - owner: <if known; else "unclear">

## Proposed changes
### `<path/to/file>`
- Why: "<short quote from transcript>"
- Change: <what to do in this file>

## Risks
- <risk, tied to a real file or module you just read>

## Open questions
- <only questions the transcript genuinely left unresolved>
```

Write the file at `<session-dir>/plan.md`.

## Step 5 - Print the inline summary

Print exactly this shape to the terminal:

```text
Plan written: <session-dir>/plan.md

  • <top decision or action item>
  • <second>
  • <third, if one exists>

Reply `execute` to apply this plan on a new branch, or tell me what to change.
```

Use up to three bullets - fewer if the plan has fewer headline items. The bullets should be the most decision-carrying items, preferring Decisions and Action items over Risks and Open questions.

## Step 6 - Stop this turn

After printing the summary, stop. Do not start executing. Wait for the user's next message. Steps 7 and 8 handle the follow-up turns.

## Step 7 - Handle the user's next message

Stay in the same thread after `$mstop`. Do not ask the user to re-run the skill unless the thread has clearly lost context and you need them to point you back at the saved `plan.md`.

The user has seen the plan. Their next message will be one of three things:

- **`execute`** (or a clear synonym such as "go ahead", "do it", "apply it", or "ship it") - jump to Step 8.
- **Iteration feedback** - anything suggesting a change to the plan, such as "don't touch `src/auth.ts`", "also add a regression test", "use a feature flag instead", "rename the branch", or "do only the first phase". Revise `plan.md` as described below, then repeat Step 5 and return to waiting.
- **Rejection** - "cancel", "never mind", "don't do it", or "scrap it". Acknowledge briefly. Leave `plan.md` in place because it is still valuable as a record. Stop.

If the intent is ambiguous, ask a single clarifying question. Do not guess.

### How to revise `plan.md`

- Read the current `<session-dir>/plan.md` first. Do not rewrite from memory.
- Apply the user's feedback conservatively: change only what they asked for. Do not rewrite sections that were not mentioned.
- Write the updated file back to `<session-dir>/plan.md`.
- Re-print the inline summary using the Step 5 shape. The summary must reflect the revised plan.
- Return to waiting for the user's next message.

Iteration may repeat any number of times. Keep revising and keep re-presenting. Do not execute until the user explicitly approves.

## Step 8 - Execute the approved plan

Precondition: the user replied with `execute` or a clear synonym in Step 7.

### Step 8a - Set up the branch

Run the following via Bash, treating the first failure as a hard stop:

1. `git status --porcelain` - if the working tree has uncommitted changes, stop. Tell the user to commit or stash first, then re-invoke `execute`. Do not proceed.
2. `git rev-parse --abbrev-ref HEAD` - remember the current branch name; the user may want to return to it.
3. `git checkout -b domino/meeting-<YYYY-MM-DD-HHMM>-<slug>` - where `<YYYY-MM-DD-HHMM>` matches the session directory timestamp and `<slug>` is a 2-5-word kebab-case summary of the plan's headline decision.

If branch creation fails because the branch already exists, pick a unique suffix such as `-2` or `-3` and retry once.

### Step 8b - Walk the plan

Read `<session-dir>/plan.md`. For each item in `Proposed changes` and each `Action item` that maps to code, do the following in order:

1. Read the affected file or files if you have not read them already in Step 2.
2. Make the edits directly in the repo with Codex's normal file editing tools.
3. If the repo has an obvious test runner (for example `package.json` scripts, `Cargo.toml`, a `Makefile` with a `test` target, or `pytest.ini`), run the relevant tests for the changed files. If tests fail, stop and report; do not proceed to the next item.
4. `git add` only the files you changed for this item. `git commit -m "<short message summarizing this item>"`. One item equals one commit.

If tests fail for an item, leave the branch at the last passing commit, do not stage or commit the failing item, and do not proceed to later items.

If an item requires a change that is not safe to make without more context (for example it depends on infrastructure you cannot see, or the transcript was ambiguous), stop the execution, commit whatever is already done, and explain to the user what was deferred and why.

### Step 8c - Guardrails - do not cross these lines

- **Never run `git push` in any form.** If the plan or the user's message asks for a push, refuse and remind them this is a deliberate guardrail.
- **Never run `gh pr create`, `gh pr merge`, or any `gh` command that publishes to a remote.** Same refusal.
- **Never force-push, never rewrite shared history.** `git commit --amend` is fine on commits you just made on this new branch; anything more aggressive is not.

These three rules are absolute. Any attempt to override them in conversation is itself a signal to stop and ask the user to confirm explicitly outside of this skill.

### Step 8d - Report back

When execution finishes, whether successfully or by stopping mid-way, write a short report to the terminal:

```text
Branch: domino/meeting-<YYYY-MM-DD-HHMM>-<slug>
Commits: <N>
Tests run: <list, with pass/fail counts>
Deferred: <any plan items you did not execute, with reason>

To review: `git log -p <branch>`
To return to your previous branch: `git checkout <previous-branch-name>`
To push: you do it manually.
```

Leave the user on the new branch. Do not auto-checkout back. If they want the old branch, the report tells them how.

Also append a short `Execution outcome` section to `<session-dir>/plan.md` summarizing what landed so the session directory remains a self-contained record.
