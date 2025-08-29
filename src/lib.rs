mod config;
mod headers;
mod http_client;
mod responses;
mod metrics;

use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use log::{info, debug, error, warn};
use std::time::Duration;

use config::PluginConfig;
use http_client::HttpClient;
use responses::{ResponseHandler, AuthAction};

// Root Context - Plugin initialization and configuration
pub struct AuthProxyRoot {
    config: Option<PluginConfig>,
}

impl AuthProxyRoot {
    fn new() -> Self {
        Self { config: None }
    }
}

impl Context for AuthProxyRoot {}

impl RootContext for AuthProxyRoot {
    fn on_configure(&mut self, plugin_configuration_size: usize) -> bool {
        if plugin_configuration_size == 0 {
            warn!("No plugin configuration provided, using defaults");
            self.config = Some(PluginConfig::default());
            return true;
        }

        match self.get_plugin_configuration() {
            Some(config_bytes) => {
                            match serde_json::from_slice::<PluginConfig>(&config_bytes) {
                Ok(config) => {
                    info!("✅ BYOIDC Plugin configured successfully");
                    info!("   Auth service endpoint: {}", config.auth_service.endpoint);
                    info!("   Auth service cluster: {}", config.auth_service.cluster);
                    info!("   Auth service verify_path: {}", config.auth_service.verify_path);
                    info!("   Auth service timeout: {} ms", config.auth_service.timeout);
                    info!("   Global auth enabled: {}", config.global_auth.enabled);
                    self.config = Some(config);
                    true
                }
                Err(e) => {
                    error!("❌ Failed to parse plugin configuration: {}", e);
                    error!("   Raw config bytes (first 200 chars): {}", 
                           String::from_utf8_lossy(&config_bytes[..std::cmp::min(200, config_bytes.len())]));
                    false
                }
            }
            }
            None => {
                error!("Plugin configuration is empty");
                false
            }
        }
    }

    fn create_http_context(&self, context_id: u32) -> Option<Box<dyn HttpContext>> {
        debug!("Creating HTTP context {}", context_id);
        match &self.config {
            Some(config) => Some(Box::new(AuthProxy::new(config.clone()))),
            None => {
                error!("Cannot create HTTP context: plugin not configured");
                None
            }
        }
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

// HTTP Context - Request processing
pub struct AuthProxy {
    config: PluginConfig,
    call_id: Option<u32>,
    http_client: HttpClient,
    response_handler: ResponseHandler,
}

impl AuthProxy {
    fn new(config: PluginConfig) -> Self {
        Self {
            config,
            call_id: None,
            http_client: HttpClient::new(),
            response_handler: ResponseHandler::new(),
        }
    }

            fn is_auth_request(&self) -> bool {
            if let Some(path) = self.get_http_request_header(":path") {
                // Skip authentication for ALL OAuth-related paths
                path.starts_with("/oauth2/") || path.starts_with(&self.config.auth_service.verify_path)
            } else {
                false
            }
        }

    fn extract_authorization_header(&self) -> Option<String> {
        self.get_http_request_header("authorization")
    }
}

impl Context for AuthProxy {
    fn on_http_call_response(
        &mut self,
        token_id: u32,
        num_headers: usize,
        body_size: usize,
        num_trailers: usize,
    ) {
        debug!("Received HTTP call response: token_id={}, headers={}, body_size={}, trailers={}", 
               token_id, num_headers, body_size, num_trailers);
        
        if Some(token_id) != self.call_id {
            warn!("Unexpected token_id: expected {:?}, got {}", self.call_id, token_id);
            return;
        }

        // Get response status
        let status = match self.get_http_call_response_header(":status") {
            Some(status_str) => status_str.parse::<u16>().unwrap_or(500),
            None => {
                error!("No status header in auth response");
                self.send_http_response(500, vec![("content-type", "text/plain")], Some(b"Internal Server Error"));
                return;
            }
        };

        // Convert status to string and handle response
        let status_str = status.to_string();
        let auth_action = self.response_handler.handle_auth_response(&status_str);
        
        // Process the auth action  
        match auth_action {
            AuthAction::Allow => {
                debug!("Authentication successful (202), continuing request to upstream");
                self.resume_http_request();
            }
            AuthAction::Deny(status_code, message) => {
                debug!("Authentication denied: {} - {}", status_code, message);
                self.send_http_response(status_code as u32, vec![("content-type", "application/json")], Some(message.as_bytes()));
            }
            AuthAction::Redirect(url) => {
                info!("Authentication needed - redirecting to OAuth start");
                
                // Build the OAuth start URL using the original request host
                let redirect_url = if url.starts_with("/oauth2/start") {
                    // Relative URL - construct full OAuth start URL
                    let original_host = self.get_http_request_header(":authority")
                        .unwrap_or("odh-gateway.apps-crc.testing".to_string());
                    format!("https://{}/oauth2/start", original_host)
                } else if url == "sign-in-page" {
                    // Handle 403 response from kube-auth-proxy - forward the location header
                    self.get_http_call_response_header("location").unwrap_or("/oauth2/start".to_string())
                } else {
                    // Direct URL from kube-auth-proxy Location header
                    url
                };
                
                debug!("Redirecting client to OAuth start: {}", redirect_url);
                self.send_http_response(302, vec![("location", &redirect_url), ("content-type", "text/html")], Some(b"<html><body>Redirecting to authentication...</body></html>"));
            }
            AuthAction::Error(error) => {
                error!("Auth service error: {}", error);
                self.send_http_response(503, vec![("content-type", "text/plain")], Some(b"Authentication service unavailable"));
            }
        }
    }
}

impl HttpContext for AuthProxy {
            fn on_http_request_headers(&mut self, num_headers: usize, end_of_stream: bool) -> Action {
            let method = self.get_http_request_header(":method").unwrap_or("UNKNOWN".to_string());
            let path = self.get_http_request_header(":path").unwrap_or("UNKNOWN".to_string());
            let authority = self.get_http_request_header(":authority").unwrap_or("UNKNOWN".to_string());
            
            info!("🌐 Incoming request: {} {} (authority: {}, headers: {}, end_of_stream: {})", 
                  method, path, authority, num_headers, end_of_stream);

            // Skip auth for requests to the auth service itself
            if self.is_auth_request() {
                info!("⏭️  Skipping auth for auth service request: {}", path);
                return Action::Continue;
            }

            // Forward ALL requests to kube-auth-proxy for authentication decisions
            info!("🔐 Forwarding request to kube-auth-proxy for authentication check");
        
        // Parse the auth service endpoint
        let (scheme, host) = match self.http_client.parse_endpoint(&self.config.auth_service.endpoint) {
            Ok(parsed) => parsed,
            Err(e) => {
                error!("Failed to parse auth service endpoint: {}", e);
                self.send_http_response(503, vec![("content-type", "text/plain")], Some(b"Service Configuration Error"));
                return Action::Pause;
            }
        };
        
        // Get original request details to forward to kube-auth-proxy
        let original_method = self.get_http_request_header(":method").unwrap_or("GET".to_string());
        let original_path = self.get_http_request_header(":path").unwrap_or("/".to_string());
        let original_authority = self.get_http_request_header(":authority").unwrap_or("unknown".to_string());
        let auth_header = self.extract_authorization_header();
        let cookie_header = self.get_http_request_header("cookie");
        
        // Build headers for auth check call - include original request info
        let mut auth_headers = vec![
            (":method", "GET"),
            (":path", &self.config.auth_service.verify_path),
            (":authority", &host),
            (":scheme", &scheme),
            ("user-agent", "BYOIDC-WASM-Plugin/1.0"),
            // Forward original request details for kube-auth-proxy context
            ("x-forwarded-method", &original_method),
            ("x-forwarded-uri", &original_path),
            ("x-forwarded-host", &original_authority),
        ];
        
        // Forward authorization header if present
        if let Some(ref auth_value) = auth_header {
            auth_headers.push(("authorization", auth_value));
        }
        
        // Forward cookie header if present (CRITICAL for session-based auth!)
        if let Some(ref cookie_value) = cookie_header {
            auth_headers.push(("cookie", cookie_value));
            debug!("Forwarding cookies to kube-auth-proxy: {}", cookie_value);
        }
        
                    // Debug log all dispatch parameters before calling
            info!("=== DISPATCH DEBUG INFO ===");
            info!("Cluster: {}", &self.config.auth_service.cluster);
            info!("Headers count: {}", auth_headers.len());
            for (i, (key, value)) in auth_headers.iter().enumerate() {
                info!("  Header[{}]: {} = {}", i, key, value);
            }
            info!("Timeout: {} ms", self.config.auth_service.timeout);
            info!("==========================");

            // Clone headers for error logging (since dispatch_http_call moves them)
            let headers_debug: Vec<(String, String)> = auth_headers.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            // Dispatch HTTP call to kube-auth-proxy for authentication check
            match self.dispatch_http_call(
                &self.config.auth_service.cluster,
                auth_headers,
                None, // No body for GET request
                vec![], // No trailers
                Duration::from_millis(self.config.auth_service.timeout)
            ) {
                Ok(call_id) => {
                    info!("✅ Auth check dispatched successfully to kube-auth-proxy with call ID: {}", call_id);
                    self.call_id = Some(call_id);
                    Action::Pause
                }
                Err(e) => {
                    error!("❌ Failed to dispatch auth call to cluster '{}': {:?}", &self.config.auth_service.cluster, e);
                    error!("   Headers that were sent:");
                    for (i, (key, value)) in headers_debug.iter().enumerate() {
                        error!("     Header[{}]: {} = {}", i, key, value);
                    }
                    self.send_http_response(503, vec![("content-type", "text/plain")], Some(b"Authentication Service Unavailable"));
                    Action::Pause
                }
            }
    }
}

// WASM plugin entry point
proxy_wasm::main! {{
    proxy_wasm::set_log_level(proxy_wasm::types::LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(AuthProxyRoot::new())
    });
}}