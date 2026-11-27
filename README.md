# wavemelo

Ultra-lightweight Discord music bot written in Rust. No Lavalink, no separate server process: Songbird handles voice, and `yt-dlp` resolves YouTube URLs and searches.

## Features

- Slash commands via Poise with ephemeral, safe error replies
- In-memory per-guild queues; no database required
- URL or YouTube search input, including Thai text
- UTF-8/Thai-safe truncation and markdown escaping in embeds
- One reusable `reqwest::Client`, minimal allocations, small release binary
- JSON structured logs through `RUST_LOG`
- Graceful shutdown on SIGINT/SIGTERM

## Prerequisites

- Rust `1.100.0` with `rustfmt` and `clippy` (pinned in `rust-toolchain.toml`)
- `yt-dlp` available on `PATH`, or an explicit path in `YTDLP_BINARY`
- A Discord bot application with:
  - `bot` and `applications.commands` OAuth2 scopes
  - `Connect` and `Speak` voice permissions
  - Gateway intents: Server Guilds and Guild Voice States (no privileged Message Content intent)
- On Debian/Ubuntu, install audio build dependencies:

  ```sh
  sudo apt-get update
  sudo apt-get install --yes build-essential pkg-config libopus-dev
  ```

## Configuration

```sh
cp .env.example .env
# Edit .env and set DISCORD_TOKEN to your real bot token.
```

| Variable | Required | Description |
| --- | --- | --- |
| `DISCORD_TOKEN` | Yes | Discord bot token. Never commit it. |
| `DISCORD_GUILD_ID` | No | Guild ID for instant slash-command registration during testing. Leave empty to register globally (may take up to an hour). |
| `YOUTUBE_COOKIES_PATH` | No | Path to a Netscape-format `cookies.txt` for YouTube access from hosting providers. Leave empty to disable. |
| `YTDLP_BINARY` | No | `yt-dlp` executable name or path. Defaults to `yt-dlp`. |
| `RUST_LOG` | No | `tracing-subscriber` filter, e.g. `info,wavemelo=debug`. |

Keep `.env` and cookie files outside Git; both patterns are ignored by `.gitignore`.

## Build and run

```sh
cargo build --release
./target/release/wavemelo
```

On Windows, run `target\release\wavemelo.exe`.

## Commands

| Command | Description |
| --- | --- |
| `/ping` | Show gateway and HTTP latency. |
| `/join` | Join your current voice channel. |
| `/leave` | Leave voice and clear the guild queue. |
| `/play <query>` | Play a URL or YouTube search; auto-joins your voice channel and queues if busy. |
| `/skip` | Stop the current track and advance to the next queued track. |
| `/stop` | Stop playback and clear the queue, staying in voice. |
| `/queue` | Show the next queued tracks. |

## Development

Run all quality gates before committing:

```sh
cargo fmt --all
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

## Notes

- There is no Lavalink dependency. Audio is decoded by Symphonia and sourced through `yt-dlp`.
- Songbird 0.6.1 does not expose a usable DAVE/E2EE API, so end-to-end encrypted voice is not supported.
- Live Discord voice playback requires valid credentials, voice permissions, and a working `yt-dlp` installation; automated checks cover compilation and unit tests only.

## License

MIT. See [LICENSE](LICENSE).
