# Build stage
FROM rust:1.75-slim-bookworm as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml .

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/colegio-bot /app/colegio-bot

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

# Create data directory
RUN mkdir -p /app/data

# Run
CMD ["./colegio-bot"]
