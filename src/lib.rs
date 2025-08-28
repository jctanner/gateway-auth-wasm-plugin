use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use std::time::Duration;
use log::{info, warn, error};

mod config;
mod http_client;
mod headers;
mod responses;
mod metrics;

use config::PluginConfig;
use http_client::HttpClient;
use headers::HeaderProcessor;
use responses::ResponseHandler;

/// Main WASM plugin struct that handles authentication requests
pub struct AuthProxy {
    config: PluginConfig,
    http_client: HttpClient,
    header_processor: HeaderProcessor,
    response_handler: ResponseHandler,
}

impl Context for AuthProxy {
    fn on_http_call_response(
        &mut self,
        _token_id: u32,
        _num_headers: usize,
        _body_size: usize,
        _num_trailers: usize,
    ) {
        // Handle response from kube-auth-proxy
        if let Some(status) = self.get_http_call_response_header(":status") {
            info!("Auth service response status: {}", status);
            
            match self.response_handler.handle_auth_response(&status) {
                responses::AuthAction::Allow => {
                    // Extract and forward user headers from kube-auth-proxy response
                    if let Some(user) = self.get_http_call_response_header("x-forwarded-user") {
                        self.set_http_request_header("x-forwarded-user", Some(&user));
                    }
                    if let Some(email) = self.get_http_call_response_header("x-forwarded-email") {
                        self.set_http_request_header("x-forwarded-email", Some(&email));
                    }
                    if let Some(token) = self.get_http_call_response_header("x-forwarded-access-token") {
                        self.set_http_request_header("x-forwarded-access-token", Some(&token));
                    }
                    if let Some(gap_auth) = self.get_http_call_response_header("gap-auth") {
                        self.set_http_request_header("gap-auth", Some(&gap_auth));
                    }
                    
                    info!("Authentication successful, allowing request");
                    self.resume_http_request();
                }
                responses::AuthAction::Deny(status_code, message) => {
                    warn!("Authentication failed: {}", message);
                    self.send_http_response(status_code as u32, vec![], Some(message.as_bytes()));
                }
                responses::AuthAction::Redirect(location) => {
                    info!("Redirecting to authentication provider");
                    let headers = vec![("location", location.as_str())];
                    self.send_http_response(302, headers, Some(b"Redirecting to authentication"));
                }
                responses::AuthAction::Error(message) => {
                    error!("Auth service error: {}", message);
                    self.send_http_response(503, vec![], Some(message.as_bytes()));
                }
            }
        } else {
            error!("No status header in auth service response");
            self.send_http_response(503, vec![], Some(b"Invalid auth service response"));
        }
    }
}

impl RootContext for AuthProxy {
    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(configuration_data) = self.get_plugin_configuration() {
            match serde_json::from_slice::<PluginConfig>(&configuration_data) {
                Ok(config) => {
                    info!("BYOIDC WASM Plugin configured successfully");
                    info!("Auth service endpoint: {}", config.auth_service.endpoint);
                    self.config = config;
                    true
                }
                Err(e) => {
                    error!("Failed to parse plugin configuration: {}", e);
                    false
                }
            }
        } else {
            error!("Plugin configuration is missing");
            false
        }
    }

    fn create_http_context(&self, _context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(AuthProxy {
            config: self.config.clone(),
            http_client: HttpClient::new(),
            header_processor: HeaderProcessor::new(),
            response_handler: ResponseHandler::new(),
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

impl HttpContext for AuthProxy {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        info!("BYOIDC: Processing authentication request");

        // Path-agnostic authentication - apply to ALL requests
        // Dynamic HTTPRoute CRs handle routing, WASM handles universal auth
        
        // Extract and forward authentication-relevant headers
        let forwarded_headers = self.header_processor.extract_auth_headers();
        let forwarded_headers_refs: Vec<(&str, &str)> = forwarded_headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        // Get auth service configuration
        let auth_config = &self.config.auth_service;
        
        // Parse endpoint URL to extract scheme and host
        let (scheme, host_with_port) = match self.http_client.parse_endpoint(&auth_config.endpoint) {
            Ok((s, h)) => (s, h),
            Err(e) => {
                error!("Invalid auth service endpoint: {}", e);
                self.send_http_response(503, vec![], Some(b"Auth service configuration error"));
                return Action::Pause;
            }
        };

        // Make HTTP call to kube-auth-proxy service
        let headers = vec![
            (":method", "GET"),
            (":path", &auth_config.verify_path),
            (":authority", &host_with_port),
            (":scheme", &scheme),
        ];

        match self.dispatch_http_call(
            &host_with_port,
            headers,
            None, // No body
            forwarded_headers_refs,
            Duration::from_millis(auth_config.timeout),
        ) {
            Ok(_) => {
                info!("Auth request dispatched to kube-auth-proxy");
                Action::Pause // Wait for response
            }
            Err(e) => {
                error!("Failed to dispatch auth request: {:?}", e);
                self.send_http_response(503, vec![], Some(b"Auth service unavailable"));
                Action::Pause
            }
        }
    }
}

// WASM plugin entry point
proxy_wasm::main! {{
    proxy_wasm::set_log_level(proxy_wasm::types::LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(AuthProxy {
            config: PluginConfig::default(),
            http_client: HttpClient::new(),
            header_processor: HeaderProcessor::new(),
            response_handler: ResponseHandler::new(),
        })
    });
}}