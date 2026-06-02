FROM rust:1.96-slim-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev
WORKDIR /app
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim AS final
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/relay /usr/local/bin/relay
ENV RELAY__DATABASE__URL="sqlite:///app/data/relay.db?mode=rwc"
WORKDIR /app/data
CMD ["relay"]
