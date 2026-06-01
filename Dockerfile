# Build stage
FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p aspectus-server

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/aspectus-server /usr/local/bin/aspectus-server
COPY migrations /migrations
EXPOSE 3100
CMD ["aspectus-server"]
