# wavemelo

Ultra-lightweight Discord music bot written in Rust. Audio is streamed through
a **Lavalink v4** node (with **LavaSrc** and the **youtube-plugin**), so the
bot itself only speaks the Lavalink and Discord gateway protocols — no local
yt-dlp, no Opus decoding, and no separate audio process inside the bot.

YouTube playback works **without a Google OAuth login**: a self-hosted
`yt-cipher` sidecar handles signature/n-parameter deciphering for the plugin.

## Features

- Slash commands via Poise with ephemeral, safe error replies
- `/ping`, `/join`, `/leave`, `/play`, `/skip`, `/stop`, `/queue`
- URL or YouTube search input, including non-Latin text such as Thai and CJK
- Playlists and albums, capped at 200 queued tracks per guild
- Unicode-safe truncation and markdown-safe embeds
- Per-guild operation locks to serialize playback transitions
- JSON structured logs through `RUST_LOG`
- Graceful shutdown on SIGINT/SIGTERM
- Docker Compose deployment with a health-checked Lavalink node
- Zero-OAuth YouTube playback via a private `yt-cipher` sidecar

## Architecture

```
Discord Gateway ──► serenity (poise)
   │                      │
   │ voice state/server   │ slash commands
   ▼                      ▼
lavalink-rs ──► Lavalink v4 node ──► Discord voice servers
                     │        │
                     │        └── yt-cipher (internal sidecar: deciphers
                     │            YouTube signature / n-parameter)
                     ├── youtube-plugin (YouTube / YouTube Music)
                     └── LavaSrc (Spotify, Apple Music, Deezer… off by default)
```

- The bot forwards `VOICE_STATE_UPDATE` / `VOICE_SERVER_UPDATE` gateway
  events to lavalink-rs, which assembles the voice connection info.
- `/join` and `/play` create a Lavalink *player context* per guild; the
  built-in queue auto-advances and stops when empty.
- `/stop` clears the queue and stops the track while staying connected;
  `/leave` deletes the player and disconnects.

## Zero-OAuth YouTube playback

The `youtube-plugin` 1.18.2 clients `ANDROID_VR` and `TV` can demand a Google
sign-in ("This video requires login"), while `WEB`/`WEBEMBEDDED` can fail with
"No supported audio streams available" when YouTube rotates its player
scripts and signature/n-parameter cipher.

wavemelo avoids account login entirely:

1. `lavalink/application.yml` lists only non-OAuth playback clients
   (`WEB`, `MWEB`, `WEBEMBEDDED`, `TVHTML5_SIMPLY`) plus `MUSIC` for search.
2. A private `yt-cipher` service runs on the internal Docker network. Lavalink
   calls it through the documented `plugins.youtube.remoteCipher` config, so
   deciphering happens locally — no Google account, no public API.

The older PoToken generator suggested in plugin docs
(`iv-org/youtube-trusted-session-generator`) is **archived/deprecated** and has
no maintained image; wavemelo does not depend on it.

## Prerequisites

- Rust `1.100.0` with `rustfmt` and `clippy` (pinned in `rust-toolchain.toml`)
- A running **Lavalink v4.2.2** node (`docker compose up` provides one)
- Docker with Compose v2 and network access on first build (the pinned
  `yt-cipher` image is built from its upstream Git repository)
- A Discord bot application with:
  - `bot` and `applications.commands` OAuth2 scopes
  - `Connect` and `Speak` voice permissions
  - Gateway intents: **Server Guilds** and **Guild Voice States**
    (no privileged Message Content intent)

## Configuration

Copy `.env.example` to `.env` and fill in the values:

| Variable | Required | Description |
| --- | --- | --- |
| `DISCORD_TOKEN` | yes | Bot token from the Discord Developer Portal |
| `DISCORD_GUILD_ID` | no | Dev guild for instant command registration |
| `LAVALINK_HOST` | no | Node hostname (default `127.0.0.1`; `lavalink` in compose) |
| `LAVALINK_PORT` | no | Node port (default `2333`) |
| `LAVALINK_PASSWORD` | no | Node password (default `youshallnotpass`) |
| `LAVALINK_SECURE` | no | `true` for TLS/WSS connections (default `false`) |
| `RUST_LOG` | no | Log filter (default `info,wavemelo=debug`) |

Secrets are read at runtime only: `.env` is git-ignored, `AppConfig` redacts
tokens and passwords in its `Debug` output, and user-facing error messages
never contain internal details.

## Running

### Docker Compose (recommended)

```sh
cp .env.example .env   # fill in DISCORD_TOKEN and LAVALINK_PASSWORD
docker compose up --build -d
docker compose logs -f bot
```

Compose runs three services on an internal network:

- `yt-cipher` — built from pinned upstream commit `91d55a2`, listens on 8001
  **inside the network only** (no host port), no API token needed.
- `lavalink` — the audio node, health-checked via a TCP probe on port 2333,
  depending on `yt-cipher`; uses `LAVALINK_SERVER_PASSWORD` env override.
- `bot` — this application, starting only after Lavalink is healthy.

The node's port is published on host loopback only (`127.0.0.1:2333`) so a
local `cargo run` can reach it; it is never exposed to the LAN.

Plugin jars download into the container on first start and re-download if the
container is re-created. Extra sources (Spotify, Deezer, …) are disabled by
default; enable them manually in `lavalink/application.yml` per the LavaSrc
README once you have credentials.

### Local development

```sh
docker compose up lavalink -d   # also starts yt-cipher as a dependency
cargo run
```

## Commands

| Command | Description |
| --- | --- |
| `/ping` | Gateway and HTTP latency check |
| `/play <query>` | Play a URL or search YouTube; queues when busy |
| `/skip` | Skip the current track |
| `/stop` | Stop playback and clear the queue (stays connected) |
| `/volume <0..150>` | Set playback volume (100 = default level) |
| `/pause` | Pause the current track (same voice channel required) |
| `/resume` | Resume a paused track (same voice channel required) |
| `/seek <position>` | Seek to `90`, `01:30`, or `00:01:30` seconds |
| `/nowplaying` | Show track, position, volume, loop, filter, and status |
| `/loop <off\|track\|queue>` | Set repeat mode for this guild |
| `/filter <preset>` | Apply bassboost, nightcore, vaporwave, 8d, karaoke, or clear |
| `/queue` | Show the current track and the first page of queued tracks |
| `/queueremove <index>` | Remove a queued track by its 1-based position |
| `/queuejump <index>` | Play a queued track immediately |
| `/queuemove <from> <to>` | Move a queued track to a new position |
| `/queueclear` | Remove all queued tracks |
| `/shuffleq` | Randomize upcoming tracks (loop mode must be off) |
| `/history` | Show recently played tracks for this guild |
| `/join` | Join your current voice channel |
| `/leave` | Leave voice and clear the queue |

All bot-controlled replies and embeds are in English. Track titles, artist
names, lyrics, and user-supplied queries keep their original language.

`/loop` offers built-in autocomplete. `/play` suggests up to five recently
played titles without contacting external services.

`/nowplaying` includes buttons for pause/resume, skip, stop, and volume steps.
Controls require the same voice channel and expire automatically when the
track changes; use `/nowplaying` again for fresh controls.

## Development

Quality gates used by CI:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

- Lavalink config lives in `lavalink/application.yml` (plugins, sources,
  remote cipher, timeout tuning); the password is overridden via environment
  in `docker-compose.yml`.
- Release verification notes are collected in `docs/RELEASE.md`.
