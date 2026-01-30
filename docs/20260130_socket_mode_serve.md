# 2026-01-30 Socket Mode Serve

## Background
When running `ccterm serve`, the process exited immediately after logging
`socket mode listener starting`. That meant no Slack events could be received
and the coordinator loop terminated because the incoming channel closed.

## Why this happened
`SlackClientSocketModeListener::start()` only kicks off internal tasks and then
returns right away. Because the listener is created inside `SlackAdapter::connect`
and not stored anywhere, it is dropped as soon as `connect()` returns. That drop
closes the internal user state (including the `SlackBridge` sender), so the
receiver side sees `None` and the coordinator loop ends.

## Decision
Switch to `SlackClientSocketModeListener::serve()` instead of `start()`.
`serve()` keeps the socket-mode loop alive until termination signals are
received, which prevents the listener from being dropped and keeps the receiver
channel open. This matches the library's intended lifecycle for long-running
listeners.
