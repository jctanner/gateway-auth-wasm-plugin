use serde::{Deserialize, Serialize};

/// Main plugin configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub auth_service: AuthServiceConfig,
    pub global_auth: GlobalAuthConfig,
    #[serde(default)]
    pub error_responses: Option<ErrorResponses>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            auth_service: AuthServiceConfig::default(),
            global_auth: GlobalAuthConfig::default(),
            error_responses: None,
        }
    }
}

/// Configuration for the kube-auth-proxy service connection
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthServiceConfig {
    /// kube-auth-proxy service endpoint (e.g., "https://kube-auth-proxy.auth-system.svc.cluster.local:4180")
    pub endpoint: String,
    /// Envoy cluster name for the auth service (e.g., "outbound|4180||kube-auth-proxy.auth-system.svc.cluster.local")
    pub cluster: String,
    /// Auth verification path (typically "/auth")
    pub verify_path: String,
    /// Request timeout in milliseconds
    pub timeout: u64,
    /// TLS configuration for HTTPS communication
    pub tls: TlsConfig,
}

impl Default for AuthServiceConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180".to_string(),
            cluster: "outbound|4180||kube-auth-proxy.auth-system.svc.cluster.local".to_string(),
            verify_path: "/auth".to_string(),
            timeout: 5000, // 5 seconds
            tls: TlsConfig::default(),
        }
    }
}

/// TLS configuration for secure communication with auth service
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Whether to verify the auth service certificate
    /// Set to false for self-signed certificates (common in Kubernetes)
    pub verify_cert: bool,
    /// Optional path to custom CA certificate bundle
    pub ca_cert_path: Option<String>,
    /// Optional client certificate for mutual TLS
    pub client_cert_path: Option<String>,
    /// Optional client private key for mutual TLS
    pub client_key_path: Option<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            verify_cert: false, // Default to false for Kubernetes serving certificates
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
        }
    }
}

/// Global authentication configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobalAuthConfig {
    /// Whether to enable authentication for all requests
    /// Note: This is path-agnostic - the WASM plugin applies auth to ALL requests
    /// Dynamic HTTPRoute CRs handle routing, WASM handles universal auth
    pub enabled: bool,
}

impl Default for GlobalAuthConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Default to requiring auth for all requests
        }
    }
}

/// Custom error response configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorResponses {
    pub auth_service_error: ErrorResponse,
    pub access_denied: ErrorResponse,
    pub authentication_required: ErrorResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub body: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

impl Default for ErrorResponse {
    fn default() -> Self {
        Self {
            status: 500,
            body: "Internal server error".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
        }
    }
}

impl PluginConfig {
    /// Validate the plugin configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate endpoint URL
        if self.auth_service.endpoint.is_empty() {
            return Err("Auth service endpoint cannot be empty".to_string());
        }

        // Ensure HTTPS is used for security
        if !self.auth_service.endpoint.starts_with("https://") {
            return Err("Auth service endpoint must use HTTPS for security".to_string());
        }

        // Validate timeout
        if self.auth_service.timeout == 0 {
            return Err("Auth service timeout must be greater than 0".to_string());
        }

        if self.auth_service.timeout > 30000 {
            return Err("Auth service timeout should not exceed 30 seconds".to_string());
        }

        // Validate verify path
        if self.auth_service.verify_path.is_empty() {
            return Err("Auth service verify path cannot be empty".to_string());
        }

        if !self.auth_service.verify_path.starts_with('/') {
            return Err("Auth service verify path must start with '/'".to_string());
        }

        Ok(())
    }
}
