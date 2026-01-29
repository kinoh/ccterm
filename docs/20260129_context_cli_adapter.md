# 2026-01-29 Claude Context + CLI Adapter

## Decisions
- Claude Context history includes only user and assistant text.
- Session branching reads full history; normal replies use assistant text only.
- CLI adapter input is plain text; optional thread id is prefixed as `thread:<id> <text>`.
- CLI adapter output pretty-prints `OutgoingMessage` for assistant responses.

## Notes
- No snapshot persistence is required; use `transcript_path` and cutoff timestamps as needed.
