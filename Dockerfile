FROM rust:1.96-slim-bookworm AS chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
ENV SQLX_OFFLINE=true
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin commit-bridge

FROM debian:bookworm-slim AS final
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/commit-bridge /usr/local/bin/commit-bridge
ENV CBRIDGE__DATABASE__URL="sqlite:///app/data/commit-bridge.db?mode=rwc"
VOLUME ["/app/data"]
EXPOSE 3000
WORKDIR /app/data
CMD ["commit-bridge"]
