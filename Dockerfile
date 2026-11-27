# ===================================================================
# Multi-Stage Production Dockerfile for Economy Aggregator Engine
# Coolify GitHub App Ready (Self-Contained, Persistent /app/data)
# ===================================================================

# Stage 1: Build Svelte Web Terminal UI with Bun
FROM oven/bun:1-slim AS web-builder
WORKDIR /web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
RUN bun run build

# Stage 2: Build Zero-GC Rust Engine (supports Cargo.lock v4 & Git deps)
FROM rust:1-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release || true

# Build actual application binary
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# Stage 3: Minimal Production Runtime Container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    tini \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/data/lake /app/data/hot_buffer /app/data/config /app/web/dist

WORKDIR /app
COPY --from=builder /app/target/release/economy-aggregator /usr/local/bin/economy-aggregator
COPY --from=web-builder /web/dist /app/web/dist

ENV PORT=8080
ENV DATA_DIR=/app/data
ENV WEB_DIST_DIR=/app/web/dist
ENV RUST_LOG=info

# Persistent Columnar Lakehouse & Subscription Storage
VOLUME ["/app/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://127.0.0.1:${PORT}/health || exit 1

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/economy-aggregator"]
CMD ["serve"]
