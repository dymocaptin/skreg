# syntax=docker/dockerfile:1
FROM rust:1.91-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY certs/ certs/
RUN cargo build --release -p skreg-worker

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        shellcheck \
        python3-pip \
    && pip3 install --no-cache-dir bandit semgrep \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/skreg-worker /usr/local/bin/skreg-worker
COPY crates/skreg-worker/rules /opt/skreg/rules
ENV SKREG_YARA_RULES_DIR=/opt/skreg/rules
ENTRYPOINT ["/usr/local/bin/skreg-worker"]
