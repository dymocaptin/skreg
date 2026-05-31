# syntax=docker/dockerfile:1

# Stage 1: build web assets
FROM node:20-slim AS web-builder
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Stage 2: build Rust binary
FROM rust:1.91-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY certs/ certs/
RUN cargo build --release -p skreg-api

# Stage 3: runtime
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/skreg-api /usr/local/bin/skreg-api
COPY --from=web-builder /web/dist /web/dist
ENV WEB_DIST_DIR=/web/dist
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/skreg-api"]
