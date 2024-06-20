# Infernoplex

Infernoplex is the server listing bot for Infinity List. *Self-hosting is not officially supported. Licensed under the AGPLv3*

## Components

- Sorbet: The webserver responsible for administration, creating server invites for users etc.
- Shadowclaw: Common code used by infernoplex

## Invite syntax

- ``none`` -> invites are disabled for this server
- ``invite_url:{invite}`` -> fixed URL invite where ``{invite}`` is the invite URL
- ``per_user:{channel_id}:{max_uses}:{max_age}`` -> per-user invite where ``{channel_id}`` is the channel ID, ``{max_uses}`` is the maximum number of uses, and ``{max_age}`` is the maximum age of the invite in seconds