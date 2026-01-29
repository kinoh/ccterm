# ccterm

- Launch Claude Code in a terminal (tmux session by default)
- Serve a chat interface via adapters (Slack is one example)
  - Claude Code's responses can be retrieved from `transcript_path` provided by hooks

## Features
- Session model:
  - Main timeline uses a shared session
  - Thread reply spawns a new session seeded from a snapshot at thread start
  - Replies within a thread reuse the thread session (users can join mid-thread)
- Users: no per-user isolation; identify users only by message metadata
- Chat interface: adapter-based integration (Slack only for now)
- Input: Slack RTM mentions to the bot
- Output: post when Stop/SubagentStop/Notification hooks fire
- Process management: start/stop Claude Code and monitor health
- Transcript handling: read/tail/parse JSONL at `transcript_path`
- tmux/Claude Code command: fixed; working directory is fixed
- Error handling: notify Slack on errors (best effort)
- Configuration: TOML (no required keys for minimal setup)
- Deployment: standalone binary (direct execution)
