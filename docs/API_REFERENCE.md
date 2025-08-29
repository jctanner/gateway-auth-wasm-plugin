# API Reference

## WasmPlugin Configuration Schema

Complete reference for all configuration parameters supported by the BYOIDC WASM Plugin.

### Root Configuration

```yaml
pluginConfig:
  auth_service:      # AuthServiceConfig - Required
  global_auth:       # GlobalAuthConfig - Optional  
  error_responses:   # ErrorResponsesConfig - Optional
  debug:             # DebugConfig - Optional
```

## `auth_service` - Authentication Service Configuration

### Schema

```yaml
auth_service:
  endpoint: string           # Required
  cluster: string            # Required  
  verify_path: string        # Required
  timeout: integer           # Optional (default: 5000)
  tls:                       # Optional
    verify_cert: boolean     # Optional (default: true)
    ca_cert: string          # Optional
    client_cert: string      # Optional
    client_key: string       # Optional
```

### Parameters

#### `endpoint` (Required)
**Type**: `string`  
**Description**: Full URL to the authentication service  
**Format**: `http://host:port` or `https://host:port`

**Examples**:
```yaml
endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
endpoint: "https://auth-service.auth-system.svc.cluster.local:8443"
```

**Validation**:
- Must be valid URL format
- Must include protocol (http/https)
- Must include host and port

#### `cluster` (Required)
**Type**: `string`  
**Description**: Istio service mesh cluster name for HTTP dispatch  
**Format**: `outbound|<port>||<service>.<namespace>.svc.cluster.local`

**Examples**:
```yaml
cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
cluster: "outbound|8443||auth-service.auth-system.svc.cluster.local"
```

**Critical Notes**:
- Must match Istio's cluster naming convention
- Port must match the service port
- Service name must be fully qualified

#### `verify_path` (Required)
**Type**: `string`  
**Description**: Authentication verification endpoint path  
**Format**: Must start with `/`

**Standard Values**:
```yaml
verify_path: "/oauth2/auth"   # ext_authz standard
verify_path: "/auth"          # Alternative standard  
```

#### `timeout` (Optional)
**Type**: `integer`  
**Description**: Request timeout in milliseconds  
**Default**: `5000`  
**Range**: `1000` - `60000`

**Examples**:
```yaml
timeout: 5000    # 5 seconds (default)
timeout: 10000   # 10 seconds (production)
timeout: 30000   # 30 seconds (development)
```

#### `tls` (Optional)
**Type**: `object`  
**Description**: TLS configuration for HTTPS connections

##### `tls.verify_cert` (Optional)
**Type**: `boolean`  
**Description**: Enable TLS certificate verification  
**Default**: `true`

**Examples**:
```yaml
tls:
  verify_cert: false  # For HTTP or self-signed certs
  verify_cert: true   # For production with valid certs
```

##### `tls.ca_cert` (Optional)
**Type**: `string`  
**Description**: Path to CA certificate file for verification

##### `tls.client_cert` (Optional)
**Type**: `string`  
**Description**: Path to client certificate for mTLS

##### `tls.client_key` (Optional)
**Type**: `string`  
**Description**: Path to client private key for mTLS

## `global_auth` - Global Authentication Configuration

### Schema

```yaml
global_auth:
  enabled: boolean           # Optional (default: true)
  skip_paths: array          # Optional (default: [])
  require_auth_header: boolean # Optional (default: false)
```

### Parameters

#### `enabled` (Optional)
**Type**: `boolean`  
**Description**: Enable global authentication for all requests  
**Default**: `true`

#### `skip_paths` (Optional)
**Type**: `array` of `string`  
**Description**: Paths that bypass authentication  
**Default**: `[]`

**Examples**:
```yaml
skip_paths:
  - "/health"
  - "/readiness"
  - "/metrics"
  - "/favicon.ico"
```

#### `require_auth_header` (Optional)
**Type**: `boolean`  
**Description**: Require Authorization header to be present  
**Default**: `false`

## `error_responses` - Error Response Configuration

### Schema

```yaml
error_responses:
  auth_service_error:        # ErrorResponse - Optional
  access_denied:            # ErrorResponse - Optional
  timeout:                  # ErrorResponse - Optional
  invalid_config:           # ErrorResponse - Optional
```

### Error Response Schema

```yaml
status: integer              # HTTP status code
headers: array               # Optional response headers
body: string                 # Optional response body
```

### Parameters

#### `status` (Required)
**Type**: `integer`  
**Description**: HTTP status code to return  
**Range**: `100` - `599`

#### `headers` (Optional)
**Type**: `array` of `[string, string]`  
**Description**: HTTP headers to include in response

**Format**:
```yaml
headers:
  - ["content-type", "application/json"]
  - ["cache-control", "no-cache"]
```

#### `body` (Optional)
**Type**: `string`  
**Description**: Response body content

**Examples**:
```yaml
body: '{"error": "authentication_required"}'
body: |
  <!DOCTYPE html>
  <html><body><h1>Authentication Required</h1></body></html>
```

### Error Types

#### `auth_service_error`
**Triggered**: When authentication service is unreachable  
**Default Status**: `503 Service Unavailable`

#### `access_denied`  
**Triggered**: When authentication fails  
**Default Status**: `403 Forbidden`

#### `timeout`
**Triggered**: When authentication service times out  
**Default Status**: `504 Gateway Timeout`

#### `invalid_config`
**Triggered**: When plugin configuration is invalid  
**Default Status**: `500 Internal Server Error`

## `debug` - Debug Configuration

### Schema

```yaml
debug:
  enabled: boolean           # Optional (default: false)
  log_level: string          # Optional (default: "info")  
  log_headers: boolean       # Optional (default: false)
  log_body: boolean          # Optional (default: false)
```

### Parameters

#### `enabled` (Optional)
**Type**: `boolean`  
**Description**: Enable debug mode  
**Default**: `false`

#### `log_level` (Optional)
**Type**: `string`  
**Description**: Logging level  
**Values**: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`  
**Default**: `"info"`

#### `log_headers` (Optional)  
**Type**: `boolean`  
**Description**: Log HTTP headers (may expose sensitive data)  
**Default**: `false`

#### `log_body` (Optional)
**Type**: `boolean`  
**Description**: Log HTTP request/response bodies  
**Default**: `false`

## Complete Example

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
    auth_service:
      endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
      cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
      verify_path: "/oauth2/auth"
      timeout: 10000
      tls:
        verify_cert: false
    
    global_auth:
      enabled: true
      skip_paths:
        - "/health"
        - "/readiness"
        - "/metrics"
      require_auth_header: false
    
    error_responses:
      auth_service_error:
        status: 503
        headers:
          - ["content-type", "application/json"]
          - ["retry-after", "60"]
        body: '{"error": "authentication_service_unavailable"}'
      
      access_denied:
        status: 401  
        headers:
          - ["content-type", "application/json"]
        body: '{"error": "authentication_required", "message": "Please authenticate to access this resource"}'
    
    debug:
      enabled: false
      log_level: "info"
      log_headers: false
```

## Validation Rules

### Configuration Validation

The plugin validates configuration at startup with these rules:

1. **Required Fields**: `auth_service.endpoint`, `auth_service.cluster`, `auth_service.verify_path`
2. **URL Format**: `endpoint` must be valid URL with protocol
3. **Cluster Format**: `cluster` must match Istio naming convention
4. **Path Format**: `verify_path` must start with `/`
5. **Timeout Range**: Must be between 1000-60000 milliseconds
6. **Status Codes**: Must be valid HTTP status codes (100-599)

### Runtime Validation

During runtime, the plugin validates:

1. **Service Accessibility**: Authentication service must be reachable
2. **Response Format**: Authentication service must return valid HTTP responses
3. **Header Safety**: Headers are sanitized to prevent injection attacks

*For implementation details, see [ARCHITECTURE.md](ARCHITECTURE.md).*
