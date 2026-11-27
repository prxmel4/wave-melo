# Release verification

Recorded 2026-11-27 for `wavemelo` v0.1.0 after the latency optimization sprint.

## Toolchain and platform

- `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- `cargo 1.100.0 (3acc79c0c 2026-11-05)`
- Pinned by `rust-toolchain.toml`
- Build/test platform: Windows 11 Enterprise 10.0.26200, AMD Ryzen 5 7600 (12 logical cores), 30.88 GB RAM
- Target artifact: `target/release/wavemelo.exe` (Windows x86_64)
- CMake was detected at `C:\Program Files\CMake\bin\cmake.exe` and added to PATH only for the command invocations.

## Optimization summary

- `/play` defers the interaction before any voice, cache, or extractor work.
- Voice connection and metadata resolution now run concurrently with `tokio::join!` when a join is required, with explicit cleanup if resolution fails after a successful join.
- An existing voice connection is reused only when it is already in the author's channel; otherwise Songbird moves the call.
- A bounded process-local metadata cache skips redundant `yt-dlp` metadata probes.
- Concurrent requests for the same key are coalesced (singleflight), including a short-lived failure coalescing window.
- Next-track prefetching is limited to one task per guild and two concurrent extractions globally; foreground extraction is limited to four concurrent permits.
- Queue mutations, manual skips, stops, disconnects, and guild removals cancel or invalidate obsolete prefetch work.
- Playback generations prevent stale `End`/`Error` events from advancing the queue twice.
- Structured `tracing` summaries now record command, guild, cache hit, prefetch hit, voice reuse, join/resolve/reply/playback timing, total wall-clock time, and outcome. Detailed extractor invocations are logged at `DEBUG`.

## Architecture changes

- `src/audio/cache.rs`: 128-entry LRU metadata cache with 20-minute TTL, `std::time::Instant` freshness, singleflight resolution, and short-lived failure coalescing.
- `src/audio/prefetch.rs`: bounded speculative resolution of only the next queued track, with version-based invalidation and cancellation.
- `src/audio/source.rs`: cache-aware metadata resolution, lazy input construction, bounded metadata strings, and safe subprocess argument assembly.
- `src/state.rs`: per-guild operation locks, playback generations, queue versions, extraction semaphores, prefetch task registry, and guild state cleanup.
- `src/audio/queue.rs`: versioned queue mutations and prefetch invalidation.
- `src/audio/events.rs`: generation-guarded transitions with telemetry and prefetch reuse.
- `src/commands/play.rs`, `src/commands/playback.rs`, `src/commands/voice.rs`: concurrent resolution, safe skip/stop/leave transitions, and command telemetry.

### Cache and concurrency limits

| Limit | Value |
| --- | --- |
| Metadata cache capacity | 128 entries |
| Metadata TTL | 20 minutes |
| Foreground extractions | 4 concurrent |
| Prefetch extractions | 2 concurrent globally |
| Prefetch tasks | 1 per guild |
| Cached payloads | Metadata only; no audio buffers or media files |

## Quality gates

All checks passed locally:

| Command | Status |
| --- | --- |
| `cargo fmt --all` | passed |
| `cargo fmt --all -- --check` | passed |
| `cargo check --all-targets` | passed |
| `cargo clippy --all-targets -- -D warnings` | passed |
| `cargo test` | passed |
| `cargo build --release` | passed |

Passing test count: **64**, 0 failed.

## Release artifact

- `cargo build --release`: passed
- `target/release/wavemelo.exe`: **15,251,968 bytes** (**14.55 MiB**)
- Previous verified size: 15,122,944 bytes (14.43 MiB); increase of 129,024 bytes.
- Release profile: `opt-level = "z"`, `lto = "thin"`, one codegen unit, no incremental artifacts, stripped symbols.

## Performance results

### Measured

| Metric | Sample count | Result | Method |
| --- | --- | --- | --- |
| Metadata cache hit lookup | 1,000 | p50 1,000 ns; p95 1,000 ns | `benchmark_cache_hit_latency` in-process test on Windows, debug profile |
| Single `yt-dlp` metadata probe | 1 before, 3 after | Before: 4,619 ms. After: 4,242 / 4,223 / 4,368 ms (min 4,223, median 4,242, max 4,368) | `yt-dlp --no-playlist --no-warnings -f bestaudio[acodec=opus]/bestaudio/best -j ytsearch1:never gonna give you up`, output discarded |
| Extractor invocations for a fresh metadata probe | Code-path audit | 1 metadata probe plus 1 fresh stream resolution at playback | Songbird 0.6.1 source inspection; previously 2 metadata/stream probes plus the same playback resolution path |

p95 and p99 for the external `yt-dlp` samples are not reported because three samples are insufficient for meaningful tail estimates.

### Not measured

The following targets were not verified end-to-end against Discord in this sprint:

- Immediate interaction acknowledgment in a live Discord client.
- Warm `/play` command processing under 500 ms.
- `/skip` under 200 ms.
- Prefetched transition under 300 ms.
- Natural queue transition gap.
- Multiple active guilds under bounded extractor concurrency.
- Peak/steady-state memory and CPU utilization.
- Voice connection failures and real audio audibility.

No performance improvement percentages are claimed because equivalent end-to-end samples were not collected.

## Benchmark methodology

- Cache benchmark: one pre-populated entry, 1,000 sequential `get_fresh` calls, nanosecond `Instant` sampling, sorted samples with p50/p95 reporting.
- External extractor benchmark: identical `yt-dlp` command before and after, stdout/stderr discarded, wall-clock milliseconds recorded.
- Environment: local Windows host described above. This is not representative of a 1-vCPU, 512 MB–1 GB VPS.
- Reproduce with:

  ```sh
  cargo test audio::cache::tests::benchmark_cache_hit_latency -- --nocapture
  ```

## New and updated tests

- Cache hit/miss, TTL expiry, strict capacity, deterministic LRU eviction, replacement, Thai/Unicode keys, concurrent singleflight, transient failure coalescing, and nanosecond cache-hit benchmark.
- Prefetch hit consumption, queue mutation invalidation, expired prefetch rejection, and task cancellation cleanup.
- Guild generation invalidation, per-guild lock serialization, extraction permit bounds, and guild state cleanup.
- Existing query, config, error, UI, text, and duration tests remain green.

## Known limitations

- Songbird 0.6.1 prepends `user_args` before its own `-f ba[abr>0][vcodec=none]/best --no-playlist` arguments. The requested `bestaudio[acodec=opus]/bestaudio/best` flag is passed, but Songbird's later format flag wins. Changing this safely requires replacing `YoutubeDl` with a pre-resolved `HttpRequest` source and replicating headers/cookies, which risks stream freshness and authentication correctness.
- Songbird's lazy `YoutubeDl` source does not cache resolved stream URLs; playback still performs one fresh extractor invocation. The cache stores only stable metadata, so signed or ephemeral media URLs are never treated as valid for the 20-minute metadata TTL.
- Playlist support remains intentionally disabled, matching the pre-existing Songbird behavior.
- Live Discord gateway, voice handshake, actual audio output, and command latency were not exercised locally during this sprint.
- DAVE/E2EE encrypted voice remains unsupported by Songbird 0.6.1.

## Release readiness

The verified release is suitable for deployment provided external `yt-dlp` behavior is acceptable on the target host. Latency targets that depend on Discord voice handshakes or YouTube upstream remain unverified and should be validated with live telemetry after deployment.
