# Troubleshooting Guide

## Common Issues and Solutions

### WASM Plugin Issues

#### Plugin Load Failures

**Symptom**: WASM plugin fails to load, Envoy logs show ABI compatibility errors.

**Cause**: Incompatible Rust toolchain or proxy-wasm version for Red Hat Service Mesh.

**Solution**:
```dockerfile
# Ensure correct Rust version in Dockerfile
FROM rustlang/rust:nightly AS builder

# Ensure compatibility label
LABEL module.wasm.image/variant=compat
```

```toml
# Ensure correct proxy-wasm version in Cargo.toml
[dependencies]
proxy-wasm = "0.2.3"  # Exact version required
```

#### Plugin Configuration Errors

**Symptom**: Plugin loads but fails during configuration parsing.

**Diagnostic Steps**:
```bash
# Check WASM plugin status
oc get wasmplugin -n openshift-ingress -o yaml

# Check configuration parsing logs  
oc logs -n openshift-ingress deployment/router-default | grep -i "config"
```

### Authentication Flow Issues

#### Authentication Loop After Login

**Symptom**: User successfully logs in via OAuth but gets redirected to login again.

**Root Cause**: Missing cookie forwarding in WASM plugin.

**Solution**: Verify cookie forwarding is implemented:
```rust
// Check src/lib.rs contains cookie forwarding
if let Some(ref cookie_value) = cookie_header {
    auth_headers.push(("cookie", cookie_value));
}
```

**Diagnostic Commands**:
```bash
# Check for cookie forwarding logs
oc logs -n openshift-ingress deployment/router-default | grep -i cookie

# Test with curl and cookies
curl -k -c /tmp/cookies.txt https://your-gateway.example.com/oauth2/start
curl -k -b /tmp/cookies.txt https://your-gateway.example.com/oauth2/auth
```

#### 401 Unauthorized Loop

**Symptom**: Continuous 401 responses, no redirect to OAuth.

**Diagnostic Steps**:
1. Check auth service endpoint:
```bash
# Verify auth service is reachable
oc exec -n openshift-ingress deployment/router-default -- curl -v http://kube-auth-proxy:4180/oauth2/auth
```

2. Verify cluster name configuration:
```bash
# Check Envoy cluster configuration
oc exec -n openshift-ingress deployment/router-default -- curl localhost:15000/clusters
```

#### HTTP Dispatch Failures

**Symptom**: `BadArgument` errors during HTTP dispatch to auth service.

**Common Causes**:
- Wrong cluster name format
- Service not reachable
- Port mismatch

**Solutions**:
```yaml
# Correct cluster name format
cluster: "outbound|4180||kube-auth-proxy.openshift-ingress.svc.cluster.local"

# Verify service exists
oc get service -n openshift-ingress kube-auth-proxy
```

### OAuth Integration Issues

#### OAuth Redirect Loop  

**Symptom**: Browser gets stuck redirecting between `/oauth2/start` and OAuth server.

**Root Cause**: WASM plugin intercepting OAuth callback paths.

**Solution**: Verify OAuth path skipping:
```rust
// Check is_auth_request() function skips OAuth paths
if path.starts_with("/oauth2/") {
    return true;  // Skip authentication for OAuth paths
}
```

#### Cross-Namespace Access Denied

**Symptom**: HTTPRoute cannot access auth service in different namespace.

**Solution**: Deploy ReferenceGrant:
```bash
# Apply cross-namespace permissions
oc apply -f deploy/reference-grant.yaml
```

### Service Mesh / Gateway API Issues

#### Service Mesh Auto-Installation Problems

**Symptom**: WasmPlugin CRD not available after creating Gateway.

**Solution**: Verify GatewayClass has correct controller name:
```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: GatewayClass
metadata:
  name: istio
spec:
  controllerName: openshift.io/gateway-controller  # Triggers auto-installation
```

#### HTTPRoute Routing Issues

**Symptom**: OAuth paths routing to wrong service.

**Diagnostic**:
```bash
# Check HTTPRoute configuration
oc describe httproute -n echo-service

# Verify rule priority (OAuth paths should be first)
```

**Solution**: Ensure OAuth paths have higher priority:
```yaml
rules:
# FIRST rule (higher priority): OAuth paths
- matches:
  - path:
      type: PathPrefix
      value: /oauth2/
  backendRefs:
  - name: kube-auth-proxy
# SECOND rule: Everything else
- matches:
  - path:
      type: PathPrefix 
      value: /
```

## Diagnostic Commands

### Environment Validation

```bash
# Check OpenShift version
oc version

# Verify CRCs IP and route accessibility  
crc ip
curl -k -I https://odh-gateway.apps-crc.testing/

# Check service mesh components
oc get pods -n openshift-ingress | grep router
oc get crd | grep -i wasm
```

### Plugin Status

```bash
# WASM plugin status
oc get wasmplugin -n openshift-ingress

# Plugin configuration
oc get wasmplugin -n openshift-ingress -o yaml

# Envoy configuration
oc exec -n openshift-ingress deployment/router-default -- curl localhost:15000/config_dump
```

### Authentication Service Status

```bash
# Auth service pods
oc get pods -n openshift-ingress | grep kube-auth-proxy

# Service endpoints
oc get endpoints -n openshift-ingress kube-auth-proxy

# Test auth service directly
oc exec -n openshift-ingress deployment/router-default -- curl -v http://kube-auth-proxy:4180/oauth2/auth
```

### Log Analysis

```bash
# WASM plugin logs
oc logs -n openshift-ingress deployment/router-default | grep -i wasm

# Authentication logs  
oc logs -n openshift-ingress deployment/kube-auth-proxy

# OAuth server logs
oc logs -n openshift-authentication deployment/oauth-openshift
```

## Performance Issues

### High Latency

**Symptom**: Slow authentication response times.

**Diagnostic**:
- Check auth service response times
- Monitor network latency between components
- Verify timeout configurations

**Solutions**:
- Increase auth service resources
- Optimize auth service configuration
- Adjust WASM plugin timeout settings

### Memory Issues

**Symptom**: Pod restarts due to memory limits.

**Diagnostic**:
```bash
# Check memory usage
oc top pods -n openshift-ingress

# Check resource limits
oc describe pod -n openshift-ingress <router-pod>
```

## Getting Help

### Log Collection

```bash
# Collect comprehensive logs for support
oc logs -n openshift-ingress deployment/router-default > gateway-logs.txt
oc logs -n openshift-ingress deployment/kube-auth-proxy > auth-logs.txt  
oc get wasmplugin -n openshift-ingress -o yaml > wasm-config.yaml
```

### Integration Testing

```bash
# Run automated integration tests
cd tests/integration/
python test-auth-flow.py --username developer --password developer --browser chrome
```

*For issues not covered here, see [bugs/BUG_001.md](../bugs/BUG_001.md) for the complete debugging journey that led to the cookie forwarding solution.*
