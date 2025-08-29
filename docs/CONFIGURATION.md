# WASM Plugin Configuration Guide

## Overview

This document provides comprehensive details on configuring the BYOIDC WASM Plugin through the `WasmPlugin` resource `pluginConfig` section.

## Basic Configuration Structure

```yaml
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: gateway-auth-wasm-plugin
  namespace: openshift-ingress
spec:
  selector:
    matchLabels:
      gateway.networking.k8s.io/gateway-name: odh-gateway
  phase: AUTHN
  priority: 1000
  url: oci://registry.tannerjc.net/gateway-auth-wasm-plugin:latest
  pluginConfig:
    # Configuration goes here
```

## Configuration Sections

### `auth_service` - Authentication Service Configuration

**Purpose**: Configures communication with your authentication service (e.g., kube-auth-proxy).

```yaml
auth_service:
  endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
  cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
  verify_path: "/oauth2/auth"
  timeout: 5000
  tls:
    verify_cert: false
```

#### Parameters

| Parameter | Type | Required | Description | Example |
|-----------|------|----------|-------------|---------|
| `endpoint` | string | ✅ | Full URL to authentication service | `"http://auth.namespace.svc.cluster.local:8080"` |
| `cluster` | string | ✅ | Istio cluster name for service mesh routing | `"outbound|8080||auth.namespace.svc.cluster.local"` |
| `verify_path` | string | ✅ | Authentication check endpoint path | `"/oauth2/auth"`, `"/auth"` |
| `timeout` | integer | ❌ | Request timeout in milliseconds (default: 5000) | `10000` |
| `tls.verify_cert` | boolean | ❌ | Enable TLS certificate verification (default: true) | `false` |

#### Critical Notes

**🔧 Istio Cluster Naming**: The `cluster` parameter MUST use Istio's service mesh naming convention:
```
outbound|<port>||<service>.<namespace>.svc.cluster.local
```

**🍪 Authentication Endpoints**: Use ext_authz compatible endpoints:
- `/oauth2/auth` - Standard ext_authz auth check endpoint
- `/auth` - Alternative auth check endpoint  

**🔒 TLS Configuration**: For HTTP-only communication (recommended for internal services):
```yaml
tls:
  verify_cert: false
```

### `global_auth` - Global Authentication Settings

**Purpose**: Controls when and how authentication is applied.

```yaml
global_auth:
  enabled: true
  skip_paths:
    - "/health"
    - "/metrics" 
  require_auth_header: false
```

#### Parameters

| Parameter | Type | Required | Description | Default |
|-----------|------|----------|-------------|---------|
| `enabled` | boolean | ❌ | Enable global authentication for all requests | `true` |
| `skip_paths` | array | ❌ | Paths that bypass authentication | `[]` |
| `require_auth_header` | boolean | ❌ | Require Authorization header to be present | `false` |

### `error_responses` - Custom Error Handling

**Purpose**: Customize error responses returned to clients.

```yaml
error_responses:
  auth_service_error:
    status: 503
    headers:
      - ["content-type", "application/json"]
    body: '{"error": "authentication_service_unavailable"}'
  access_denied:
    status: 403
    headers:
      - ["content-type", "application/json"]  
    body: '{"error": "access_denied", "message": "Authentication required"}'
  timeout:
    status: 504
    body: '{"error": "authentication_timeout"}'
```

#### Response Types

| Response Type | When Triggered | Default Status |
|---------------|----------------|----------------|
| `auth_service_error` | Authentication service unavailable | 503 |
| `access_denied` | Authentication failed | 403 |
| `timeout` | Authentication service timeout | 504 |
| `invalid_config` | Plugin configuration error | 500 |

## Advanced Configuration Examples

### Production Configuration

```yaml
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"
    cluster: "outbound|4180||kube-auth-proxy.auth-system.svc.cluster.local"
    verify_path: "/oauth2/auth"
    timeout: 10000
    tls:
      verify_cert: true
      ca_cert: "/etc/ssl/certs/ca-bundle.crt"
  
  global_auth:
    enabled: true
    skip_paths:
      - "/health"
      - "/readiness" 
      - "/metrics"
      - "/favicon.ico"
    require_auth_header: false
  
  error_responses:
    auth_service_error:
      status: 503
      headers:
        - ["content-type", "application/json"]
        - ["retry-after", "60"]
      body: '{"error": "authentication_service_unavailable", "retry_after": 60}'
    
    access_denied:
      status: 401
      headers:
        - ["content-type", "text/html"]
      body: |
        <!DOCTYPE html>
        <html>
        <head><title>Authentication Required</title></head>
        <body><h1>Please log in to access this resource</h1></body>
        </html>
```

### Development Configuration

```yaml
pluginConfig:
  auth_service:
    endpoint: "http://auth-service.default.svc.cluster.local:8080"
    cluster: "outbound|8080||auth-service.default.svc.cluster.local"
    verify_path: "/auth"
    timeout: 30000  # Longer timeout for debugging
    tls:
      verify_cert: false
  
  global_auth:
    enabled: true
    skip_paths:
      - "/debug"
      - "/health"
    
  # Enable debug logging
  debug:
    enabled: true
    log_level: "debug"
    log_headers: true
    log_body: false
```

## Configuration Validation

### Required Fields Validation

The plugin validates these required fields at startup:

```rust
// src/config.rs validation
impl PluginConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.auth_service.endpoint.is_empty() {
            return Err("auth_service.endpoint is required".to_string());
        }
        
        if self.auth_service.cluster.is_empty() {
            return Err("auth_service.cluster is required".to_string());
        }
        
        if self.auth_service.verify_path.is_empty() {
            return Err("auth_service.verify_path is required".to_string());
        }
        
        Ok(())
    }
}
```

### Common Configuration Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `BadArgument` during dispatch | Wrong cluster name | Use Istio service mesh format |
| `timeout` errors | Network issues or wrong endpoint | Verify service accessibility |
| `invalid configuration` | Missing required fields | Check required parameters |
| Plugin load failure | Wrong WASM binary format | Verify Rust/proxy-wasm versions |

## Header Forwarding Behavior

### Automatic Header Forwarding

The WASM plugin automatically forwards these headers to the authentication service:

```rust
// Headers forwarded to auth service
if let Some(ref cookie_value) = cookie_header {
    auth_headers.push(("cookie", cookie_value));  // CRITICAL for session-based auth
}

if let Some(ref auth_value) = auth_header {
    auth_headers.push(("authorization", auth_value));
}

// Context headers for auth service
auth_headers.push(("x-forwarded-method", &original_method));
auth_headers.push(("x-forwarded-uri", &original_path)); 
auth_headers.push(("x-forwarded-host", &original_authority));
```

### Cookie Forwarding (Critical)

**🍪 Session-based authentication REQUIRES cookie forwarding**:

```yaml
# This is handled automatically by the plugin
# No configuration needed - cookies are always forwarded
```

## Environment-Specific Configurations

### OpenShift 4.19 + CRC

```yaml
# Optimized for single-node CRC deployment
pluginConfig:
  auth_service:
    endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
    verify_path: "/oauth2/auth"
    timeout: 5000
    tls:
      verify_cert: false  # HTTP-only for simplicity
  
  global_auth:
    enabled: true
```

### Production OpenShift with TLS

```yaml
# Production deployment with HTTPS
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"
    cluster: "outbound|4180||kube-auth-proxy.auth-system.svc.cluster.local"
    verify_path: "/oauth2/auth" 
    timeout: 10000
    tls:
      verify_cert: true
      
  global_auth:
    enabled: true
    skip_paths:
      - "/health"
      - "/readiness"
```

## Testing Configuration

### Validation Commands

```bash
# 1. Verify plugin loads successfully
oc get wasmplugin -n openshift-ingress -o yaml

# 2. Check plugin configuration parsing
oc logs -n openshift-ingress deployment/router-default | grep -i config

# 3. Test auth service connectivity  
oc exec -n openshift-ingress deployment/router-default -- curl -v http://kube-auth-proxy:4180/oauth2/auth

# 4. Validate authentication flow
curl -k -v https://your-gateway.example.com/
```

### Configuration Debugging

Enable debug logging to troubleshoot configuration issues:

```yaml
pluginConfig:
  debug:
    enabled: true
    log_level: "debug"
    log_headers: true
    
  # ... rest of configuration
```

## Next Steps

- **Architecture**: See [ARCHITECTURE.md](ARCHITECTURE.md) for how configuration is processed
- **Troubleshooting**: See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for configuration debugging
- **Integration**: See [INTEGRATION.md](INTEGRATION.md) for auth service-specific configurations
