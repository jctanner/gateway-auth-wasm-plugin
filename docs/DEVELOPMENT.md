# Development Guide

## Setting Up Development Environment

### Prerequisites

- Rust toolchain (nightly)
- Docker or Podman  
- OpenShift cluster (CRC recommended)
- Git

### Initial Setup

```bash
# Clone repository
git clone <repository-url>
cd gateway-auth-wasm-plugin

# Setup Rust environment
rustup install nightly
rustup default nightly
rustup target add wasm32-unknown-unknown

# Build development environment
make setup-dev  # TODO: Create this target
```

## Development Workflow

### Local Development Loop

```bash
# 1. Make code changes in src/
# 2. Build WASM binary
make build-wasm-cargo

# 3. Build and push container image
make image && make push

# 4. Deploy to test cluster
make deploy

# 5. Test changes
cd tests/integration/
python test-auth-flow.py --username developer --password developer
```

### Code Structure

```
src/
├── lib.rs          # Main plugin entry point
├── config.rs       # Configuration parsing and validation
├── http_client.rs  # HTTP dispatch utilities
├── headers.rs      # Header processing and forwarding
├── responses.rs    # Response handling and error mapping
└── metrics.rs      # Observability and metrics
```

## Testing Strategy

### Unit Tests

```bash
# Run Rust unit tests
cargo test

# TODO: Add more unit test coverage
```

### Integration Tests

```bash
# Run browser-based authentication flow tests
cd tests/integration/
source ../../venv/bin/activate
python test-auth-flow.py --browser chrome --no-headless
```

### Manual Testing

```bash
# Test individual components
make test-auth-service   # TODO: Create this target  
make test-oauth-flow     # TODO: Create this target
```

## Code Guidelines

### Rust Style

- Follow standard Rust formatting (`cargo fmt`)
- Use `clippy` for linting (`cargo clippy`)
- Document public functions and structs
- Handle errors explicitly

### WASM-Specific Considerations

```rust
// Use appropriate log levels
log::info!("Important runtime information");
log::debug!("Detailed debugging information");

// Handle proxy-wasm return values
match self.dispatch_http_call(/* ... */) {
    Ok(_) => Action::Pause,
    Err(e) => {
        log::error!("Dispatch failed: {:?}", e);
        Action::Continue
    }
}
```

## Debugging

### Local Development

```bash
# Enable debug logging
export RUST_LOG=debug

# Build with debug symbols
cargo build --target wasm32-unknown-unknown
```

### In-Cluster Debugging

```bash
# Check plugin loading
oc logs -n openshift-ingress deployment/router-default | grep -i wasm

# Monitor authentication flow
oc logs -f -n openshift-ingress deployment/router-default
```

### Debug Configuration

```yaml
# Add to WasmPlugin for verbose logging
pluginConfig:
  debug:
    enabled: true
    log_level: "debug"
    log_headers: true
  # ... rest of config
```

## Contributing

### Pull Request Process

1. Fork repository
2. Create feature branch
3. Make changes with tests
4. Update documentation
5. Submit pull request

### Code Review Checklist

- [ ] Code follows Rust style guidelines
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Documentation updated
- [ ] No security vulnerabilities introduced
- [ ] Performance implications considered

## Build System

### Makefile Targets

```bash
make build-wasm-cargo    # Build WASM binary
make image               # Build OCI container image
make push                # Push to registry
make deploy              # Deploy to cluster
make test                # Run all tests
make clean               # Clean build artifacts
```

### Container Build

```dockerfile
# Multi-stage build for efficiency
FROM rustlang/rust:nightly AS builder
# ... build WASM binary

FROM scratch AS wasm
# ... copy WASM binary only
```

## Release Process

### Version Management

- Update version in `Cargo.toml`
- Tag release: `git tag v1.0.0`
- Update changelog

### Container Registry

```bash
# Build and tag for release
make image TAG=v1.0.0
make push TAG=v1.0.0
```

## Performance Optimization

### Memory Usage

- Minimize allocations in hot paths
- Reuse buffers when possible
- Monitor memory usage patterns

### Request Latency

- Optimize HTTP dispatch calls
- Cache configuration parsing
- Minimize header processing overhead

## Security Considerations

### Input Validation

- Validate all configuration parameters
- Sanitize headers before forwarding
- Prevent header injection attacks

### Secrets Management

- Never log sensitive headers
- Use secure configuration parsing
- Implement proper error handling

*This document will be expanded as development practices evolve.*
