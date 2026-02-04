## Decision
Ensure Slack replies are sent only on `Stop` hooks and do not require a pending queue entry to send.

## Context
Current behavior can drop replies when `SubagentStop` or `Notification` consumes the pending queue before `Stop` arrives.
This results in `Stop` events logging "received hook with no pending request" and no Slack response, even though a new
assistant message exists in the transcript.

## Why This Change
- The user wants replies to be sent whenever a new assistant message exists on `Stop`.
- Intermediate events are not needed.
- The "one user input -> one reply" constraint is unimportant compared to avoiding reply loss.

## User Feedback
The user explicitly stated that reply loss is the main problem and that intermediate progress is unnecessary.

## Alternatives Considered
- Keep the pending queue as a strict gate and handle all hook events.
  - Rejected because it can drop `Stop` replies when earlier events consume the queue.
- Send on every hook event that has content.
  - Rejected because it risks duplicate replies and sending non-final content.

## Implementation Notes
- Treat only `Stop` as a send trigger.
- Remove the pending queue gate from reply delivery.
- Deduplicate by the latest assistant message UUID to avoid sending the same `Stop` twice.
- Log when a `Stop` hook arrives but no assistant text or UUID change is detected.
- Seeded context is replaced by optional thread context in `CLAUDE.md`.
