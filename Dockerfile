# Build stage
FROM rust:latest AS builder

# Install build dependencies for aws-lc-sys (rustls backend)
RUN apt-get update && apt-get install -y \
    cmake \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy files to satisfy Cargo.toml references
RUN mkdir -p src tests && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > tests/cucumber.rs && \
    echo "fn main() {}" > tests/integration_cucumber.rs

# Build dependencies only
RUN cargo build --release && rm -rf src tests

# Copy actual source code
COPY src ./src

# Create dummy test files (not needed for binary, but Cargo checks them)
RUN mkdir -p tests && \
    echo "fn main() {}" > tests/cucumber.rs && \
    echo "fn main() {}" > tests/integration_cucumber.rs

# Touch main.rs to trigger rebuild
RUN touch src/main.rs

# Build the actual binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies (CA certificates for HTTPS)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false vixy

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/vixy /app/vixy

# Copy example config
COPY config.example.toml /app/config.example.toml

# Set ownership
RUN chown -R vixy:vixy /app

USER vixy

# Default port
EXPOSE 8080

# Default command
ENTRYPOINT ["/app/vixy"]
CMD ["--config", "/app/config.toml", "--listen", "0.0.0.0:8080"]
