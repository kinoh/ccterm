## Decision
Resolve Slack user IDs to display names and prefix incoming messages with "DisplayName: ".

## Context
Slack `app_mention` text includes `<@U...>` tokens, which are not human-readable in `CLAUDE.md` or the Claude input.
The user wants the prior messages to show display names (e.g., "Alice: hello") instead of raw mention tokens.

## Why This Change
- Improves readability for both the model and humans.
- Removes bot-mention noise from the text while preserving the sender identity.

## Implementation Notes
- Use `users.info` to resolve the sender's display name.
- Cache user ID -> display name to reduce API calls.
- Prefer `profile.display_name`, then fall back to `profile.real_name`, `real_name`, and `name`.
- Requires the `users:read` bot scope.
