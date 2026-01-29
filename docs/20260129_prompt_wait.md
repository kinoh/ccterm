# 2026-01-29 Prompt Readiness Check

## Decision
- Added a prompt readiness check using tmux capture-pane output.
- The CLI waits for an idle prompt (`❯` with no input and no "esc to interrupt") before sending input.
- The check is optional via `--wait-prompt` / `--no-wait-prompt` with a configurable timeout.
