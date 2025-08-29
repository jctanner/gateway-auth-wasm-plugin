# BYOIDC WASM Plugin - Architecture

## Overview

This document explains the internal architecture and design of the BYOIDC WASM Plugin.

## Core Components

### 1. WASM Plugin Entry Point (`src/lib.rs`)

```rust
#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(AuthProxyRoot::new())
    });
}
```

### 2. HTTP Context Handler

```rust
impl HttpContext for AuthProxy {
    fn on_http_request_headers(&mut self, num_headers: usize, end_of_stream: bool) -> Action {
        // 1. Skip auth for OAuth paths
        // 2. Forward request to auth service
        // 3. Process auth response
        // 4. Allow/deny original request
    }
}
```

## Request Processing Flow

### Phase 1: Request Interception
- [ ] TODO: Document request header processing
- [ ] TODO: Explain path-based auth skipping
- [ ] TODO: Detail cookie extraction and forwarding

### Phase 2: Authentication Check
- [ ] TODO: Document HTTP dispatch to auth service
- [ ] TODO: Explain cluster name resolution
- [ ] TODO: Detail timeout and error handling

### Phase 3: Response Processing
- [ ] TODO: Document auth response handling
- [ ] TODO: Explain redirect vs. allow logic
- [ ] TODO: Detail error response generation

## Configuration Processing

### Deserialization Pipeline
- [ ] TODO: Explain PluginConfig parsing
- [ ] TODO: Document validation logic
- [ ] TODO: Detail error handling

## Internal State Management

### Context Lifecycle
- [ ] TODO: Document RootContext vs HttpContext
- [ ] TODO: Explain state persistence
- [ ] TODO: Detail memory management

## Security Considerations

### Header Handling
- [ ] TODO: Document sensitive header filtering
- [ ] TODO: Explain injection prevention
- [ ] TODO: Detail sanitization logic

## Performance Characteristics

### Memory Usage
- [ ] TODO: Document memory footprint
- [ ] TODO: Explain optimization strategies

### Request Latency
- [ ] TODO: Document auth service call overhead
- [ ] TODO: Explain optimization opportunities

## Integration Points

### Istio Service Mesh
- [ ] TODO: Explain cluster name resolution
- [ ] TODO: Document service discovery integration

### Gateway API
- [ ] TODO: Document HTTPRoute interaction
- [ ] TODO: Explain path-based routing

*This document is a work in progress. Sections marked with TODO will be expanded based on implementation needs.*
