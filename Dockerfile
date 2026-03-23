FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY migrations ./migrations
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/gold-notify /usr/local/bin/gold-notify

ENV RUST_LOG=gold_notify=info,sqlx=warn

ENTRYPOINT ["/usr/local/bin/gold-notify"]
