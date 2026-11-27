# Release verification

Recorded 2026-11-27 for `wavemelo` v0.1.0.

## Toolchain

- `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- Pinned by `rust-toolchain.toml`

## Automated checks

All checks passed locally:

- `cargo fmt --all`
- `cargo check --all-targets`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

Result: 44 unit tests passed, 0 failed.

## Release artifact

- `cargo build --release`: passed
- `target/release/wavemelo.exe`: 15,031,296 bytes

The release profile uses `opt-level = "z"`, `lto = "thin"`, one codegen unit, no incremental artifacts, and stripped symbols to keep builds practical on low-spec VPS hosts.

## Runtime notes

- `yt-dlp` must be installed and reachable via `YTDLP_BINARY`.
- Optional YouTube cookies are passed to `yt-dlp` as `--cookies <path>`; cookie contents are never logged.
- Songbird 0.6.1 includes DAVE/AEAD crates but exposes no usable DAVE/E2EE API, so encrypted voice is not supported.
- Live Discord gateway, voice connection, and playback were not verified locally because no Discord token was configured and `yt-dlp` was unavailable in this environment. These paths require manual verification on a real Discord server.
