# Multi-stage Dockerfile for BYOIDC WASM Plugin
# Builds an OCI-compliant image for Istio WasmPlugin deployment

#
# Build Stage - Rust compilation
#
FROM rustlang/rust:nightly as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add WASM target
RUN rustup target add wasm32-unknown-unknown

# Set working directory
WORKDIR /build

# Copy dependency manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo 'fn main() { println!("Dummy"); }' > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --target wasm32-unknown-unknown --release
RUN rm -rf src

# Copy actual source code
COPY src/ ./src/

# Build the actual WASM plugin
RUN cargo build --target wasm32-unknown-unknown --release

# Verify the WASM binary was created
RUN ls -la target/wasm32-unknown-unknown/release/ && \
    test -f target/wasm32-unknown-unknown/release/byoidc_wasm_plugin.wasm

#
# Runtime Stage - Minimal image with just the WASM binary
#
FROM scratch as runtime

# Red Hat Service Mesh compatibility label
LABEL module.wasm.image/variant=compat

# Copy the WASM binary to the root of the image
COPY --from=builder /build/target/wasm32-unknown-unknown/release/byoidc_wasm_plugin.wasm /plugin.wasm

# Istio/Envoy will load the WASM plugin from /plugin.wasm
# This follows the standard convention for WASM plugin OCI images

#
# Development Stage - Full toolchain for development
#
FROM rust:1.75 as development

# Install development tools
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Add WASM target
RUN rustup target add wasm32-unknown-unknown

# Install additional development tools
RUN cargo install cargo-watch wasm-pack

# Set working directory
WORKDIR /workspace

# Development stage is used for interactive development
# Mount source code as volume: -v $(pwd):/workspace

# Default development command
CMD ["bash"]

#
# Testing Stage - Runtime image with debugging tools
#
FROM alpine:latest as testing

# Install debugging tools
RUN apk add --no-cache \
    curl \
    file \
    hexdump

# Copy WASM binary
COPY --from=builder /build/target/wasm32-unknown-unknown/release/byoidc_wasm_plugin.wasm /plugin.wasm

# Verify WASM binary properties
RUN file /plugin.wasm && \
    ls -la /plugin.wasm && \
    echo "WASM binary size: $(wc -c < /plugin.wasm) bytes"

# Command to inspect the WASM binary
CMD ["sh", "-c", "echo 'BYOIDC WASM Plugin Binary:' && file /plugin.wasm && ls -la /plugin.wasm"]
