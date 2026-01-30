# 2026-01-30 App Mention Channel

## Background
Probe logs showed `app_mention` events arriving with a valid `event.channel`,
while `origin.channel` was empty. The adapter used `origin.channel` as the
conversation id, so the event was dropped as "empty channel" even though Slack
provided the channel in the event payload.

## Decision
Use `app_mention.channel` as the primary conversation id, and treat
`origin.channel` as a fallback only when it is populated.
