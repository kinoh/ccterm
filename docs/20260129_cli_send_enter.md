# 2026-01-29 CLI Send Timing

## Decision
- Send CLI input in two steps: text first, then Enter after a 5ms delay.
- This avoids losing the Enter key when Claude Code is not yet ready to accept input.
