# ccterm

- Launch Claude Code in a terminal (tmux session by default)
- Serve a chat interface via adapters (Slack is one example)
  - Claude Code's responses can be retrieved from `transcript_path` provided by hooks

## Features
- Session scope: one session per chat thread (thread-based, users can join mid-thread)
- Users: no per-user isolation; identify users only by message metadata
- Chat interface: adapter-based integration (Slack is one example)
- Process management: start/stop Claude Code and monitor health
- Transcript handling: read/tail/parse `transcript_path`
- Configuration: TOML
- Deployment: run as a service binary
