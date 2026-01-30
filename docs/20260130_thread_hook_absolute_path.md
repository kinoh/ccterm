# 2026-01-30 Thread Hook Absolute Path

## Background
Thread sessions copy `.claude/settings.json` from the base project directory.
Those settings use `$CLAUDE_PROJECT_DIR/target/debug/ccterm` as the hook command.
In thread directories, `target/` does not exist, so hooks fail and no
`transcript_path` updates are recorded.

## Decision
When creating a thread directory, render a thread-specific settings file and
replace the hook command with the absolute path of the running `ccterm` binary.
This keeps hook output scoped to the thread directory while ensuring the hook
command resolves in environments without `target/`.
