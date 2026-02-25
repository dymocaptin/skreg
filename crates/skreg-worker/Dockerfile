# syntax=docker/dockerfile:1
FROM rust:1.91-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p skreg-worker

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/skreg-worker /usr/local/bin/skreg-worker
ENTRYPOINT ["/usr/local/bin/skreg-worker"]
