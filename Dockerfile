# ===================================================================
# Multi-Stage Production Dockerfile for Economy Aggregator Engine
# ===================================================================

FROM rust:1.80-slim-bullseye AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release || true

COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# -------------------------------------------------------------------
# Final Runtime Container
# -------------------------------------------------------------------
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/economy-aggregator /usr/local/bin/economy-aggregator
COPY web/dist /app/web/dist

ENV PORT=8080
ENV RUST_LOG=info

# Persistent Columnar Lakehouse Storage
VOLUME ["/app/data"]
EXPOSE 8080

ENTRYPOINT ["economy-aggregator"]
CMD ["serve"]
