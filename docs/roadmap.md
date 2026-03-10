# Roadmap Notes

## Slack bot integration (`pi-bot-slack`)

Slack bot support is intentionally **deferred** for the current milestone.

- Workspace feature gate: `slack-bot`.
- Current state: gate is reserved for future optional integration and does not compile extra crates yet.
- Planned follow-up:
  1. Add `crates/pi-bot-slack` with a thin adapter from Slack events into `pi-core` sessions.
  2. Add per-channel/per-thread session routing.
  3. Add capability hardening presets for bot deployments.
