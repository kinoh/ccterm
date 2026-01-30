# 2026-01-30 Thread Directories

## Decisions
- Thread sessions run in per-thread subdirectories under `.ccterm/threads/<thread_ts>`.
- Each subdirectory has its own `.claude/settings.json` copied from the base project.
- Hook events are matched by `cwd` to identify which session emitted them.
