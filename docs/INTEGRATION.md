# Integration Guide

## Overview

This document explains how to integrate the BYOIDC WASM Plugin with different authentication services and deployment scenarios.

## Supported Authentication Services

### kube-auth-proxy (Primary)

**Description**: FIPS-compliant authentication proxy for OpenShift Data Hub (ODH) and Red Hat OpenShift AI (RHOAI).

**Repository**: https://github.com/opendatahub-io/kube-auth-proxy/

**Configuration**:
```yaml
pluginConfig:
  auth_service:
    endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
    verify_path: "/oauth2/auth"
    timeout: 5000
```

**kube-auth-proxy Configuration**:
```yaml
args:
- --provider=openshift
- --client-id=system:serviceaccount:openshift-ingress:kube-auth-proxy
- --openshift-service-account=kube-auth-proxy
- --http-address=0.0.0.0:4180
- --upstream=static://200
- --redirect-url=https://your-gateway.example.com/oauth2/callback
- --skip-provider-button  # Critical for WASM plugin integration
```

### oauth2-proxy (Compatible)

**Description**: OAuth2 proxy for cloud providers and identity systems.

**Configuration**:
```yaml
pluginConfig:
  auth_service:
    endpoint: "http://oauth2-proxy.auth-system.svc.cluster.local:4180"
    cluster: "outbound|4180||oauth2-proxy.auth-system.svc.cluster.local"
    verify_path: "/oauth2/auth"
```

**oauth2-proxy Configuration**:
```yaml
args:
- --provider=oidc
- --oidc-issuer-url=https://your-oidc-provider.com
- --client-id=your-client-id
- --http-address=0.0.0.0:4180
- --upstream=static://202
- --skip-provider-button
```

## Authentication Providers

### OpenShift OAuth

**Use Case**: Internal OpenShift authentication with existing user accounts.

**Integration Pattern**: WASM Plugin → kube-auth-proxy → OpenShift OAuth

**Configuration**:
```yaml
# kube-auth-proxy args for OpenShift OAuth
args:
- --provider=openshift
- --client-id=system:serviceaccount:openshift-ingress:kube-auth-proxy
- --client-secret-file=/var/run/secrets/kubernetes.io/serviceaccount/token
- --openshift-service-account=kube-auth-proxy
```

**ServiceAccount Setup**:
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
  annotations:
    serviceaccounts.openshift.io/oauth-redirecturi.primary: "https://your-gateway.example.com/oauth2/callback"
```

### External OIDC Providers

**Use Case**: Integration with external identity providers (Google, Azure AD, etc.).

**Integration Pattern**: WASM Plugin → kube-auth-proxy/oauth2-proxy → External OIDC

**Configuration Example (Google)**:
```yaml
# kube-auth-proxy args for Google OIDC
args:
- --provider=oidc
- --oidc-issuer-url=https://accounts.google.com
- --client-id=your-google-client-id.apps.googleusercontent.com
- --email-domain=your-company.com
```

**Configuration Example (Azure AD)**:
```yaml  
args:
- --provider=azure
- --azure-tenant=your-tenant-id
- --client-id=your-azure-app-id
```

### Keycloak / Red Hat SSO

**Use Case**: Enterprise identity management with Keycloak.

**Integration Pattern**: WASM Plugin → oauth2-proxy → Keycloak

**Configuration**:
```yaml
args:
- --provider=keycloak-oidc
- --oidc-issuer-url=https://keycloak.example.com/realms/your-realm
- --client-id=your-keycloak-client
- --scope=openid email profile
```

## Deployment Patterns

### Same-Namespace Deployment (Recommended)

**Benefits**:
- Simplified service discovery
- Reduced network complexity
- Easier RBAC management

**Architecture**:
```
openshift-ingress/
├── Gateway (odh-gateway)
├── WASM Plugin (gateway-auth-wasm-plugin)  
├── Auth Service (kube-auth-proxy)
└── HTTPRoute (routing configuration)
```

**Configuration**:
```yaml
# No cross-namespace configuration needed
auth_service:
  endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
  cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"
```

### Cross-Namespace Deployment

**Benefits**:
- Logical separation of concerns
- Independent scaling and management
- Multi-tenant architectures

**Architecture**:
```
openshift-ingress/        auth-system/
├── Gateway              ├── Auth Service
├── WASM Plugin          └── Secrets/Config
└── HTTPRoute (with ReferenceGrant)
```

**Required Components**:
```yaml
# ReferenceGrant for cross-namespace access
apiVersion: gateway.networking.k8s.io/v1beta1
kind: ReferenceGrant
metadata:
  name: allow-gateway-to-auth
  namespace: auth-system
spec:
  from:
  - group: gateway.networking.k8s.io
    kind: HTTPRoute
    namespace: openshift-ingress
  to:
  - group: ""
    kind: Service
    name: kube-auth-proxy
```

### Multi-Tenant Deployment

**Use Case**: Multiple tenants with different authentication requirements.

**Pattern**: Multiple WASM plugins with different configurations.

**Example**:
```yaml
# Tenant A - Internal Auth
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: tenant-a-auth
  namespace: tenant-a
spec:
  selector:
    matchLabels:
      tenant: tenant-a
  pluginConfig:
    auth_service:
      endpoint: "http://openshift-oauth.auth-system.svc.cluster.local:4180"
---
# Tenant B - External OIDC
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin  
metadata:
  name: tenant-b-auth
  namespace: tenant-b
spec:
  selector:
    matchLabels:
      tenant: tenant-b
  pluginConfig:
    auth_service:
      endpoint: "http://google-oauth.auth-system.svc.cluster.local:4180"
```

## Advanced Integration Scenarios

### Header-Based Routing

**Use Case**: Route to different auth services based on request headers.

**Implementation**: Multiple HTTPRoute rules with header matching.

```yaml
rules:
# Enterprise users → LDAP auth
- matches:
  - path:
      type: PathPrefix
      value: /
    headers:
    - name: X-User-Type
      value: enterprise
  backendRefs:
  - name: ldap-auth-service
    port: 4180

# Consumer users → Social auth  
- matches:
  - path:
      type: PathPrefix  
      value: /
    headers:
    - name: X-User-Type
      value: consumer
  backendRefs:
  - name: social-auth-service
    port: 4180
```

### Path-Based Authentication

**Use Case**: Different authentication requirements for different paths.

**Implementation**: Multiple WASM plugins with path-specific selectors.

```yaml
# Admin paths - Strong auth
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: admin-auth
spec:
  selector:
    matchLabels:
      gateway: admin-gateway
  pluginConfig:
    global_auth:
      enabled: true
    auth_service:
      endpoint: "http://strong-auth.auth-system.svc.cluster.local:4180"
      
# Public API - Basic auth
apiVersion: extensions.istio.io/v1alpha1  
kind: WasmPlugin
metadata:
  name: api-auth
spec:
  selector:
    matchLabels:
      gateway: api-gateway  
  pluginConfig:
    global_auth:
      enabled: true
      skip_paths:
        - "/public"
        - "/health"
    auth_service:
      endpoint: "http://basic-auth.auth-system.svc.cluster.local:4180"
```

## Integration Testing

### Auth Service Connectivity

```bash
# Test auth service from gateway pod
oc exec -n openshift-ingress deployment/router-default -- \
  curl -v http://kube-auth-proxy:4180/oauth2/auth

# Expected: 401 Unauthorized (no session)
```

### OAuth Flow Testing  

```bash
# Test OAuth initiation
curl -k -v https://your-gateway.example.com/oauth2/start

# Expected: 302 redirect to OAuth provider
```

### End-to-End Integration

```bash
# Run automated integration test
cd tests/integration/
python test-auth-flow.py --username user --password pass

# Expected: 3/3 tests passed
```

## Performance Considerations

### Request Latency

**Factors**:
- Auth service response time
- Network latency between components
- WASM plugin processing overhead

**Optimization**:
```yaml
# Tune timeout for performance vs reliability
auth_service:
  timeout: 3000  # Faster timeout for responsive services
```

### Scaling

**Auth Service Scaling**:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kube-auth-proxy
spec:
  replicas: 3  # Scale based on load
  template:
    spec:
      containers:
      - name: kube-auth-proxy
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi" 
            cpu: "200m"
```

## Troubleshooting Integration Issues

### Common Integration Problems

1. **Service Discovery Failures**
   - Verify service names and ports
   - Check namespace configurations
   - Validate cluster name format

2. **Authentication Flow Issues**  
   - Verify OAuth callback URLs
   - Check client ID/secret configuration
   - Validate redirect URI configuration

3. **Cross-Namespace Access**
   - Ensure ReferenceGrant exists
   - Verify RBAC permissions
   - Check NetworkPolicy restrictions

### Debug Commands

```bash
# Check service endpoints
oc get endpoints -n openshift-ingress kube-auth-proxy

# Verify HTTPRoute configuration
oc describe httproute -n echo-service

# Check WASM plugin logs
oc logs -n openshift-ingress deployment/router-default | grep -i wasm
```

## Migration Strategies

### From EnvoyFilter to WASM Plugin

**Migration Steps**:
1. Deploy WASM plugin alongside existing EnvoyFilter
2. Route small percentage of traffic to WASM plugin
3. Monitor and validate behavior
4. Gradually increase traffic percentage
5. Remove EnvoyFilter once fully migrated

### From ext_authz to WASM Plugin

**Compatibility**: WASM plugin uses same endpoints as ext_authz
- `/oauth2/auth` endpoint compatibility
- Same header forwarding patterns
- Compatible response codes

**Migration**: Direct replacement possible with minimal auth service changes

*For specific integration questions, refer to the auth service documentation or contact the development team.*
