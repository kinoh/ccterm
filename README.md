# ccterm

Slack Socket Mode chat bridge for running Claude Code inside tmux sessions.

## Requirements
- Slack app with Socket Mode enabled.
- App-level token (xapp-) and bot token (xoxb-).
- tmux installed on the host.
- `claude` CLI available in PATH (or configured command).
- Claude hooks configured per project directory.

## Slack permissions (minimum)
App-level token:
- `connections:write`

Bot token:
- `app_mentions:read`
- `chat:write`

Optional (only if posting to public channels without joining):
- `chat:write.public`

## Configuration
Copy `ccterm.example.toml` to `ccterm.toml` and fill in tokens.

```toml
[slack]
bot_token = "xoxb-REPLACE_ME"
app_token = "xapp-REPLACE_ME"

[claude]
command = "claude"
cwd = "."

[tmux]
session_prefix = "ccterm"

[hooks]
events_path = ".claude/hooks/events.jsonl"

[coordinator]
hook_timeout_secs = 10
prompt_timeout_ms = 10000
```

## Claude hooks
Each project directory needs `.claude/settings.json` that runs the hook command.
Use `$CLAUDE_PROJECT_DIR` so per-thread directories resolve correctly.

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "$CLAUDE_PROJECT_DIR/target/debug/ccterm hook --out $CLAUDE_PROJECT_DIR/.claude/hooks/events.jsonl"
          }
        ]
      }
    ]
  }
}
```

## Thread directories
- Thread sessions run in `.ccterm/threads/<thread_ts>`.
- Each thread directory has its own `.claude/settings.json` copied from the base.
- Hook events are matched by `cwd` to identify which session emitted them.

## Run
```bash
cargo run -- serve --config ccterm.toml
```
