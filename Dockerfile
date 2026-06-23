# Build stage
FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p aspectus-server
# Install sqlx-cli for runtime migrations
RUN cargo install sqlx-cli --no-default-features --features postgres,rustls --root /tmp/sqlx

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/aspectus-server /usr/local/bin/aspectus-server
COPY --from=builder /tmp/sqlx/bin/sqlx /usr/local/bin/sqlx
COPY migrations /app/migrations
WORKDIR /app
EXPOSE 3100
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -sf http://localhost:3100/health || exit 1
# Run pending migrations before starting the server.
# Requires DATABASE_URL to be set in the container environment.
CMD ["/bin/sh", "-c", "sqlx migrate run --source /app/migrations && exec aspectus-server"]
