# Build stage
FROM rust:latest AS builder
LABEL maintainer="Nick Brandaleone <nbrand@mac.com>"

WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to compile and cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src/ target/release/deps/cr_pricing*

# Copy the actual source code
COPY src/ src/

# Build the real binary
RUN cargo install --path .

# Production stage
FROM debian:trixie-slim AS prod

# Install any basic runtime dependencies if needed (none are required for pure Rust, but standard)
RUN apt-get update && apt-get install \
    --no-install-recommends \
    -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/cr-pricing /usr/local/bin/cr-pricing

# Set entrypoint to run the pricing application
ENTRYPOINT ["/usr/local/bin/cr-pricing"]
