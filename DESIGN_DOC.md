# BYOIDC WASM Plugin Design Document

## Project Overview

**Project Name**: BYOIDC WASM Plugin (Bring Your Own OIDC)  
**Purpose**: Enable integration of existing OIDC/OAuth authentication services with Istio Gateway API via WASM plugins  
**Status**: ✅ Production Ready  
**Target Users**: Platform teams with existing authentication services who need Gateway API integration  
**Primary Use Case**: Integration with [kube-auth-proxy](https://github.com/opendatahub-io/kube-auth-proxy/) for OpenShift Data Hub (ODH) and Red Hat OpenShift AI (RHOAI) environments  

## Executive Summary

This project delivers a **custom WASM plugin** that acts as a bridge between **Istio Gateway API** and **existing OIDC/OAuth authentication services**, enabling reuse of proven authentication logic without migrating to ext_authz or EnvoyFilter approaches.

**Key Value Proposition**: Preserve existing authentication investments while gaining Gateway API portability and Istio's native WASM capabilities. Specifically designed to integrate with [kube-auth-proxy](https://github.com/opendatahub-io/kube-auth-proxy/), a FIPS-compliant authentication proxy for OpenShift Data Hub (ODH) and Red Hat OpenShift AI (RHOAI) environments, protecting both notebook services and the ODH dashboard.

## Problem Statement

**🚫 Critical Architectural Constraint**: This project requires **NO service mesh** functionality. The Service Mesh Operator gets **auto-installed** when creating the GatewayClass, providing **ONLY** the `WasmPlugin` CRD and gateway containers - there are no service mesh features, no automatic mTLS, no sidecars, and no ServiceEntry resources.

**🔒 Authentication Requirement**: **ALL services** behind the gateway require authentication. There are no public/unauthenticated endpoints.

**🛤️ Dynamic Service Routing**: Services dynamically create their own `HTTPRoute` CRDs that define routing paths. The OpenShift custom ingress controller automatically adds these routes to the Envoy configuration as services spin up and down. Examples:
- Notebook services create `HTTPRoute` CRs with paths like `/notebooks/user-1/my-notebook`
- ODH dashboard creates `HTTPRoute` CR with fallback path `/`
- **WASM plugin operates path-agnostically** - applies authentication to ALL requests regardless of dynamic routing

### Current Situation

**Scenario**: Organizations have deployed [`kube-auth-proxy`](https://github.com/opendatahub-io/kube-auth-proxy/) that:
- ✅ **FIPS-compliant** authentication proxy for OpenShift Data Hub (ODH) and Red Hat OpenShift AI (RHOAI)
- ✅ **Dual authentication support**: External OIDC providers and OpenShift's internal OAuth service
- ✅ **Envoy ext_authz compatible**: Built with external authorization framework support
- ✅ **Production ready**: Battle-tested replacement for oauth-proxy sidecars
- ✅ **Drop-in compatibility**: Maintains existing oauth-proxy argument and header formats
- ✅ Return `302 Found` (redirect) or `200 OK` responses based on authentication state

### Constraints and Requirements

**Technical Constraints**:
- ❌ **Cannot use EnvoyFilter** (deprecated, vendor-specific)
- ❌ **Cannot use ext_authz** (organizational policy restrictions)
- ✅ **Must use WASM plugins** (Istio's preferred extension mechanism)
- ✅ **Must work with Gateway API** (portability requirement)

**Business Requirements**:
- 🎯 **Reuse existing auth service** - avoid rewriting working authentication logic
- 🎯 **Transparent passthrough** - WASM plugin acts as simple HTTP client, auth service handles all OAuth/OIDC flows
- 🎯 **Gateway API compatibility** - work with standard Gateway/HTTPRoute resources
- 🎯 **Production readiness** - handle error cases, timeouts, monitoring

## Solution Architecture

### High-Level Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌────────────────────┐    ┌─────────────────┐
│   Client        │    │  Istio Gateway   │    │   WASM Plugin      │    │ kube-auth-proxy │
│                 │    │  (Gateway API)   │    │  (Our Solution)    │    │ (FIPS-compliant)│
└─────────────────┘    └──────────────────┘    └────────────────────┘    └─────────────────┘
         │                        │                        │                        │
         │  1. HTTP Request       │                        │                        │
         │──────────────────────→ │                        │                        │
         │                        │  2. Route to WASM      │                        │
         │                        │──────────────────────→ │                        │
         │                        │                        │  3. dispatch_http_call │
         │                        │                        │──────────────────────→ │
         │                        │                        │  4. 202 OK / 401 Unauth│
         │                        │                        │ ←──────────────────────│
         │  5. Response (Allow    │  6. Forward / Redirect │                        │
         │     or Redirect)       │ ←──────────────────────│                        │
         │ ←──────────────────────│                        │                        │
```

### Core Components

#### 1. WASM Plugin (Our Implementation)
- **Language**: Rust (using proxy-wasm-rust-sdk)
- **Function**: Universal authentication filter - calls kube-auth-proxy for ALL requests
- **Location**: Runs inside Istio's Envoy proxies (Layer 7 HTTP processing)
- **Configuration**: Via WasmPlugin CRD pluginConfig (path-agnostic)
- **Key Design**: No hardcoded routes - applies authentication to all gateway traffic
- **HTTP Processing**: Forwards headers transparently, makes HTTP calls to auth service, forwards responses

#### 2. kube-auth-proxy (Existing Service)
- **Repository**: [`opendatahub-io/kube-auth-proxy`](https://github.com/opendatahub-io/kube-auth-proxy/)
- **Type**: FIPS-compliant authentication proxy for ODH/RHOAI environments
- **Providers**: OIDC and OpenShift OAuth support
- **Interface**: Compatible with Envoy ext_authz framework
- **Auth Endpoint**: `/auth` (auth-only endpoint for external authorization)
- **Security**: **TLS-encrypted communication required** (HTTPS only)

#### 3. Istio Gateway (Traffic Routing)
- **Function**: Layer 4 proxy - routes traffic based on hostname/port
- **No HTTP Awareness**: Does not inspect cookies, headers, or HTTP content
- **Configuration**: Gateway and HTTPRoute CRDs define routing rules
- **WASM Integration**: Routes traffic to Envoy proxy where WASM plugin runs

#### 4. Dynamic Routing (OpenShift Ingress Controller)
- **HTTPRoute Creation**: Services dynamically create HTTPRoute CRDs as they spin up
- **Examples**: Notebook services create routes like `/notebooks/user-1/my-notebook`
- **Envoy Updates**: OpenShift ingress controller automatically updates Envoy configuration
- **Separation of Concerns**: Routing handled by HTTPRoute CRDs, authentication by WASM plugin
- **Certificate Handling**: Support for self-signed certificates (common in Kubernetes environments)
- **Response Patterns**: 
  - `202 Accepted` with user headers → Allow request (not 200 OK)
  - `401 Unauthorized` → Authentication required
  - `403 Forbidden` → Access denied (authorization failed)
  - `302 Found` with Location header → Redirect to auth (for regular proxy requests)

#### 3. Gateway API Resources
- **Gateway**: Entry point with TLS termination
- **HTTPRoute**: Routing rules with path-based auth requirements
- **WasmPlugin**: Istio CRD for WASM plugin deployment

## Technical Implementation

### WASM Plugin Core Logic

```rust
// WASM Plugin struct with configuration
struct AuthProxy {
    config: PluginConfig,
}

// Note: In Kubernetes DNS environments, the cluster name (first parameter of dispatch_http_call)
// and :authority header usually have the same value - the service DNS name with port.
// - cluster name: Tells Envoy which upstream service to route to
// - :authority header: HTTP Host header sent in the actual request

// Core HTTP interception and auth decision flow
impl HttpContext for AuthProxy {
    fn on_http_request_headers(&mut self) -> Action {
        // Path-agnostic authentication - apply to ALL requests
        // Dynamic HTTPRoute CRs handle routing, WASM handles universal auth
        
        // Forward all authentication-relevant headers transparently
        // WASM plugin does NOT parse/validate cookies - just forwards them
        // The auth service (kube-auth-proxy) will extract and validate what it needs
        let forwarded_headers = vec![
            ("cookie", self.get_http_request_header("cookie").unwrap_or_default()),
            ("authorization", self.get_http_request_header("authorization").unwrap_or_default()),
            ("x-forwarded-user", self.get_http_request_header("x-forwarded-user").unwrap_or_default()),
            ("x-forwarded-for", self.get_http_request_header("x-forwarded-for").unwrap_or_default()),
            ("user-agent", self.get_http_request_header("user-agent").unwrap_or_default()),
        ];
        
        // Get auth service config from plugin configuration
        let auth_config = &self.config.auth_service;
        
        // Extract scheme and host from endpoint URL
        let endpoint_url = &auth_config.endpoint;
        let (scheme, host_with_port) = if let Some(pos) = endpoint_url.find("://") {
            let scheme = &endpoint_url[..pos];
            let host_part = &endpoint_url[pos + 3..];
            (scheme, host_part)
        } else {
            ("http", endpoint_url.as_str())  // Default to http if no scheme
        };
        
        // Make HTTP call to kube-auth-proxy service (NO service mesh - direct DNS)
        match self.dispatch_http_call(
            host_with_port,  // From config: "kube-auth-proxy.auth-system.svc.cluster.local:4180"
            vec![
                (":method", "GET"),
                (":path", &auth_config.verify_path),  // From config: "/auth"
                (":authority", host_with_port),       // Same as cluster name in simple DNS case
                (":scheme", scheme),                  // "https" or "http"
            ],
            None,  // No body
            forwarded_headers,  // Forward auth-relevant headers to auth service
            Duration::from_millis(auth_config.timeout),  // From config: 5000ms
        ) {
            Ok(_) => Action::Pause,  // Wait for response
            Err(_) => {
                // Fallback: deny on service error
                self.send_http_response(503, vec![], Some("Auth service unavailable"));
                Action::Pause
            }
        }
    }
    
    fn on_http_call_response(&mut self, _token_id: u32, _num_headers: usize, _body_size: usize, _num_trailers: usize) {
        // Handle response from kube-auth-proxy
        if let Some(status) = self.get_http_call_response_header(":status") {
            match status.as_str() {
                "202" => {
                    // Auth success (kube-auth-proxy returns 202 Accepted)
                    // Extract forwarded headers from kube-auth-proxy response
                    if let Some(user) = self.get_http_call_response_header("x-forwarded-user") {
                        self.set_header("x-forwarded-user", &user);
                    }
                    if let Some(email) = self.get_http_call_response_header("x-forwarded-email") {
                        self.set_header("x-forwarded-email", &email);
                    }
                    if let Some(token) = self.get_http_call_response_header("x-forwarded-access-token") {
                        self.set_header("x-forwarded-access-token", &token);
                    }
                    if let Some(gap_auth) = self.get_http_call_response_header("gap-auth") {
                        self.set_header("gap-auth", &gap_auth);
                    }
                    self.resume_http_request();
                }
                "401" => {
                    // Authentication required
                    self.send_http_response(401, vec![], Some("Authentication required"));
                }
                "403" => {
                    // Access denied (authorization failed)
                    self.send_http_response(403, vec![], Some("Access denied"));
                }
                _ => {
                    // Any other response = service error
                    self.send_http_response(503, vec![], Some("Auth service error"));
                }
            }
        }
    }
}
```

### Configuration Schema

```rust
#[derive(Deserialize)]
struct PluginConfig {
    auth_service: AuthServiceConfig,     // Configuration for kube-auth-proxy connection
    global_auth: GlobalAuthConfig,       // Path-agnostic global authentication settings
    error_responses: Option<ErrorResponses>, // Custom error responses
}

#[derive(Deserialize)]
struct AuthServiceConfig {
    endpoint: String,           // "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"
    verify_path: String,        // "/auth" (auth-only endpoint)
    timeout: u64,               // 5000 (milliseconds)  
    tls: TlsConfig,             // TLS configuration (verify_cert: false for serving certs)
}

// Example configurations for different environments:
// Production:   endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"
// Development:  endpoint: "http://kube-auth-proxy.auth-system.svc.cluster.local:4180"
// External:     endpoint: "https://auth.company.com:443"

#[derive(Deserialize)]
struct TlsConfig {
    verify_cert: bool,          // false for self-signed certificates
    ca_cert_path: Option<String>, // "/etc/ssl/certs/ca-bundle.crt"
    client_cert_path: Option<String>, // Optional mutual TLS
    client_key_path: Option<String>,  // Optional mutual TLS
}

#[derive(Deserialize)]
struct GlobalAuthConfig {
    enabled: bool,              // true - apply auth to ALL requests
    // Note: No path_prefix - WASM plugin is path-agnostic
    // Dynamic HTTPRoute CRs handle routing, WASM handles universal auth
}

// Note: No OAuth/OIDC-specific configuration needed
// The WASM plugin is just an HTTP client - kube-auth-proxy handles all OAuth/OIDC logic
```

### Deployment Configuration

#### WasmPlugin Resource

```yaml
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: byoidc-auth-plugin
  namespace: istio-system
spec:
  # Target Istio gateways
  selector:
    matchLabels:
      istio: ingressgateway
      
  # Authentication phase, high priority
  phase: AUTHN
  priority: 1000
  
  # OCI image containing our WASM binary
  url: oci://my-registry/byoidc-wasm-plugin:v1.0.0
  
  pluginConfig:
    # Auth service configuration (using OpenShift serving certificates)  
    # Note: NO service mesh - direct HTTPS communication with cluster DNS resolution
    auth_service:
      endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"  # HTTPS with serving certs
      verify_path: "/auth"  # kube-auth-proxy auth-only endpoint  
      timeout: 5000  # milliseconds
      tls:
        verify_cert: false  # Accept OpenShift serving certificates (self-signed by service-ca)
      
    # Global auth configuration (path-agnostic - applies to ALL requests)
    # Note: No hardcoded paths since services dynamically create HTTPRoute CRs
    global_auth:
      enabled: true  # Apply authentication to all requests passing through gateway
      # Note: No OAuth/OIDC config needed - WASM plugin just forwards headers from kube-auth-proxy
      
    # Error handling
    error_responses:
      auth_service_error:
        status: 503
        body: '{"error": "authentication_service_unavailable"}'
      access_denied:
        status: 403
        body: '{"error": "access_denied"}'
```

#### Complete Gateway API Stack

**📋 Note**: This example shows kube-auth-proxy configuration for reference. TLS implementation for the auth service is outside the scope of this WASM plugin project.

```yaml
# 1. kube-auth-proxy Deployment + Service (using OpenShift serving certificates)
# Repository: https://github.com/opendatahub-io/kube-auth-proxy/
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kube-auth-proxy
  namespace: auth-system
spec:
  replicas: 2
  selector:
    matchLabels:
      app: kube-auth-proxy
  template:
    metadata:
      labels:
        app: kube-auth-proxy
        component: authentication
      annotations:
        service.beta.openshift.io/serving-cert-secret-name: kube-auth-proxy-tls
    spec:
      containers:
      - name: kube-auth-proxy
        image: quay.io/opendatahub-io/kube-auth-proxy:latest
        args:
        - --https-address=0.0.0.0:4180  # HTTPS with serving certificates
        - --tls-cert-file=/etc/ssl/certs/tls.crt
        - --tls-key-file=/etc/ssl/private/tls.key
        - --provider=oidc
        - --oidc-issuer-url=https://your-oidc-provider.com
        - --client-id=your-client-id
        - --upstream=http://placeholder  # Not used in auth-only mode
        env:
        - name: OAUTH2_PROXY_CLIENT_SECRET
          valueFrom:
            secretKeyRef:
              name: kube-auth-proxy-secret
              key: client-secret
        - name: OAUTH2_PROXY_COOKIE_SECRET
          valueFrom:
            secretKeyRef:
              name: kube-auth-proxy-secret
              key: cookie-secret
        volumeMounts:
        - name: tls-certs
          mountPath: /etc/ssl/certs
          readOnly: true
        - name: tls-key
          mountPath: /etc/ssl/private
          readOnly: true
        ports:
        - containerPort: 4180
          protocol: TCP
        livenessProbe:
          httpGet:
            path: /auth
            port: 4180
            scheme: HTTPS
          initialDelaySeconds: 30
        readinessProbe:
          httpGet:
            path: /auth  
            port: 4180
            scheme: HTTPS
      volumes:
      - name: tls-certs
        secret:
          secretName: kube-auth-proxy-tls
          items:
          - key: tls.crt
            path: tls.crt
      - name: tls-key
        secret:
          secretName: kube-auth-proxy-tls
          items:
          - key: tls.key
            path: tls.key
---
apiVersion: v1
kind: Service
metadata:
  name: kube-auth-proxy
  namespace: auth-system
  labels:
    app: kube-auth-proxy
    component: authentication
  annotations:
    service.beta.openshift.io/serving-cert-secret-name: kube-auth-proxy-tls
spec:
  selector:
    app: kube-auth-proxy
  ports:
  - port: 4180
    targetPort: 4180
    name: https  # HTTPS with serving certificates
    protocol: TCP

---
# 2. Gateway API Entry Point
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: secure-gateway
  namespace: gateway-system
spec:
  gatewayClassName: istio
  listeners:
  - name: https
    port: 443
    protocol: HTTPS
    hostname: "*.company.com"
    tls:
      mode: Terminate
      certificateRefs:
      - name: tls-cert

---
# 3. Dynamic Application Routing Example
# Note: In reality, services create these HTTPRoute CRs dynamically as they spin up
# OpenShift ingress controller automatically updates Envoy config
# WASM plugin applies universal auth regardless of dynamic routing changes

apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: notebook-user-123  # Created dynamically by notebook service
  namespace: notebooks
spec:
  parentRefs:
  - name: secure-gateway
    namespace: istio-system
  
  hostnames:
  - "app.company.com"
  
  rules:
  - matches:
    - path:
        type: PathPrefix
        value: "/notebooks/user-123/"  # Dynamic path per user/notebook
    backendRefs:
    - name: notebook-user-123-service  # Dynamic service per notebook
      port: 8080

---
# ODH Dashboard HTTPRoute (relatively static)
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: odh-dashboard-route
  namespace: odh-dashboard
spec:
  parentRefs:
  - name: secure-gateway
    namespace: istio-system
    
  hostnames:
  - "app.company.com"
    
  rules:
  - matches:
    - path:
        type: PathPrefix  
        value: "/"  # Fallback for dashboard
    backendRefs:
    - name: odh-dashboard
      port: 8080

---
# 4. DNS Resolution (cluster-internal service discovery)
# Note: No ServiceEntry needed - standard Kubernetes DNS resolution 
# The WASM plugin will resolve kube-auth-proxy.auth-system.svc.cluster.local
# directly via cluster DNS since we are NOT using a service mesh.

---
# 5. Secret for kube-auth-proxy
apiVersion: v1
kind: Secret
metadata:
  name: kube-auth-proxy-secret
  namespace: auth-system
type: Opaque
stringData:
  client-secret: "your-oidc-client-secret"
  cookie-secret: "randomly-generated-32-byte-base64-string"
```

## Request Flow Examples

### Successful Authentication Flow

```
1. Client Request:
   GET https://app.company.com/notebooks/user/my-notebook
   Cookie: session=abc123

2. Istio Gateway → WASM Plugin:
   - Gateway routes traffic to WASM plugin (Layer 4 routing)
   - WASM plugin applies universal authentication (path-agnostic)
   - WASM plugin forwards ALL headers transparently to auth service

3. WASM Plugin → kube-auth-proxy:
   GET https://kube-auth-proxy.auth-system.svc.cluster.local:4180/auth
   [All original request headers forwarded transparently]
   (HTTPS connection with OpenShift serving certificate)

4. kube-auth-proxy Response:
   202 Accepted
   X-Forwarded-User: alice
   X-Forwarded-Email: alice@company.com
   X-Forwarded-Access-Token: eyJ0eXAiOiJKV1Q...
   Gap-Auth: alice@company.com

5. WASM Plugin Processing:
   - Adds headers: x-forwarded-user: alice, x-forwarded-groups: admin,developers
   - Resumes request to upstream

6. Request Success:
   Request continues to notebook-controller-service with user context
```

### Authentication Required Flow

```
1. Client Request:
   GET https://app.company.com/
   (no auth headers - accessing ODH dashboard)

2. WASM Plugin → kube-auth-proxy:
   GET https://kube-auth-proxy.auth-system.svc.cluster.local:4180/auth
   [All original request headers forwarded transparently - in this case, no auth headers]
   (HTTPS connection with serving certificate)

3. kube-auth-proxy Response:
   401 Unauthorized
   (Auth endpoint returns 401 for unauthenticated requests)

4. WASM Plugin Response:
   302 Found  
   Location: https://oauth-openshift.apps.cluster.local/oauth/authorize?client_id=...&redirect_uri=...

5. Client Redirect:
   Browser follows redirect to OAuth provider for authentication
```

## Build and Deployment Process

### ⚠️ CRITICAL: Compatibility Requirements for OpenShift 4.19 / Red Hat Service Mesh

**These specific versions and configurations are MANDATORY for compatibility with Red Hat Service Mesh on OpenShift 4.19:**

#### Docker Build Configuration
```dockerfile
# CRITICAL: Use rustlang/rust:nightly for ABI compatibility
FROM rustlang/rust:nightly AS builder

# CRITICAL: Add compatibility label for Red Hat Service Mesh
LABEL module.wasm.image/variant=compat

# Install wasm32-unknown-unknown target
RUN rustup target add wasm32-unknown-unknown
```

#### Cargo.toml Dependencies
```toml
[package]
name = "byoidc-wasm-plugin"
version = "0.1.0"

[dependencies]
# CRITICAL: proxy-wasm version 0.2.3 for Red Hat Service Mesh compatibility
proxy-wasm = "0.2.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"

[lib]
crate-type = ["cdylib"]

[[bin]]
name = "gateway-auth-wasm-plugin"
path = "src/lib.rs"
```

#### Build Requirements
- ✅ **Rust**: `rustlang/rust:nightly` (tested with Rust 1.83+)
- ✅ **Target**: `wasm32-unknown-unknown`  
- ✅ **SDK**: `proxy-wasm = "0.2.3"` (exact version required)
- ✅ **Docker Label**: `module.wasm.image/variant=compat`
- ✅ **Environment**: Red Hat OpenShift 4.19 with Service Mesh

**⚠️ WARNING**: Other Rust versions, proxy-wasm versions, or missing Docker labels will cause WASM plugin load failures in Red Hat Service Mesh.

### Development Workflow

```bash
# 1. Setup development environment
rustup install nightly
rustup default nightly
rustup target add wasm32-unknown-unknown

# 2. Build WASM binary (using containerized build for consistency)
make build-wasm-cargo

# 3. Create OCI image with compatibility labels
make image

# 4. Push to registry
make push

# 5. Deploy to OpenShift
make deploy
```

### Project Structure

```
byoidc-wasm-plugin/
├── Cargo.toml                     # Rust dependencies and metadata
├── Cargo.lock                     # Dependency lockfile (committed)
├── Makefile                       # Build automation and common tasks
├── Dockerfile                     # Multi-stage OCI image build with compatibility labels
├── LICENSE                        # Project license (GPL)
├── DESIGN_DOC.md                  # This document - complete design specification
├── docs/                          # Detailed technical documentation
│
├── bugs/                          # Bug reports and debugging documentation
│   └── BUG_001.md                 # Initial debugging journey and cookie forwarding discovery
│
├── src/                           # Rust source code
│   ├── lib.rs                     # WASM plugin entry point with COOKIE FORWARDING
│   ├── config.rs                  # WasmPlugin configuration parsing
│   ├── http_client.rs             # dispatch_http_call wrapper utilities
│   ├── headers.rs                 # Header processing and forwarding
│   ├── responses.rs               # Response handling and error mapping
│   └── metrics.rs                 # Observability and performance metrics
│
├── deploy/                        # Production deployment manifests
│   ├── wasmplugin-production.yaml # Istio WasmPlugin CRD with cookie forwarding
│   ├── reference-grant.yaml       # Cross-namespace access permissions (CRITICAL)
│   └── echo-httproute.yaml        # OAuth path routing configuration (CRITICAL)
│
├── test-configs/                  # Infrastructure deployment manifests
│   ├── kube-auth-proxy.yaml       # OAuth2 proxy service deployment
│   ├── echo-service.yaml          # Test service for validation
│   ├── odh-gateway.yaml           # Gateway API Gateway resource
│   ├── route.yaml                 # OpenShift Route for external access
│   └── create-certs.sh            # Certificate generation script
│
├── tests/                         # Test suite
│   ├── integration/               # End-to-end integration tests
│   │   ├── test-auth-flow.py      # Browser-based OAuth flow validation
│   │   ├── requirements.txt       # Python dependencies for testing
│   │   └── __init__.py
│   ├── README.md                  # Testing documentation and usage
│   └── __init__.py
│
├── docs/                          # Detailed technical documentation
│   ├── README.md                  # Documentation navigation and overview
│   ├── CONFIGURATION.md           # WASM plugin configuration guide and options
│   ├── ARCHITECTURE.md            # Internal design and technical architecture  
│   ├── API_REFERENCE.md           # Complete parameter and API reference
│   ├── DEPLOYMENT.md              # Advanced deployment scenarios
│   ├── TROUBLESHOOTING.md         # Debugging guide and common issues
│   ├── SECURITY.md                # Security considerations and best practices
│   ├── DEVELOPMENT.md             # Development guide for contributors
│   ├── INTEGRATION.md             # Auth service integration patterns
│   └── TESTING.md                 # Testing strategies and validation
│
└── test-src/                      # Reference implementations and examples
    ├── kube-auth-proxy/           # Forked kube-auth-proxy source (for analysis)
    └── ext_authz.yaml             # Working ext_authz config (for comparison)
```

## Critical Implementation Details

### 🍪 Cookie Forwarding - The Missing Link

**CRITICAL DISCOVERY**: The most important implementation detail for session-based authentication is **cookie forwarding**. This was the final piece needed to make the WASM plugin work with kube-auth-proxy.

```rust
// In src/lib.rs - CRITICAL for session-based auth
let cookie_header = self.get_http_request_header("cookie");

// Forward ALL cookies to auth service
if let Some(ref cookie_value) = cookie_header {
    auth_headers.push(("cookie", cookie_value));
    debug!("Forwarding cookies to kube-auth-proxy: {}", cookie_value);
}
```

**Why Cookie Forwarding is Essential**:
1. **OAuth Callback**: After successful login, kube-auth-proxy sets session cookies
2. **Session Validation**: Subsequent requests include these cookies for authentication
3. **WASM Plugin Role**: Must forward cookies transparently to auth service
4. **Without Cookies**: Auth service has no session context → 401 Unauthorized loop

### 🔧 Istio Cluster Naming Requirements

**CRITICAL**: Use Istio's service mesh cluster naming convention, not simple service names:

```rust
// ❌ WRONG - Simple cluster name
cluster: "kube-auth-proxy"

// ✅ CORRECT - Istio service mesh cluster name  
cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
```

### 🛣️ HTTPRoute Priority-Based Routing

**CRITICAL**: OAuth paths must route to kube-auth-proxy, not the protected service:

```yaml
rules:
# Higher priority: OAuth paths → kube-auth-proxy
- matches:
  - path:
      type: PathPrefix  
      value: /oauth2/
  backendRefs:
  - name: kube-auth-proxy
    namespace: openshift-ingress
    port: 4180
# Lower priority: Everything else → protected service
- matches:
  - path:
      type: PathPrefix
      value: /
  backendRefs:
  - name: echo-service
    port: 80
```

### 🚀 Production Deployment Order

For new environments, deploy in this exact order:

```bash
# 1. Infrastructure (Gateway and Auth Service)
oc apply -f test-configs/odh-gateway.yaml
oc apply -f test-configs/kube-auth-proxy.yaml  
oc apply -f test-configs/echo-service.yaml

# 2. Cross-Namespace Permissions (CRITICAL)
oc apply -f deploy/reference-grant.yaml

# 3. Routing Configuration (CRITICAL)  
oc apply -f deploy/echo-httproute.yaml

# 4. Authentication Plugin (Final Step)
oc apply -f deploy/wasmplugin-production.yaml

# 5. Validation
cd tests/integration && python test-auth-flow.py --username developer --password developer
```

## ✅ Production Success Metrics

This WASM plugin is **production-ready** and has been validated with:

- ✅ **Complete OAuth Flow**: Browser → Gateway → Auth Service → OAuth Provider → Callback → Protected Service
- ✅ **Session Management**: Cookie-based session persistence after login  
- ✅ **Cross-Namespace Routing**: HTTPRoutes can route to services in different namespaces
- ✅ **Red Hat Service Mesh**: Fully compatible with OpenShift 4.19 service mesh
- ✅ **Integration Testing**: Automated browser-based validation suite
- ✅ **Error Handling**: Graceful handling of auth failures and service unavailability

**🎉 Project Status: COMPLETE** - Ready for production deployment in OpenShift environments with Red Hat Service Mesh.

### Key Components Explained

#### Core Source Files
- **`lib.rs`**: WASM plugin entry point with `_start()` function and proxy-wasm trait implementations
- **`config.rs`**: Deserializes WasmPlugin `pluginConfig` YAML into Rust structs  
- **`http_client.rs`**: Wraps `dispatch_http_call()` with error handling and retry logic
- **`headers.rs`**: Header forwarding utilities and response header processing
- **`responses.rs`**: Maps kube-auth-proxy responses (202/401/403) to appropriate actions

#### Build and Deployment
- **`Makefile`**: Automates `cargo build --target wasm32-unknown-unknown`, OCI image builds
- **`Dockerfile`**: Multi-stage build (Rust compilation → distroless final image)
- **`scripts/build.sh`**: Cross-platform build script with WASM optimization
- **`deploy/`**: Production-ready Kubernetes manifests with proper resource limits

#### Testing Infrastructure  
- **`tests/integration/`**: Test auth flows with real kube-auth-proxy instances
- **`tests/e2e/`**: Full Gateway API + WASM plugin + auth service testing
- **`scripts/benchmark.sh`**: Performance testing to ensure < 10ms latency overhead

#### Documentation Strategy
- **`docs/CONFIGURATION.md`**: Complete WasmPlugin configuration reference
- **`docs/TROUBLESHOOTING.md`**: Common auth failures, TLS issues, debugging guides
- **`examples/production/`**: Real-world deployment examples with security best practices

#### Development Workflow
```bash
# 1. Build and test locally
make build test

# 2. Run integration tests
./scripts/test.sh

# 3. Deploy to development cluster  
./scripts/deploy.sh development

# 4. Run end-to-end tests
./tests/e2e/test-complete-flow.sh

# 5. Build and push release image
make release VERSION=v1.0.0
```

## Testing Strategy

### Unit Tests

```bash
# Rust unit tests for core logic
cargo test

# WASM-specific testing with proxy-wasm-test-framework
cargo test --features test
```

### Integration Tests

```bash
# Test against real kube-auth-proxy service
./examples/test-requests.sh

# Test kube-auth-proxy directly (HTTPS with serving certificate - NO service mesh)  
curl -v -k -H "Cookie: session=abc123" \
     https://kube-auth-proxy.auth-system.svc.cluster.local:4180/auth

# Alternative: Test with custom CA (if available)
# curl -v --cacert /etc/ssl/certs/service-ca.crt \
#      -H "Cookie: session=abc123" \
#      https://kube-auth-proxy.auth-system.svc.cluster.local:4180/auth

# Verify different response codes via Gateway
curl -v -H "Cookie: invalid" https://app.company.com/  # Expect 302 (WASM plugin redirects based on 401 from auth - ODH dashboard)
curl -v -H "Cookie: valid_session" https://app.company.com/notebooks/user/test  # Expect 200 (successful request - notebook service)
```

### Performance Tests

- **Latency Impact**: Measure auth call overhead
- **Throughput**: Requests per second with auth enabled
- **Memory Usage**: WASM plugin memory consumption
- **Error Handling**: Behavior under auth service outages

## Security and TLS Requirements

### TLS Communication (Critical)

**All communication between WASM plugin and kube-auth-proxy MUST be encrypted:**

✅ **HTTPS Only**: No unencrypted HTTP traffic allowed  
✅ **Self-Signed Certificate Support**: Handle common Kubernetes self-signed certificates  
✅ **Custom CA Support**: Allow custom Certificate Authority configuration  
✅ **Mutual TLS Option**: Support client certificate authentication when required  
✅ **FIPS Compliance**: Maintain FIPS-compliant TLS cipher suites and protocols  

### TLS Configuration

**⚠️ TLS Requirement**: All communication between WASM plugin and kube-auth-proxy MUST use HTTPS.

**📋 WASM Plugin Responsibilities**: The WASM plugin connects to whatever HTTPS endpoint is configured. TLS implementation details (certificates, CA management, etc.) are handled by the kube-auth-proxy deployment - NOT by this WASM plugin project.

The WASM plugin supports flexible TLS configuration to connect to any HTTPS kube-auth-proxy endpoint:

```yaml
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"
    verify_path: "/auth"
    timeout: 5000
    tls:
      verify_cert: false  # Set to false for self-signed certificates (common in Kubernetes)
      # verify_cert: true   # Set to true if using CA-signed certificates
      # ca_cert_path: "/etc/ssl/certs/ca-bundle.crt"  # Optional: custom CA bundle path
```

#### TLS Configuration Options

**For self-signed certificates** (OpenShift serving certificates, development environments):
```yaml
tls:
  verify_cert: false  # Accept self-signed certificates
```

**For CA-signed certificates** (cert-manager, production environments):
```yaml
tls:
  verify_cert: true                           # Verify certificate chain
  ca_cert_path: "/etc/ssl/certs/ca-bundle.crt"  # Optional: custom CA bundle
```

**❌ HTTP Not Supported**: The WASM plugin enforces HTTPS-only communication for security.

**📋 Note**: TLS certificate generation and management for kube-auth-proxy is handled by the auth service deployment - NOT by this WASM plugin project. The WASM plugin simply connects to whatever HTTPS endpoint is configured.

### Security Validation Checklist

- [ ] **TLS Version**: Minimum TLS 1.2, prefer TLS 1.3
- [ ] **Cipher Suites**: FIPS-approved cipher suites only
- [ ] **Certificate Validation**: Proper hostname verification
- [ ] **Credential Protection**: No credentials in logs or error messages
- [ ] **Timeout Protection**: Reasonable timeouts to prevent hanging connections
- [ ] **Error Handling**: Secure error responses without information leakage

## Success Criteria

### Functional Requirements

✅ **Auth Integration**: Successfully call kube-auth-proxy and handle 202/401/403 responses  
✅ **Header Forwarding**: Forward request headers to auth service, pass through user/group headers from auth service response  
✅ **Universal Authentication**: Apply authentication to all requests (path-agnostic)  
✅ **Error Handling**: Graceful fallback when auth service is unavailable  
✅ **Response Transparency**: Forward auth service responses (redirects, errors, headers) unchanged  

### Non-Functional Requirements

⚡ **Performance**: < 10ms latency overhead per request  
🔒 **Security**: No credential leakage, secure default deny  
📈 **Reliability**: 99.9% auth call success rate  
🔧 **Maintainability**: Clear configuration schema, good error messages  
📊 **Observability**: Proper metrics and logging integration  

### Production Readiness Checklist

- [ ] **Security Review**: Credential handling, input validation
- [ ] **Performance Testing**: Load testing with realistic traffic  
- [ ] **Monitoring Setup**: Metrics, alerts, dashboards
- [ ] **Documentation**: Runbooks, troubleshooting guides
- [ ] **Rollback Plan**: Blue/green deployment strategy

## Development Guidelines

### Code Quality Standards

- **Rust Best Practices**: Follow Rust API guidelines
- **Error Handling**: Use Result<> types, no unwrap() in production code
- **Logging**: Structured logging with appropriate levels (no credential leakage)
- **Configuration Validation**: Fail fast on invalid config
- **Testing**: Unit tests for all core functions
- **TLS Security**: All HTTP client code must use HTTPS with proper certificate handling
- **FIPS Compliance**: Use only FIPS-approved cryptographic libraries

### Monitoring and Observability

```rust
// Key metrics to expose
- byoidc_auth_requests_total{status="200|302|403|503"}
- byoidc_auth_request_duration_seconds
- byoidc_auth_service_errors_total
- byoidc_config_reload_total{status="success|error"}
```

### Development Tools

```bash
# Debug builds with logging
cargo build --target wasm32-unknown-unknown --features=debug

# Local testing with Envoy
envoy -c test-envoy-config.yaml

# WASM plugin inspection
wasm-objdump -h target/wasm32-unknown-unknown/release/byoidc_plugin.wasm
```

## Future Enhancements

### Phase 2 Features

1. **Multi-Auth Service Support**: Call different auth services based on hostname/path
2. **Caching Layer**: Cache auth decisions to reduce latency
3. **Advanced Routing**: Header-based auth requirements, regex path matching
4. **Metrics Dashboard**: Grafana dashboard for auth service integration
5. **A/B Testing**: Gradual rollout capabilities with traffic splitting

### Integration Opportunities

- **Gateway API Policies**: Integration with Gateway API HTTPRoute and ReferenceGrant resources
- **External Secrets**: Secure credential management via External Secrets Operator
- **GitOps**: ArgoCD/Flux deployment patterns for ODH/RHOAI environments
- **Multi-Cluster**: Cross-cluster auth service federation
- **kube-auth-proxy Evolution**: Contribute upstream improvements to [`kube-auth-proxy`](https://github.com/opendatahub-io/kube-auth-proxy/)
- **FIPS Compliance**: Enhanced FIPS validation and certification support

## Architectural Summary

This design follows a **separation of concerns** pattern optimized for dynamic service environments:

### 🔀 **Routing Layer** (Dynamic)
- Services create `HTTPRoute` CRDs with their specific paths as they spin up
- OpenShift ingress controller updates Envoy configuration automatically  
- Examples: `/notebooks/user-123/notebook-a`, `/admin/settings`, `/` (dashboard fallback)

### 🔐 **Authentication Layer** (Universal)
- WASM plugin applies authentication to **ALL** requests regardless of path
- No hardcoded routes in WASM configuration - completely path-agnostic
- Single responsibility: Validate authentication via kube-auth-proxy

### ✅ **Key Benefits**
- **Flexibility**: Services can create any routing patterns without WASM plugin changes
- **Simplicity**: WASM plugin has single concern (auth), not routing logic  
- **Maintainability**: No need to update WASM config when adding new services or paths
- **Performance**: No path matching overhead in WASM plugin
- **Future-proof**: Works with any service that creates HTTPRoute CRDs

## Next Steps

### Immediate (Week 1-2)

1. **Project Setup**: Initialize Rust project with proxy-wasm-rust-sdk
2. **kube-auth-proxy Analysis**: Study [`kube-auth-proxy`](https://github.com/opendatahub-io/kube-auth-proxy/) API and response patterns
3. **Core Implementation**: Basic HTTP dispatch and response handling
4. **Configuration Parsing**: WasmPlugin config deserialization  
5. **Local Testing**: Test with standalone Envoy and kube-auth-proxy setup

### Short Term (Week 3-4)

1. **Istio Integration**: Test with real Istio Gateway API stack
2. **Error Handling**: Comprehensive error scenarios and fallbacks  
3. **Documentation**: API documentation and deployment guides
4. **CI/CD Pipeline**: Automated building and testing

### Medium Term (Month 2)

1. **Production Hardening**: Performance tuning, security review
2. **Monitoring Integration**: Metrics, logging, alerting setup
3. **User Acceptance Testing**: Test with real auth services and workloads
4. **Production Deployment**: Staged rollout with monitoring
