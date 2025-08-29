# BYOIDC WASM Plugin - Container-based Build System
# All operations use containers to avoid host toolchain dependencies

# Configuration
PLUGIN_NAME := gateway-auth-wasm-plugin
VERSION ?= latest
REGISTRY ?= registry.tannerjc.net
IMAGE_NAME := $(REGISTRY)/$(PLUGIN_NAME)
RUST_IMAGE := rustlang/rust:nightly

# Docker build arguments
DOCKER_BUILDKIT := 1
SELINUX_MOUNT := :z

# Default target
.DEFAULT_GOAL := help

##@ General

.PHONY: help
help: ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Development

.PHONY: check
check: ## Check code compilation (fast)
	@echo "🔍 Checking code compilation..."
	@docker run --rm \
		-v $(shell pwd):/app$(SELINUX_MOUNT) \
		-w /app \
		$(RUST_IMAGE) \
		sh -c "rustup target add wasm32-unknown-unknown && cargo check"

.PHONY: test
test: ## Run unit tests
	@echo "🧪 Running unit tests..."
	@docker run --rm \
		-v $(shell pwd):/app$(SELINUX_MOUNT) \
		-w /app \
		$(RUST_IMAGE) \
		cargo test

.PHONY: build-wasm
build-wasm: ## Build WASM binary directly
	@echo "🔨 Building WASM binary..."
	@docker run --rm \
		-v $(shell pwd):/app$(SELINUX_MOUNT) \
		-w /app \
		$(RUST_IMAGE) \
		sh -c "rustup target add wasm32-unknown-unknown && cargo install wasm-pack && wasm-pack build --target web --out-dir pkg"
	@echo "✅ WASM binary built with wasm-pack"

.PHONY: build-wasm-cargo
build-wasm-cargo: ## Build WASM binary with cargo (fallback)
	@echo "🔨 Building WASM binary with cargo..."
	@docker run --rm \
		-v $(shell pwd):/app$(SELINUX_MOUNT) \
		-w /app \
		$(RUST_IMAGE) \
		sh -c "rustup target add wasm32-unknown-unknown && cargo build --target wasm32-unknown-unknown --release"
	@echo "✅ WASM binary built: target/wasm32-unknown-unknown/release/$(PLUGIN_NAME).wasm"

.PHONY: dev-shell
dev-shell: ## Start interactive development container
	@echo "🐚 Starting development shell..."
	@docker run -it --rm \
		-v $(shell pwd):/workspace$(SELINUX_MOUNT) \
		-w /workspace \
		$(RUST_IMAGE) \
		bash

##@ Docker Images

.PHONY: image
image: ## Build production OCI image
	@echo "📦 Building production OCI image..."
	@DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) docker build \
		--target runtime \
		--tag $(IMAGE_NAME):$(VERSION) \
		--tag $(IMAGE_NAME):latest \
		.
	@echo "✅ Built image: $(IMAGE_NAME):$(VERSION)"

.PHONY: image-dev
image-dev: ## Build development image
	@echo "📦 Building development image..."
	@DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) docker build \
		--target development \
		--tag $(IMAGE_NAME):dev \
		.
	@echo "✅ Built development image: $(IMAGE_NAME):dev"

.PHONY: image-test
image-test: ## Build testing image with debugging tools
	@echo "📦 Building testing image..."
	@DOCKER_BUILDKIT=$(DOCKER_BUILDKIT) docker build \
		--target testing \
		--tag $(IMAGE_NAME):test \
		.
	@echo "✅ Built testing image: $(IMAGE_NAME):test"

.PHONY: image-all
image-all: image image-dev image-test ## Build all image variants

##@ Testing

.PHONY: test-image
test-image: image-test ## Test the built WASM binary
	@echo "🔍 Testing WASM binary in container..."
	@docker run --rm $(IMAGE_NAME):test

.PHONY: inspect-wasm
inspect-wasm: build-wasm ## Inspect WASM binary properties
	@echo "🔍 Inspecting WASM binary..."
	@docker run --rm \
		-v $(shell pwd)/target/wasm32-unknown-unknown/release$(SELINUX_MOUNT):/wasm:ro \
		alpine:latest \
		sh -c "apk add --no-cache file && file /wasm/$(PLUGIN_NAME).wasm && ls -la /wasm/$(PLUGIN_NAME).wasm"

##@ Registry

.PHONY: push
push: image ## Push image to registry
	@echo "📤 Pushing image to registry..."
	@docker push $(IMAGE_NAME):$(VERSION)
	@docker push $(IMAGE_NAME):latest
	@echo "✅ Pushed $(IMAGE_NAME):$(VERSION)"

.PHONY: pull
pull: ## Pull image from registry
	@echo "📥 Pulling image from registry..."
	@docker pull $(IMAGE_NAME):$(VERSION)

##@ Deployment

.PHONY: deploy
deploy: ## Deploy WASM plugin to cluster (requires oc login)
	@echo "🚀 Deploying WASM plugin to cluster..."
	@oc apply -f deploy/wasmplugin-production.yaml
	@echo "✅ WASM plugin deployed"

.PHONY: undeploy
undeploy: ## Remove WASM plugin from cluster
	@echo "🗑️  Removing WASM plugin from cluster..."
	@oc delete -f deploy/wasmplugin-production.yaml --ignore-not-found=true
	@echo "✅ WASM plugin removed"

.PHONY: status
status: ## Check WASM plugin deployment status
	@echo "📊 WASM Plugin Deployment Status:"
	@echo ""
	@echo "🔍 WasmPlugin Resources:"
	@oc get wasmplugin -n openshift-ingress 2>/dev/null || echo "No WasmPlugin resources found"
	@echo ""
	@echo "🔍 Gateway Pods:"
	@oc get pods -n openshift-ingress -l gateway.networking.k8s.io/gateway-name=odh-gateway -o wide 2>/dev/null || echo "No gateway pods found"
	@echo ""
	@echo "🔍 Auth Service:"
	@oc get pods -n auth-system -l app=kube-auth-proxy 2>/dev/null || echo "No auth service pods found"

.PHONY: logs
logs: ## View gateway logs (useful for debugging)
	@echo "📋 Gateway Pod Logs (last 50 lines):"
	@oc logs -n openshift-ingress -l gateway.networking.k8s.io/gateway-name=odh-gateway --tail=50 || echo "Could not retrieve gateway logs"

.PHONY: generate-manifest
generate-manifest: ## Generate WasmPlugin manifest
	@echo "📄 Generating WasmPlugin manifest..."
	@mkdir -p deploy
	@echo 'apiVersion: extensions.istio.io/v1alpha1' > deploy/wasmplugin.yaml
	@echo 'kind: WasmPlugin' >> deploy/wasmplugin.yaml
	@echo 'metadata:' >> deploy/wasmplugin.yaml
	@echo '  name: $(PLUGIN_NAME)' >> deploy/wasmplugin.yaml
	@echo '  namespace: istio-system' >> deploy/wasmplugin.yaml
	@echo 'spec:' >> deploy/wasmplugin.yaml
	@echo '  selector:' >> deploy/wasmplugin.yaml
	@echo '    matchLabels:' >> deploy/wasmplugin.yaml
	@echo '      istio: ingressgateway' >> deploy/wasmplugin.yaml
	@echo '  phase: AUTHN' >> deploy/wasmplugin.yaml
	@echo '  priority: 1000' >> deploy/wasmplugin.yaml
	@echo '  url: oci://$(IMAGE_NAME):$(VERSION)' >> deploy/wasmplugin.yaml
	@echo '  pluginConfig:' >> deploy/wasmplugin.yaml
	@echo '    auth_service:' >> deploy/wasmplugin.yaml
	@echo '      endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180"' >> deploy/wasmplugin.yaml
	@echo '      verify_path: "/auth"' >> deploy/wasmplugin.yaml
	@echo '      timeout: 5000' >> deploy/wasmplugin.yaml
	@echo '      tls:' >> deploy/wasmplugin.yaml
	@echo '        verify_cert: false' >> deploy/wasmplugin.yaml
	@echo '    global_auth:' >> deploy/wasmplugin.yaml
	@echo '      enabled: true' >> deploy/wasmplugin.yaml
	@echo "✅ Generated deploy/wasmplugin.yaml"

##@ Cleanup

.PHONY: clean
clean: ## Clean build artifacts
	@echo "🧹 Cleaning build artifacts..."
	@rm -rf target/
	@docker image prune -f --filter label=stage=builder
	@echo "✅ Cleaned build artifacts"

.PHONY: clean-images
clean-images: ## Remove all built images
	@echo "🧹 Cleaning Docker images..."
	@docker rmi $(IMAGE_NAME):$(VERSION) $(IMAGE_NAME):latest $(IMAGE_NAME):dev $(IMAGE_NAME):test 2>/dev/null || true
	@echo "✅ Cleaned Docker images"

##@ Quick Commands

.PHONY: quick-build
quick-build: check build-wasm ## Quick build: check + WASM binary

.PHONY: quick-test
quick-test: test image test-image ## Quick test: unit tests + image test

.PHONY: release
release: clean test image test-image push generate-manifest ## Full release pipeline

##@ Information

.PHONY: info
info: ## Show build information
	@echo "📊 Build Information:"
	@echo "  Plugin Name: $(PLUGIN_NAME)"
	@echo "  Version: $(VERSION)"
	@echo "  Registry: $(REGISTRY)" 
	@echo "  Image: $(IMAGE_NAME):$(VERSION)"
	@echo "  Rust Image: $(RUST_IMAGE)"
	@echo ""
	@echo "🏗️  Available Targets:"
	@echo "  Development: make dev-shell, make check, make build-wasm"
	@echo "  Images: make image, make image-dev, make image-test"
	@echo "  Testing: make test, make test-image, make inspect-wasm"
	@echo "  Deployment: make deploy, make undeploy, make status, make logs"
	@echo "  Registry: make push, make pull, make generate-manifest"
	@echo "  Quick: make quick-build, make quick-test, make release"

.PHONY: versions
versions: ## Show tool versions
	@echo "🔧 Tool Versions:"
	@echo "Docker:"
	@docker --version
	@echo ""
	@echo "Rust (in container):"
	@docker run --rm $(RUST_IMAGE) rustc --version
	@echo ""
	@echo "Cargo (in container):"  
	@docker run --rm $(RUST_IMAGE) cargo --version
