# Infernoplex

Infernoplex is the server listing bot for Omniplex. Self-hosting is not
officially supported. Licensed under the AGPLv3.

## Components

- Sorbet: the internal HTTP API responsible for creating server invites for
  users, checking whether the bot is present in a guild, and other
  administrative actions other services (Popplio, Omniplex) need without
  running their own Discord gateway connection.
- Shadowclaw: shared code used throughout Infernoplex (invites, permission
  checks, guild stats, image handling).

## Requirements

Infernoplex is a Rust project built with `poise`/`serenity` (Discord),
`axum` (Sorbet), and `sqlx` (Postgres). All platforms need:

- Rust (stable toolchain, installed via [rustup](https://rustup.rs))
- Git, since several dependencies are pulled directly from GitHub rather
  than crates.io (`poise`, `serenity`, `kittycat`, `botox`)
- A reachable Postgres database
- A Discord bot token and client secret

### Windows

The build compiles `libgit2` from source (used by the `vergen` build
dependency to embed the current git commit/version into the binary), which
requires a C/C++ toolchain. Install
[Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
with the "Desktop development with C++" workload selected. Without this,
the build fails during the `libgit2-sys` build script with errors about
missing `cl.exe` or `git2/sys/features.h`.

### Linux

Install the standard build toolchain and headers needed for the same
native dependencies:

```bash
sudo apt install build-essential cmake pkg-config
```

## Configuration

Infernoplex reads its runtime configuration from `config.yaml`, not
environment variables. On first run, if `config.yaml` does not exist, the
binary writes out `config.yaml.sample` with the default shape and exits.
Copy that to `config.yaml` and fill in real values (database URL, bot
token, client secret, prefix, server IDs, frontend URL, etc.) for each
environment.

Which environment is active is controlled by the `current-env` file at the
project root, containing `staging`, `prod`, or `dev`. Config fields that
differ between environments (token, prefix, frontend URL, server port) are
defined as `{staging: ..., prod: ...}` pairs in `config.yaml` and resolved
against `current-env` at startup.

Each of those pairs also accepts an optional `dev: ...` value, only
consulted when `current-env` is `dev`, and only used if actually set — an
unset `dev` falls back to `staging`. This lets a local checkout run against
a personal Discord bot application (`token`) without touching the real
staging config, and without needing a `dev` value for every field, just the
ones that matter locally. Background tasks that touch real guilds only run
when `current-env` is `prod`; `staging` and `dev` both skip them.

Separately, `sqlx`'s `query!` macros verify SQL against a real database at
**compile time**, either a live connection or a cached `.sqlx/` directory.
This is controlled by the `DATABASE_URL` environment variable, independent
of `config.yaml`'s own `database_url` field (which the built binary uses
at runtime). Copy `.env.sample` to `.env`, or set `DATABASE_URL` directly
in your shell, pointing at a reachable Postgres instance before building.

## Building and running

```bash
cargo build            # debug build
cargo build --release  # release build
cargo run               # build and run
```

If `sqlx::query!` fails to compile with an error about missing offline
query data, refresh the cache against a live database:

```bash
cargo install sqlx-cli --version 0.8.3 --no-default-features --features rustls,postgres
cargo sqlx prepare
```

Use the `0.8.x` line of `sqlx-cli` to match this project's `sqlx = "0.8"`
dependency; the `0.9.x` line also requires a newer Rust toolchain than a
default rustup install typically has.

## Deployment

`Makefile` targets are used for deploying to the actual staging/prod
servers over `systemd`, and assume a Linux host:

- `make all` builds a release binary.
- `make restartwebserver` regenerates the `sqlx` offline cache, builds,
  and restarts the `infernoplex-<current-env>` service.
- `make restartwebserver_nobuild` restarts the service using the
  already-built binary, without rebuilding.
- `make promoteprod` copies the current staging checkout to prod, deploys
  it, and pushes the result to the `current-prod` branch.
- `make ts` regenerates and publishes the TypeScript bindings exported via
  `ts-rs` to the CDN.

## Invite syntax

- `none`: invites are disabled for this server.
- `invite_url:{invite}`: a fixed URL invite, where `{invite}` is the
  invite URL.
- `per_user:{channel_id}:{max_uses}:{max_age}`: a per-user invite, where
  `{channel_id}` is the channel ID, `{max_uses}` is the maximum number of
  uses, and `{max_age}` is the maximum age of the invite in seconds.
