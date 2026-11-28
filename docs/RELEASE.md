# Release verification

Recorded 2026-11-28 for `wavemelo` v0.2.0 after the Lavalink v4 migration.

## Toolchain and platform

- `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- Pinned by `rust-toolchain.toml`
- Build/test platform: Windows 11 Enterprise 10.0.26200, AMD Ryzen 5 7600 (12 logical cores), 30.88 GB RAM
- Target artifact: `target/release/wavemelo.exe` (Windows x86_64)

## Migration summary

wavemelo was migrated from a local audio pipeline (Songbird 0.6.1 + Symphonia +
yt-dlp subprocess) to a **Lavalink v4** backend:

- **lavalink-rs 0.15.0** — Lavalink v4 client with built-in queue, voice
  handshake accumulation, and per-guild player contexts.
- **Lavalink 4.2.2** — Java audio node via `ghcr.io/lavalink-devs/lavalink:4.2.2`.
- **youtube-plugin 1.18.2** — Replaces the deprecated built-in YouTube source
  (verified via Maven POM at `maven.lavalink.dev`).
- **LavaSrc 4.8.3** — Optional Spotify, Apple Music, Deezer, Yandex Music,
  VK Music, Tidal, Qobuz, and yt-dlp sources (all off by default; verified
  via Maven POM at `maven.lavalink.dev`).
- **Docker Compose** — Two-service deployment (`lavalink` + `bot`) with a
  TCP health check and an internal bridge network.

### What was removed

- Songbird 0.6.1 and all voice manager code.
- Symphonia / Opus decoding dependencies (no longer in `Cargo.lock`).
- yt-dlp subprocess invocation, metadata cache, prefetch system, and
  singleflight coalescing.
- `src/audio/source.rs`, `src/audio/cache.rs`, `src/audio/prefetch.rs`,
  `src/audio/queue.rs`, `src/audio/metadata.rs`.
- `src/ui/colors.rs` (consolidated into `src/ui/embeds.rs`).
- `src/utils/` directory (consolidated into `src/utils.rs`).

### What was added

- `src/audio/client.rs` — Lavalink client construction with event registration.
- `src/audio/loader.rs` — Track resolution via Lavalink's REST API.
- `src/audio/events.rs` — Lavalink event handlers and Discord voice event
  forwarding with bot-only forced-disconnect cleanup.
- `lavalink/application.yml` — Lavalink v4 configuration with plugin
  declarations and the youtube-source recommended client list.
- `Dockerfile` — Multi-stage build (`rust:1.100-slim-bookworm` →
  `debian:bookworm-slim`), explicit gcc/libc6-dev for `ring`, non-root
  runtime user.
- `docker-compose.yml` — Service orchestration with a TCP health check,
  loopback-only port publish, and `LAVALINK_SERVER_PASSWORD` env override.
- `.dockerignore` — Minimal build context (Cargo manifests + src/).

## Architecture changes

```
Before (v0.1.0):                     After (v0.2.0):
serenity + Songbird                  serenity (no voice feature)
    │                                     │
    ├── yt-dlp subprocess                 │ voice events
    ├── Symphonia decoder                 ▼
    └── Opus encoder               lavalink-rs ──► Lavalink v4
                                              │
                                              ├── youtube-plugin
                                              └── LavaSrc (off by default)
```

- Voice state and server update events are forwarded from serenity's gateway
  to lavalink-rs, which assembles connection info and sends it to Lavalink.
- Lavalink handles all audio: YouTube resolution, decoding, Opus encoding,
  and Discord voice server communication.
- The bot binary went from ~14.55 MiB (with Opus + Symphonia) to
  **9.79 MiB** (no local audio dependencies).
- The process no longer requires `yt-dlp` on PATH or audio build
  dependencies (`libopus-dev`, `pkg-config`).

## Quality gates

All checks passed locally on 2026-11-28:

| Command | Status |
| --- | --- |
| `cargo fmt --all --check` | passed |
| `cargo clippy --all-targets --locked -- -D warnings` | passed |
| `cargo test --locked` | **28 passed**, 0 failed |
| `cargo build --release --locked` | passed (10,264,064 bytes) |

### Test inventory

| Module | Tests | Description |
| --- | --- | --- |
| `config` | 14 | Token, guild ID, Lavalink host/port/password/secure validation, boolean parsing, address formatting, debug redaction |
| `error` | 4 | Display, user messages, detail leakage prevention |
| `audio::query` | 6 | URL/search classification, scheme normalization, Thai text, edge cases |
| `utils` | 4 | Duration formatting, Thai-safe truncation |
| **Total** | **28** | |

The previous 64-test suite included tests in modules removed during
migration (cache, prefetch, queue, state, source).

## Configuration

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `DISCORD_TOKEN` | yes | — | Bot token |
| `DISCORD_GUILD_ID` | no | — | Dev guild for instant command registration |
| `LAVALINK_HOST` | no | `127.0.0.1` | Lavalink hostname (`lavalink` in Compose) |
| `LAVALINK_PORT` | no | `2333` | Lavalink port |
| `LAVALINK_PASSWORD` | yes | `youshallnotpass` | Validated as non-empty; Compose injects via `LAVALINK_SERVER_PASSWORD` |
| `LAVALINK_SECURE` | no | `false` | TLS/WSS |
| `RUST_LOG` | no | `info,wavemelo=debug` | Log filter |

Extra sources (Spotify, Deezer, …) are configured directly in
`lavalink/application.yml` per the LavaSrc README; they are not exposed
through bot environment variables.

## Docker deployment

```sh
cp .env.example .env   # fill in DISCORD_TOKEN and LAVALINK_PASSWORD
docker compose up --build -d
docker compose logs -f bot
```

- `lavalink` service: health-checked via a bash `/dev/tcp` TCP probe on
  port 2333 (the base image has no `curl`/`wget`). Port is published on
  **host loopback only** (`127.0.0.1:2333`) for local `cargo run` use;
  remove for remote-only deployments.
- `bot` service: depends on healthy `lavalink`, runs as a non-root user,
  minimal Debian runtime with CA certificates only.
- Plugin jars download into the container layer on first start and
  re-download if the container is re-created. No named volumes are used
  because the image runs as uid 322 and a fresh named volume would be
  root-owned, breaking plugin downloads.
- The node password is overridden via the documented
  `LAVALINK_SERVER_PASSWORD` environment variable (Spring Boot env
  override that takes precedence over `application.yml`).

## Known limitations

- Spotify, Apple Music, Deezer, and other LavaSrc sources require manual
  configuration in `lavalink/application.yml` and valid credentials; none
  are wired through the bot's environment.
- Live Discord gateway, voice handshake, and actual audio output were not
  exercised locally during this migration (no Docker daemon available in
  the build environment); the first live deployment should be monitored
  for voice connection and track loading issues.
- The health check uses a TCP probe rather than the `/version` HTTP
  endpoint because the base image lacks HTTP client tools.

## v0.1.0 release notes (archived)

<details>
<summary>Previous release verification (2026-11-27)</summary>

- 64 tests passing.
- `target/release/wavemelo.exe`: 15,251,968 bytes (14.55 MiB).
- Local audio pipeline with Songbird 0.6.1, yt-dlp metadata cache, prefetch,
  and singleflight coalescing.
- Cache hit p50 1 μs, p95 1 μs.
- External yt-dlp metadata probe: ~4.2 seconds.
</details>

## Release readiness

The verified release is suitable for deployment with a running Lavalink v4
node. Latency targets that depend on Discord voice handshakes or YouTube
upstream remain unverified and should be validated with live telemetry.
