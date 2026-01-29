# 2026-01-29 Hooks & Sessions Validation

## Decisions
- Implemented a minimal Rust CLI with `run` and `hook` subcommands to validate the flow.
- The hook receiver appends raw JSON to `.claude/hooks/events.jsonl` for inspection.
- Project hook settings use `$CLAUDE_PROJECT_DIR` to call `target/debug/ccterm hook`.
- tmux session names are derived from a UNIX timestamp.
- The run flow can optionally auto-confirm the trust prompt by sending Enter.

## Validation Result
- Verified: start Claude Code in tmux, send a message, and receive a Stop hook event.
- The hook event file captured `hook_event_name: Stop` with `transcript_path`.

## Notes from User
- Keep scope to "send message -> receive hook event" only.
- Place `.claude/settings.json` in this repo.
