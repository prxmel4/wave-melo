# syntax=docker/dockerfile:1

# ---- Build stage ---------------------------------------------------------
# rust:1.100-slim-bookworm matches rust-toolchain.toml (verified on Docker Hub).
FROM rust:1.100-slim-bookworm AS builder
WORKDIR /build

# ring (rustls backend) compiles C code; install the C toolchain explicitly
# rather than relying on the slim base image's contents.
RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc libc6-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies with a dummy workspace using the locked manifests.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src target/release/deps/wavemelo* \
       target/release/.fingerprint/wavemelo* target/release/build/wavemelo*

# Build the real binary.
COPY src ./src
RUN cargo build --release --locked

# ---- Runtime stage -------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system wavemelo \
    && useradd --system --gid wavemelo --home-dir /app --shell /usr/sbin/nologin wavemelo

WORKDIR /app
COPY --from=builder /build/target/release/wavemelo /usr/local/bin/wavemelo

USER wavemelo
ENV RUST_LOG="info,wavemelo=debug"

CMD ["wavemelo"]
