## Decision
Replace the seed message with an optional `CLAUDE.md` file in each thread directory to provide background context.

## Context
Seeding the new thread session with a synthetic "conversation so far" message consumes a full turn and can suppress
the first real reply. The user wants to avoid spending a turn while still giving Claude optional context.

## Why This Change
- The context is supplemental and should not be required focus.
- A thread-local `CLAUDE.md` is automatically read when the session cwd is the thread directory.
- This preserves context without using a model turn.

## User Feedback
The user requested removing the seed turn and asked for an alternative such as a file in the thread directory.
They also want the text to clearly indicate it is optional background.

## Implementation Notes
- On thread creation, generate `CLAUDE.md` if it does not already exist.
- Write `CLAUDE.md` before spawning the Claude session so it is loaded at startup.
- The file explicitly states that the context is optional and may be ignored.
- Context is derived from the main conversation transcript up to the incoming message timestamp.
- Log why `CLAUDE.md` was not written when context cannot be built.
- Compare Slack timestamps and transcript ISO timestamps by parsing both into epoch nanoseconds.
