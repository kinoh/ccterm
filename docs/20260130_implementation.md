# 2026-01-30 Implementation Notes

## Slack Adapter
- Use slack-morphism Socket Mode listener with push events for `app_mention`.
- Map Slack `channel` to `conversation_id` and `thread_ts` to `thread_id`.
- Post replies with `chat.postMessage` and `thread_ts` when present.

## Coordinator Behavior
- Serialize outputs by pairing the next hook event with the oldest pending request.
- Requests are tracked per session cwd; hook events are matched by `cwd`.
- Suppress output for seed messages when creating thread sessions.

## Thread Seeding
- Build a single seed prompt from `user` and `assistant` text history.
- Include an instruction: "Do not respond... wait for the next user input."
- Use `transcript_path` from the main session and the incoming message timestamp as cutoff.
- Thread sessions run in subdirectories under `.ccterm/threads/<thread_ts>`.

## Config
- Use `ccterm.toml` by default with required `slack.bot_token` and `slack.app_token`.
- Hook events are read from `hooks.events_path` in the config.
- Added `ccterm.example.toml` as a starting template.
