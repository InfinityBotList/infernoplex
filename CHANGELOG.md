# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

### Added

- `current-env` now also accepts `dev`, a third environment alongside
  `staging`/`prod`. Every `Differs<T>` config key (`src/config.rs`) gains an
  optional `dev` value, only consulted when `current-env` is `dev`, and only
  used if actually set — an unset `dev` falls back to `staging`, so no
  existing `config.yaml` needs to change. Lets a local checkout run against
  a personal Discord bot application (`token`) without touching the real
  staging config. Background tasks that touch real guilds now only run when
  `current-env` is `prod`; previously they ran on anything that wasn't
  `staging`, which would have included `dev`.
- `/setup`: creates a server's listing (team, vanity, invite resolution,
  and an initial member-count/online-count snapshot) from within the guild
  being listed.
- `/update`, `/delete`, `/leaderboard`, `/stats`, `/help` commands.
- Sorbet: an internal HTTP API (`POST /`, same externally-tagged
  single-endpoint pattern as Arcadia's `panelapi`) used by Popplio and
  Omniplex to talk to the bot without needing their own gateway
  connection — `CreateInvite`, `ResolveInvite`, and `IsInGuild` (checks
  whether the bot is currently a member of a given guild, and returns a
  ready-to-use invite URL for it — used to nudge server owners to invite
  the bot when features that need it, like emoji/sticker sync, won't work
  without it).
- `teamcleanup` background task: periodically removes a user from a
  server's team once they've left the guild or lost Administrator, checked
  via REST rather than reacting to gateway events.
- `serversync` background task: for every server that's opted in via
  `show_emojis`, periodically snapshots the guild's custom emojis and
  stickers into the database (skipping, not clearing, any server the bot
  currently can't reach — REST calls to a guild the bot has left simply
  fail and that row is left untouched until it's reachable again).

### Fixed

- `Differs<T>`'s `staging`/`prod` fields required both to be present in
  `config.yaml` even though only one is ever read for a given
  `current-env` — a staging-only or prod-only config failed to parse at
  all (`missing field`) instead of just leaving the unused environment's
  value at its default.
- Sorbet's `IsInGuild` `invite_url` was a generic "add this app to any
  server you manage" OAuth link, unrelated to the `guild_id` that was
  actually being checked — it now includes `&guild_id={guild_id}
  &disable_guild_select=true` so the invite flow is locked to the
  specific server being added instead of leaving it to the owner to pick
  correctly out of every server they manage.

### Changed

- `proxy_url` now defaults to `https://gateway.nodebyte.host/proxy/discord`
  (the shared parent-company gateway), replacing the old local
  `http://127.0.0.1:3219` twilight-http-proxy convention. Since that gateway
  authenticates every request with its own shared bot credential by
  default, Infernoplex's REST client now sends its own token via an
  `X-Upstream-Authorization` header instead, which the gateway forwards as
  the real `Authorization` header sent to Discord — so Infernoplex keeps
  its own bot identity rather than authenticating as whichever bot the
  gateway holds.
- All user-facing commands (`/setup`, `/update`, `/delete`, `/leaderboard`,
  `/stats`, `/help`) are now slash-command only, prefix-command support
  was dropped for these. `!register` (or whatever prefix is configured)
  remains prefix-only, since it's the bootstrap command needed to register
  slash commands with Discord in the first place and can't be a slash
  command itself yet on a fresh install.
- Infernoplex no longer requests the privileged Server Members or Presence
  gateway intents, and never will regardless of scale, so it never needs
  to go through Discord's intent verification review. Everything that
  depended on them was moved to REST instead:
  - Team-member cleanup moved from reactive gateway event handlers to the
    `teamcleanup` polling task, using per-member REST lookups (not the
    privileged bulk `List Guild Members` endpoint).
  - `/setup`'s member/online counts now come from
    `GET /guilds/{id}?with_counts=true` (Discord's approximate counts)
    instead of the gateway presence cache.
  - `/setup` no longer bulk-scans guild members to auto-add every
    Administrator to the new team, since that also requires the privileged
    Server Members intent with no REST alternative. It now adds the guild
    owner and, if different, whoever ran the command; other admins are
    added manually afterward via Team Settings, as the setup confirmation
    message already says.
