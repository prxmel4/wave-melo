# Changelog

All notable changes to wavemelo are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow Semantic Versioning. The authoritative version is declared
in `Cargo.toml`.

## [Unreleased]

## [0.7.1] - 2026-11-28

### Fixed

- Refreshed `Cargo.lock` so the v0.7.0 dependency manifest matches the declared
  package version and all `--locked` gates succeed from a clean checkout.

## [0.7.0] - 2026-11-28

### Added

- `/queueremove`, `/queuejump`, `/queuemove`, and `/queueclear` slash commands
  with strict 1-based index validation performed before any queue mutation.
- Pure queue helpers (`zero_based`, `move_indices`, `reordered`, `jump_plan`)
  with deterministic tests for boundaries, empty queues, and reorder safety.
- Same-voice-channel authorization for every queue management command.

### Changed

- `/queueclear` also clears the saved queue-loop snapshot so repeat cycles
  cannot resurrect removed tracks.

## [0.6.0] - 2026-11-28

### Added

- Per-guild session registry shared with Lavalink event handlers, with atomic
  loop-mode state and bounded queue snapshots that never hold locks across
  network operations.
- `/loop <off|track|queue>` with buffered repeat copies so track and queue
  looping survive Lavalink's automatic advance timing.
- Internal loop markers are stripped from the real queue when loop mode
  changes or is disabled, keeping user tracks authoritative.
- Deterministic tests for loop transitions, bounded snapshots, marker handling,
  and registry lifecycle.

### Changed

- `/nowplaying` now reports the guild's stored loop mode and active filter.
- Lavalink user data now carries the guild state registry for safe event-side
  coordination.

## [0.5.0] - 2026-11-28

### Added

- `/seek <position>` supporting `90`, `01:30`, and `00:01:30` formats with
  strict validation, checked conversion, live/non-seekable rejection, and
  out-of-bounds protection.
- `/nowplaying` with track title, URL, artist, requester, position, duration,
  volume, loop mode, active filter, status, and a responsive progress bar.
- Pure seek parsing/validation and progress-bar modules with deterministic
  tests for malformed input, overflow, and live streams.

## [0.4.0] - 2026-11-28

### Added

- `/volume <0..150>`, `/pause`, and `/resume` slash commands, backed by a
  shared playback operations module so future interactive controls reuse the
  same behavior.
- Same-voice-channel authorization and active-player checks for all new
  playback controls.
- Structured INFO telemetry for volume and pause-state transitions.
- Deterministic tests for volume boundaries and strict text parsing.

### Changed

- Repeated `/pause` and `/resume` requests are idempotent and report their
  current state instead of issuing redundant Lavalink updates.

## [0.3.0] - 2026-11-28

### Added

- A deterministic regression test that scans `src/**/*.rs` and rejects Thai
  Unicode characters (U+0E00–U+0E7F), enforcing the English-only source policy.
- Tests for Unicode-safe truncation boundaries, including small limits.

### Changed

- All bot-controlled command responses, embeds, errors, and help text are now
  professional English.
- External track metadata, user queries, and search results remain unmodified.
- `README.md` now documents the English-only reply policy.
- `truncate()` no longer returns a string longer than the requested limit.

### Removed

- All Thai-language literals from `src/` and their obsolete test expectations.

## [0.2.0] - 2026-11-28

### Added

- Lavalink v4 playback backend via `lavalink-rs` 0.15.0 and Lavalink 4.2.2.
- `youtube-plugin` 1.18.2 and optional LavaSrc 4.8.3 sources (off by default).
- Docker Compose deployment with health checks and an internal `yt-cipher`
  sidecar for zero-OAuth YouTube deciphering.
- Slash commands: `/ping`, `/play`, `/skip`, `/stop`, `/queue`, `/join`,
  `/leave`.

### Changed

- Replaced the local Songbird/Symphonia/yt-dlp pipeline with Lavalink.
- Per-guild operation locks serialize playback transitions.
- Secrets are redacted in `Debug` output and excluded from user-facing errors.

### Removed

- Local audio decoding, yt-dlp subprocess, metadata cache, prefetch, and
  singleflight modules.

## [0.1.0] - 2026-11-27

- Initial release with a local audio pipeline, 64 tests, and a 14.55 MiB
  Windows release binary. Historical details are recorded in `docs/RELEASE.md`.
