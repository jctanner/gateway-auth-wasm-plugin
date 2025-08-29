# BYOIDC WASM Plugin Tests

This directory contains test suites for the BYOIDC WASM Plugin project.

## Directory Structure

```
tests/
├── integration/          # End-to-end integration tests
│   ├── test-auth-flow.py    # Browser-based OAuth flow testing
│   ├── requirements.txt     # Python dependencies for integration tests
│   └── __init__.py
├── unit/                # Unit tests (future)
└── README.md           # This file
```

## Integration Tests

### Prerequisites

1. **OpenShift Cluster**: Running OpenShift cluster with Gateway API support
2. **Deployed Services**: All components from `test-configs/` deployed
3. **WASM Plugin**: Deployed from `deploy/wasmplugin-production.yaml`

### Running Integration Tests

```bash
# From project root
cd tests/integration/

# Create virtual environment (first time only)
python3 -m venv venv
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt

# Run browser-based OAuth flow test
python test-auth-flow.py --username developer --password developer --browser chrome

# Run in visible mode (non-headless)
python test-auth-flow.py --username developer --password developer --browser chrome --no-headless
```

### Test Scenarios

The integration test validates:

1. ✅ **Initial Gateway Access**: Redirected to OAuth login
2. ✅ **OAuth Login Form**: User authentication successful  
3. ✅ **Post-Login Access**: Session cookies validated, access granted

### Expected Output

```
🧪 Starting BYOIDC WASM Plugin Authentication Flow Test
============================================================
✅ Test 1 PASSED: Initial Gateway Access - Redirected to login
✅ Test 2 PASSED: OAuth Login Form - Authentication successful
✅ Test 3 PASSED: Post-Login Redirect - Access granted
============================================================
🎉 ALL TESTS PASSED (3/3)
```

## Unit Tests (Future)

Unit tests will be added to `tests/unit/` for individual component testing.

## Test Environment

The tests expect the following services to be running:
- `odh-gateway` (OpenShift Gateway)
- `kube-auth-proxy` (OAuth2 proxy)
- `echo-service` (Protected test service)
- BYOIDC WASM Plugin (authentication filter)
