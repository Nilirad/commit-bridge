FROM nixos/nix:latest AS builder
ENV NIX_CONFIG="experimental-features = nix-command flakes"
WORKDIR /app
COPY . .
RUN nix build . --accept-flake-config --print-build-logs

FROM debian:bookworm-slim AS final
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/result/bin/relay /usr/local/bin/relay
ENV DATABASE_URL="sqlite:///app/data/relay.db?mode=rwc"
WORKDIR /app/data
CMD ["relay"]
