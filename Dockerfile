# ==============================================================================
# Multi-Stage Dockerfile for Draft VCS & DraftMultiverse Web Platform
# ==============================================================================

# Stage 1: Build Environment (using modern Rust toolchain)
FROM rust:slim-bookworm AS builder

WORKDIR /usr/src/draft

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifest and source trees
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binaries
RUN cargo build --release -p daft-cli

# Stage 2: Minimal Production Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Draft binaries into system PATH
COPY --from=builder /usr/src/draft/target/release/dft /usr/local/bin/dft
RUN ln -s /usr/local/bin/dft /usr/local/bin/draft && ln -s /usr/local/bin/dft /usr/local/bin/drf

# Mount target repository directory
WORKDIR /repo

# Expose DraftMultiverse HTTP Port
EXPOSE 3333

# Environment defaults
ENV DFT_HOST=0.0.0.0
ENV DFT_PORT=3333

# Default entrypoint: Run DraftMultiverse Web Platform
CMD ["dft", "ui", "--host", "0.0.0.0", "--port", "3333", "--no-browser"]

