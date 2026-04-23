# syntax=docker/dockerfile:1.7
# Rust build → slim Debian runtime for `sorry-server`.
# Only `sorry-core` and `sorry-server` are needed here; `sorry-wasm` is stubbed
# so the workspace resolves without dragging in wasm-bindgen on the server side.

FROM rust:1-bookworm AS build
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY sorry-core/ sorry-core/
COPY sorry-server/ sorry-server/
# Workspace member stub: the real sorry-wasm crate compiles with wasm-bindgen
# and only targets wasm32 — we don't want its deps in the server image.
COPY sorry-wasm/Cargo.toml sorry-wasm/Cargo.toml
RUN mkdir -p sorry-wasm/src && : > sorry-wasm/src/lib.rs

RUN --mount=type=cache,id=sorry-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=sorry-server-target,target=/app/target,sharing=locked \
    cargo build --release --package sorry-server \
    && cp /app/target/release/sorry-server /usr/local/bin/sorry-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/bin/sorry-server /usr/local/bin/sorry-server

ENV SORRY_PORT=8080 \
    SORRY_CORS_ORIGIN="" \
    RUST_LOG="sorry_server=info,tower_http=info"

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD timeout 3 bash -c 'cat < /dev/null > /dev/tcp/127.0.0.1/${SORRY_PORT:-8080}' || exit 1

CMD ["sorry-server"]
