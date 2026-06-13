# syntax=docker/dockerfile:1

# ---- Build stage ---------------------------------------------------------
# Static musl build so the final image can be FROM scratch (no libc needed).
FROM rust:slim AS builder

# musl toolchain for a fully static binary
RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Cache deps: copy manifests, build a stub, then the real sources.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl \
    && rm -rf src

COPY src ./src
# Touch so cargo rebuilds the real main, not the cached stub.
RUN touch src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl

# ---- Runtime stage -------------------------------------------------------
# scratch = empty image. Only the static binary ships -> ~10-15 MB.
FROM scratch AS runtime

COPY --from=builder \
    /app/target/x86_64-unknown-linux-musl/release/metrics-service /metrics-service

# Defaults; override at run time (see docker-compose.yml).
ENV PORT=3000 \
    RUST_LOG=metrics_service=info

EXPOSE 3000

# Binary self-probes /health (no shell/curl in a scratch image).
HEALTHCHECK --interval=30s --timeout=3s --start-period=2s --retries=3 \
    CMD ["/metrics-service", "healthcheck"]

# Binary handles SIGTERM for graceful shutdown (see main.rs).
ENTRYPOINT ["/metrics-service"]
