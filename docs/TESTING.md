# Testing Guide

## Overview

This document covers testing strategies, procedures, and validation approaches for the BYOIDC WASM Plugin.

## Testing Strategy

### Test Pyramid

```
    /\
   /  \     Unit Tests (Rust)
  /____\    
 /      \   Integration Tests (Python/Browser)
/________\  End-to-End Tests (Live Environment)
```

## Unit Testing

### Rust Unit Tests

**Location**: `src/*.rs` files with `#[cfg(test)]` modules

**Current Coverage**: [ ] TODO - Implement unit tests

**Example Test Structure**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = PluginConfig {
            auth_service: AuthServiceConfig {
                endpoint: "http://test:8080".to_string(),
                cluster: "outbound|8080||test.svc.cluster.local".to_string(),
                verify_path: "/auth".to_string(),
                timeout: 5000,
                tls: None,
            },
            global_auth: None,
            error_responses: None,
        };
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let config = PluginConfig {
            auth_service: AuthServiceConfig {
                endpoint: "".to_string(),  // Invalid empty endpoint
                cluster: "invalid".to_string(),
                verify_path: "/auth".to_string(),
                timeout: 5000,
                tls: None,
            },
            global_auth: None,
            error_responses: None,
        };
        
        assert!(config.validate().is_err());
    }
}
```

**Running Unit Tests**:
```bash
# Run all unit tests
cargo test

# Run with output  
cargo test -- --nocapture

# Run specific test
cargo test test_config_validation
```

## Integration Testing

### Browser-Based Authentication Flow Tests

**Location**: `tests/integration/test-auth-flow.py`

**Purpose**: Validate complete OAuth authentication flow using real browser automation.

**Test Scenarios**:
1. **Initial Gateway Access**: User redirected to OAuth login
2. **OAuth Login Form**: User authentication successful
3. **Post-Login Access**: Session cookies validated, access granted

**Running Integration Tests**:
```bash
# Setup test environment
cd tests/integration/
source ../../venv/bin/activate
pip install -r requirements.txt

# Run headless tests (CI/automated)
python test-auth-flow.py --username developer --password developer --browser chrome

# Run visible tests (debugging)
python test-auth-flow.py --username developer --password developer --browser chrome --no-headless

# Run with different browsers
python test-auth-flow.py --username developer --password developer --browser firefox
```

**Test Configuration**:
```python
# Customize test behavior
TEST_GATEWAY_URL = "https://odh-gateway.apps-crc.testing"
TEST_USERNAME = "developer"  
TEST_PASSWORD = "developer"
BROWSER_TIMEOUT = 30  # seconds
```

### Component Integration Tests

**HTTP Client Tests**:
```bash
# Test auth service connectivity
curl -v http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180/oauth2/auth

# Test OAuth flow initiation  
curl -k -v https://odh-gateway.apps-crc.testing/oauth2/start

# Test with session cookies
curl -k -c /tmp/cookies.txt https://odh-gateway.apps-crc.testing/oauth2/start
curl -k -b /tmp/cookies.txt https://odh-gateway.apps-crc.testing/
```

**Configuration Validation Tests**:
```bash
# Test valid configuration
oc apply -f deploy/wasmplugin-production.yaml
oc get wasmplugin -n openshift-ingress -o yaml

# Test invalid configuration (should fail)
cat <<EOF | oc apply -f -
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: test-invalid-config
  namespace: openshift-ingress
spec:
  pluginConfig:
    auth_service:
      endpoint: ""  # Invalid empty endpoint
      cluster: "invalid"
      verify_path: "/auth"
EOF
```

## End-to-End Testing

### Production Environment Validation

**Prerequisites**:
- Complete deployment (infrastructure + authentication)
- Valid user credentials
- External network access

**Test Scenarios**:

#### 1. Unauthenticated Access
```bash
# Should redirect to OAuth login
curl -k -I https://your-gateway.example.com/
# Expected: 302 Found with Location header
```

#### 2. OAuth Flow Completion
```bash
# Manual browser test
# 1. Navigate to https://your-gateway.example.com/
# 2. Follow OAuth redirect
# 3. Enter credentials  
# 4. Verify access to protected resource
```

#### 3. Session Persistence
```bash
# Test session cookie persistence
curl -k -c /tmp/session.txt https://your-gateway.example.com/oauth2/start
# Complete OAuth flow to get session cookie
curl -k -b /tmp/session.txt https://your-gateway.example.com/
# Expected: 200 OK (authenticated access)
```

### Load Testing

**Basic Load Test**:
```bash
# Install hey (HTTP load testing tool)
go install github.com/rakyll/hey@latest

# Run load test against authenticated endpoint
hey -n 1000 -c 10 -H "Cookie: session=valid_session_cookie" \
    https://your-gateway.example.com/
```

**Auth Service Load Test**:
```bash
# Test auth service directly
hey -n 1000 -c 10 \
    http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180/oauth2/auth
```

## Test Data Management

### Test User Accounts

**OpenShift OAuth Test Users**:
```bash
# Default CRC users
username: developer
password: developer

username: kubeadmin  
password: <generated-password>
```

**External OIDC Test Users**:
```bash
# Configure test accounts in your OIDC provider
# Use non-production test accounts only
```

### Test Environment Setup

**CRC Test Environment**:
```bash
# Start CRC with test configuration
crc start --cpus 8 --memory 16384

# Deploy test infrastructure
oc apply -f test-configs/

# Deploy authentication plugin
oc apply -f deploy/
```

**Cleanup Test Environment**:
```bash
# Remove test deployments
oc delete -f deploy/
oc delete -f test-configs/

# Reset CRC (if needed)
crc stop && crc delete
```

## Test Automation

### CI/CD Integration

**GitHub Actions Example**:
```yaml
name: Integration Tests
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    
    - name: Setup CRC
      run: |
        # Download and setup CRC
        # Start cluster
        
    - name: Deploy Test Infrastructure
      run: |
        oc apply -f test-configs/
        oc apply -f deploy/
        
    - name: Run Integration Tests
      run: |
        cd tests/integration/
        python test-auth-flow.py --headless
```

### Test Reporting

**Integration Test Output**:
```
🧪 Starting BYOIDC WASM Plugin Authentication Flow Test
============================================================
🚀 Setting up chrome driver (headless=True)
📱 Test 1: Initial Gateway Access
✅ Test 1 PASSED: Initial Gateway Access - Redirected to login
📱 Test 2: OAuth Login Form  
✅ Test 2 PASSED: OAuth Login Form - Authentication successful
📱 Test 3: Post-Login Redirect
✅ Test 3 PASSED: Post-Login Redirect - Access granted
============================================================
🎉 ALL TESTS PASSED (3/3)
Test completed in 45.2 seconds
```

**JUnit XML Output** (for CI systems):
```bash
# Generate JUnit-compatible test results
python test-auth-flow.py --junit-xml results.xml
```

## Performance Testing

### Latency Testing

**Request Latency Measurement**:
```bash
# Measure authentication overhead
time curl -k https://your-gateway.example.com/

# Compare with direct service access (bypass auth)
time curl -k http://echo-service.echo-service.svc.cluster.local/
```

**WASM Plugin Overhead**:
```bash
# Test with WASM plugin enabled
hey -n 100 -c 1 https://your-gateway.example.com/

# Test with WASM plugin disabled  
oc delete wasmplugin -n openshift-ingress gateway-auth-wasm-plugin
hey -n 100 -c 1 https://your-gateway.example.com/
```

### Resource Usage Testing

**Memory Usage**:
```bash
# Monitor gateway pod memory usage
oc top pods -n openshift-ingress --containers | grep router

# Memory usage over time
watch 'oc top pods -n openshift-ingress --containers'
```

**CPU Usage**:
```bash
# Monitor CPU usage during load test
oc top pods -n openshift-ingress --containers &
hey -n 10000 -c 50 https://your-gateway.example.com/
```

## Regression Testing

### Test Suite Execution

**Full Regression Test**:
```bash
#!/bin/bash
set -e

echo "Running full regression test suite..."

# 1. Unit tests
echo "1/4: Running unit tests..."
cargo test

# 2. Configuration validation  
echo "2/4: Testing configuration validation..."
oc apply -f deploy/wasmplugin-production.yaml
oc get wasmplugin -n openshift-ingress

# 3. Integration tests
echo "3/4: Running integration tests..."
cd tests/integration/
python test-auth-flow.py --username developer --password developer

# 4. Performance baseline
echo "4/4: Running performance baseline..."
hey -n 100 -c 5 -t 30 https://odh-gateway.apps-crc.testing/ > /tmp/perf-results.txt

echo "All regression tests passed ✅"
```

### Compatibility Testing

**OpenShift Version Matrix**:
- OpenShift 4.18 + Service Mesh 2.x
- OpenShift 4.19 + Service Mesh 2.x (primary)  
- OpenShift 4.20 + Service Mesh 2.x (future)

**Browser Compatibility**:
- Chrome/Chromium (primary)
- Firefox
- Edge/Safari (best effort)

## Debugging Test Failures

### Common Test Issues

#### Browser Test Failures
```bash
# Check if gateway is accessible
curl -k -I https://odh-gateway.apps-crc.testing/

# Check auth service status
oc get pods -n openshift-ingress | grep kube-auth-proxy

# Check WASM plugin status
oc get wasmplugin -n openshift-ingress
```

#### Integration Test Timeouts
```bash
# Increase timeout in test configuration
TEST_TIMEOUT = 60  # seconds

# Check for DNS resolution issues  
nslookup odh-gateway.apps-crc.testing
```

### Test Debug Mode

**Enable Debug Logging**:
```python
# In test-auth-flow.py
import logging
logging.basicConfig(level=logging.DEBUG)

# Run with verbose output
python test-auth-flow.py --verbose
```

**Browser Debug Mode**:
```bash
# Run with visible browser for debugging
python test-auth-flow.py --no-headless --debug
```

## Test Environment Management

### Test Isolation

**Namespace Isolation**:
```bash
# Create isolated test namespace
oc create namespace test-env-$(date +%s)

# Deploy to test namespace
oc apply -f test-configs/ -n test-env-123456
```

**Resource Cleanup**:
```bash
# Automatic cleanup after tests
cleanup() {
  oc delete namespace test-env-$TEST_ID
}
trap cleanup EXIT
```

### Test Data Cleanup

**Session Cleanup**:
```bash
# Clear browser cache and cookies
rm -rf /tmp/chrome_test_profile/

# Clear curl cookie files
rm -f /tmp/cookies.txt /tmp/session.txt
```

*For test contribution guidelines, see [DEVELOPMENT.md](DEVELOPMENT.md).*
