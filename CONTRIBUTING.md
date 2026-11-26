# Contributing to wavemelo

Thanks for helping keep wavemelo small, safe, and easy to run on low-spec servers.

## Development setup

1. Install the Rust toolchain pinned in `rust-toolchain.toml` (rustup will do this automatically).
2. Copy `.env.example` to `.env` and fill in a **test bot** token. Never commit `.env`.
3. Install `yt-dlp` on your `PATH` if you want to test playback.
4. Run `cargo build` to verify the workspace.

## Before opening a pull request

Run these commands and fix every failure:

```sh
cargo fmt --all
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test
```

For the final pedantic audit, also run:

```sh
cargo clippy --all-targets -- -D warnings -W clippy::pedantic
```

## Guidelines

- Keep dependencies and Cargo features minimal; RAM and binary size matter.
- Preserve UTF-8 correctness, including Thai text. Never slice strings at arbitrary byte indexes.
- Do not log, print, or commit Discord tokens, cookie contents, or private guild data.
- Use slash commands for user-facing features and return errors as ephemeral replies.
- Add unit tests for parsing, formatting, and validation logic.
- Keep one logical change per commit and use the exact commit message required by the project plan.

## Reporting bugs

Open an issue with the command used, the expected behavior, the observed behavior, and relevant
`RUST_LOG` output with all identifiers and secrets redacted. For security issues, follow
`SECURITY.md` instead of filing a public issue.
