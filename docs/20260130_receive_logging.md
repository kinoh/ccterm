# 2026-01-30 Receive Path Logging

## Background
Slack mentions were not reaching the coordinator. We needed to determine whether
events were missing at the socket layer, filtered inside the adapter, or dropped
before they reached the coordinator.

## Why
The existing logs only confirmed connection setup. They did not show:
- whether a Socket Mode hello was received,
- whether any event types other than app_mention were arriving,
- whether app_mention contained usable channel data,
- or whether the coordinator received anything at all.

Without that visibility, any fix would be guesswork.

## Decision
Add probe logs along the receive path:
- Socket Mode hello reception.
- All received event types (including non-app_mention).
- app_mention field dumps (event channel vs. origin channel, thread, text length).
- Coordinator receipt of incoming Slack messages.

These logs let us identify exactly where the event flow stops before making
behavioral changes.
