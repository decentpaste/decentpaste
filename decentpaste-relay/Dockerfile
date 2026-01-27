# Build stage
FROM rust:1.82-slim-bookworm as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy source to cache dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/decentpaste-relay /usr/local/bin/

# libp2p port
EXPOSE 4001
# Health check port
EXPOSE 8080

ENTRYPOINT ["decentpaste-relay"]
CMD ["--port", "4001", "--health-port", "8080"]
