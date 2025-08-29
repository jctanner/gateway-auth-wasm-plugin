# Advanced Deployment Guide

## Overview

This document covers advanced deployment scenarios, production considerations, and operational practices for the BYOIDC WASM Plugin.

*Note: For a concrete CRC/OpenShift 4.19 deployment example, see [REFERENCE_DEPLOYMENT.md](REFERENCE_DEPLOYMENT.md).*

## Production Deployment Scenarios

### High Availability Deployment

**Multi-Zone Gateway Deployment**:
```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: production-gateway
  namespace: openshift-ingress
spec:
  gatewayClassName: istio
  listeners:
  - name: https
    hostname: "*.production.example.com"
    port: 443
    protocol: HTTPS
    tls:
      mode: Terminate
      certificateRefs:
      - name: production-tls-cert
  infrastructure:
    annotations:
      # High availability configuration
      service.beta.kubernetes.io/aws-load-balancer-cross-zone-load-balancing-enabled: "true"
      service.beta.kubernetes.io/aws-load-balancer-nlb-target-type: "instance"
```

**Scalable Auth Service**:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
spec:
  replicas: 5  # Scale based on load
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 2
      maxUnavailable: 1
  template:
    spec:
      containers:
      - name: kube-auth-proxy
        image: quay.io/opendatahub-io/kube-auth-proxy:latest
        resources:
          requests:
            memory: "256Mi"
            cpu: "200m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /ping
            port: 4180
          initialDelaySeconds: 30
        readinessProbe:
          httpGet:
            path: /ping  
            port: 4180
          initialDelaySeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
spec:
  selector:
    app: kube-auth-proxy
  ports:
  - port: 4180
    targetPort: 4180
  sessionAffinity: ClientIP  # Important for session-based auth
```

### Multi-Cluster Deployment

**Federated Authentication**:
```yaml
# Cluster A - Primary
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: federated-auth-primary
  namespace: openshift-ingress
spec:
  pluginConfig:
    auth_service:
      endpoint: "https://central-auth.shared-services.svc.cluster.local:4180"
      cluster: "outbound|4180||central-auth.shared-services.svc.cluster.local"

# Cluster B - Secondary  
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: federated-auth-secondary
  namespace: openshift-ingress
spec:
  pluginConfig:
    auth_service:
      endpoint: "https://central-auth.primary.example.com:443"  # External endpoint
      cluster: "central-auth-external"
```

## Security Hardening

### TLS Configuration

**Production TLS Setup**:
```yaml
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    tls:
      verify_cert: true
      ca_cert: "/etc/ssl/certs/ca-bundle.crt"
      # Optional: Client certificate for mTLS
      client_cert: "/etc/ssl/client/tls.crt"  
      client_key: "/etc/ssl/client/tls.key"
```

**Certificate Management**:
```yaml
# OpenShift service-ca integration
apiVersion: v1
kind: Service
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
  annotations:
    service.beta.openshift.io/serving-cert-secret-name: kube-auth-proxy-tls
spec:
  ports:
  - port: 4180
    targetPort: 4180
---
# Mount certificates in deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kube-auth-proxy
spec:
  template:
    spec:
      containers:
      - name: kube-auth-proxy
        args:
        - --https-address=0.0.0.0:4180
        - --tls-cert-file=/etc/tls/tls.crt
        - --tls-key-file=/etc/tls/tls.key
        volumeMounts:
        - name: tls-certs
          mountPath: /etc/tls
          readOnly: true
      volumes:
      - name: tls-certs
        secret:
          secretName: kube-auth-proxy-tls
```

### Network Security

**NetworkPolicy Configuration**:
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
  - Ingress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: openshift-ingress
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: kube-auth-proxy
    ports:
    - protocol: TCP
      port: 4180
  - to: []  # Allow DNS resolution
    ports:
    - protocol: UDP
      port: 53
```

### RBAC Configuration

**Minimal RBAC**:
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
  annotations:
    serviceaccounts.openshift.io/oauth-redirecturi.primary: "https://production.example.com/oauth2/callback"
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
rules:
- apiGroups: [""]
  resources: ["services", "endpoints"]
  verbs: ["get", "list"]
- apiGroups: [""]
  resources: ["secrets"]
  resourceNames: ["kube-auth-proxy-secret"]
  verbs: ["get"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
subjects:
- kind: ServiceAccount
  name: kube-auth-proxy
  namespace: openshift-ingress
roleRef:
  kind: Role
  name: kube-auth-proxy
  apiGroup: rbac.authorization.k8s.io
```

## Monitoring and Observability

### Metrics Collection

**WASM Plugin Metrics** (Future):
```yaml
# TODO: Implement metrics in src/metrics.rs
pluginConfig:
  metrics:
    enabled: true
    endpoint: "/metrics"
    labels:
      - service
      - version
      - cluster
```

**Prometheus ServiceMonitor**:
```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: kube-auth-proxy
  namespace: openshift-ingress
spec:
  selector:
    matchLabels:
      app: kube-auth-proxy
  endpoints:
  - port: metrics
    interval: 30s
    path: /metrics
```

### Logging Configuration

**Structured Logging**:
```yaml
pluginConfig:
  debug:
    enabled: false  # Disable in production
    log_level: "info"
    log_headers: false  # Security: Never log sensitive headers
```

**Log Aggregation**:
```yaml
# FluentBit configuration for WASM plugin logs
apiVersion: v1
kind: ConfigMap
metadata:
  name: fluent-bit-config
data:
  fluent-bit.conf: |
    [INPUT]
        Name tail
        Path /var/log/containers/*router-default*.log
        Tag wasm.plugin.*
        Parser docker
    
    [FILTER]
        Name grep
        Match wasm.plugin.*
        Regex log wasm|auth
    
    [OUTPUT]
        Name elasticsearch
        Match wasm.plugin.*
        Host elasticsearch.logging.svc.cluster.local
        Port 9200
        Index wasm-plugin-logs
```

### Health Checks

**WASM Plugin Health**:
```bash
# Check plugin loading status
oc logs -n openshift-ingress deployment/router-default | grep -i wasm

# Check configuration parsing
oc get wasmplugin -n openshift-ingress -o yaml
```

**Auth Service Health**:
```yaml
# Health check configuration
containers:
- name: kube-auth-proxy
  livenessProbe:
    httpGet:
      path: /ping
      port: 4180
      scheme: HTTP
    initialDelaySeconds: 30
    periodSeconds: 10
  readinessProbe:
    httpGet:
      path: /ping
      port: 4180
      scheme: HTTP
    initialDelaySeconds: 5
    periodSeconds: 5
```

## Performance Optimization

### Resource Allocation

**Gateway Pod Resources**:
```yaml
# Update router deployment with appropriate resources
spec:
  template:
    spec:
      containers:
      - name: router
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
```

**Auth Service Tuning**:
```yaml
containers:
- name: kube-auth-proxy
  args:
  - --upstream=static://200
  - --cookie-expire=24h  # Reduce auth frequency
  - --cookie-refresh=1h  # Background session refresh
  resources:
    requests:
      memory: "256Mi"
      cpu: "200m"
    limits:
      memory: "512Mi"
      cpu: "500m"
```

### Caching Strategies

**Session Caching**:
```yaml
# Configure session storage backend
args:
- --session-store-type=redis
- --redis-connection-url=redis://redis.cache.svc.cluster.local:6379
```

**DNS Caching**:
```yaml
# Configure DNS caching in gateway pods
spec:
  template:
    spec:
      dnsConfig:
        options:
        - name: ndots
          value: "2"
        - name: cache
          value: "true"
```

## Disaster Recovery

### Backup Procedures

**Configuration Backup**:
```bash
#!/bin/bash
# Backup all WASM plugin configurations
oc get wasmplugin -n openshift-ingress -o yaml > wasm-plugin-backup.yaml
oc get httproute -n echo-service -o yaml > httproute-backup.yaml
oc get referencegrant -n openshift-ingress -o yaml > referencegrant-backup.yaml
```

**Secret Backup**:
```bash
# Backup authentication secrets (encrypted)
oc get secret -n openshift-ingress kube-auth-proxy-secret -o yaml | \
  kubeseal -o yaml > kube-auth-proxy-sealed-secret.yaml
```

### Recovery Procedures

**Service Recovery**:
```bash
#!/bin/bash
# Disaster recovery script
set -e

echo "Starting disaster recovery..."

# 1. Restore infrastructure
oc apply -f test-configs/

# 2. Restore authentication configuration
oc apply -f wasm-plugin-backup.yaml
oc apply -f httproute-backup.yaml  
oc apply -f referencegrant-backup.yaml

# 3. Verify service health
oc wait --for=condition=Ready pod -l app=kube-auth-proxy -n openshift-ingress --timeout=300s

# 4. Test authentication flow
curl -k -I https://production.example.com/

echo "Disaster recovery completed"
```

## CI/CD Integration

### GitOps Deployment

**ArgoCD Application**:
```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: byoidc-wasm-plugin
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/your-org/gateway-auth-wasm-plugin
    targetRevision: main
    path: deploy/
  destination:
    server: https://kubernetes.default.svc
    namespace: openshift-ingress
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
    - CreateNamespace=true
```

### Deployment Pipeline

**GitHub Actions Workflow**:
```yaml
name: Production Deployment
on:
  release:
    types: [published]

jobs:
  deploy-production:
    runs-on: ubuntu-latest
    environment: production
    steps:
    - uses: actions/checkout@v3
    
    - name: Build and Push Image
      run: |
        make image TAG=${{ github.event.release.tag_name }}
        make push TAG=${{ github.event.release.tag_name }}
    
    - name: Update Deployment Manifest
      run: |
        sed -i 's|:latest|:${{ github.event.release.tag_name }}|g' deploy/wasmplugin-production.yaml
    
    - name: Deploy to Production
      run: |
        oc login --token=${{ secrets.OPENSHIFT_TOKEN }} --server=${{ secrets.OPENSHIFT_SERVER }}
        oc apply -f deploy/
        
    - name: Verify Deployment
      run: |
        oc wait --for=condition=Ready pod -l app=router-default -n openshift-ingress --timeout=300s
        cd tests/integration/
        python test-auth-flow.py --username ${{ secrets.TEST_USER }} --password ${{ secrets.TEST_PASS }}
```

## Environment-Specific Configurations

### Development Environment

```yaml
# Development WasmPlugin configuration
pluginConfig:
  auth_service:
    endpoint: "http://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    timeout: 30000  # Longer timeout for debugging
    tls:
      verify_cert: false
  debug:
    enabled: true
    log_level: "debug"
    log_headers: true  # OK for development
```

### Staging Environment

```yaml
# Staging WasmPlugin configuration  
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    timeout: 10000
    tls:
      verify_cert: true
  debug:
    enabled: false
    log_level: "info"
  # Test error responses in staging
  error_responses:
    auth_service_error:
      status: 503
      body: '{"error": "staging_auth_service_unavailable"}'
```

### Production Environment

```yaml
# Production WasmPlugin configuration
pluginConfig:
  auth_service:
    endpoint: "https://kube-auth-proxy.openshift-ingress.svc.cluster.local:4180"
    timeout: 5000
    tls:
      verify_cert: true
      ca_cert: "/etc/ssl/certs/ca-bundle.crt"
  global_auth:
    enabled: true
    skip_paths:
      - "/health"
      - "/readiness"
  debug:
    enabled: false
    log_level: "warn"  # Minimal logging in production
```

## Migration Strategies

### Blue-Green Deployment

```bash
#!/bin/bash
# Blue-green deployment script

# Deploy new version (green)
oc apply -f deploy/wasmplugin-green.yaml

# Wait for health checks
sleep 30

# Run smoke tests against green
cd tests/integration/
python test-auth-flow.py --endpoint https://green.production.example.com/

# Switch traffic to green
oc patch httproute production-route --patch '{"spec": {"rules": [{"backendRefs": [{"name": "green-service"}]}]}}'

# Monitor for issues
sleep 300

# Cleanup blue deployment
oc delete -f deploy/wasmplugin-blue.yaml
```

### Canary Deployment

```yaml
# Canary traffic splitting
apiVersion: gateway.networking.k8s.io/v1
kind: HTTPRoute
metadata:
  name: canary-route
spec:
  rules:
  - matches:
    - headers:
      - name: X-Canary-User
        value: "true"
    backendRefs:
    - name: canary-service
      port: 80
  - backendRefs:
    - name: stable-service
      port: 80
      weight: 90
    - name: canary-service
      port: 80
      weight: 10
```

*For additional deployment scenarios and troubleshooting, see [TROUBLESHOOTING.md](TROUBLESHOOTING.md) and [INTEGRATION.md](INTEGRATION.md).*
