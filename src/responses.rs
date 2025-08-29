use log::{debug, info, warn, error};

/// Actions that can be taken based on authentication response
#[derive(Debug, PartialEq)]
pub enum AuthAction {
    /// Allow the request and optionally forward user headers
    Allow,
    /// Deny the request with specific status code and message
    Deny(u16, String),
    /// Redirect to authentication provider
    Redirect(String),
    /// Service error occurred
    Error(String),
}

/// Response handler for processing kube-auth-proxy authentication responses
pub struct ResponseHandler {}

impl ResponseHandler {
    pub fn new() -> Self {
        Self {}
    }

    /// Handle authentication response from kube-auth-proxy
    /// Maps HTTP status codes to appropriate actions according to the design document
    pub fn handle_auth_response(&self, status: &str) -> AuthAction {
        debug!("Processing auth response with status: {}", status);
        
        match status {
            "202" => {
                // Accepted - kube-auth-proxy returns this for authenticated requests
                // This is the expected response for successful authentication
                info!("Authentication successful (202 Accepted)");
                AuthAction::Allow
            }
            "401" => {
                // Unauthorized - authentication required
                warn!("Authentication required (401 Unauthorized)");
                AuthAction::Deny(401, "Authentication required".to_string())
            }
            "403" => {
                // For kube-auth-proxy, 403 means "redirect to login" - forward the response
                info!("kube-auth-proxy returning sign-in page (403)");
                AuthAction::Redirect("sign-in-page".to_string()) // Will forward the actual response content
            }
            "302" => {
                // Found - redirect to authentication provider
                // This should not happen in auth-only mode, but handle it gracefully
                info!("Auth service requested redirect (302 Found)");
                // Note: In actual implementation, we'd extract Location header
                AuthAction::Redirect("/oauth2/start".to_string())
            }
            "307" => {
                // Temporary redirect - also handle redirect case
                info!("Auth service requested temporary redirect (307)");
                AuthAction::Redirect("/oauth2/start".to_string())
            }
            "408" => {
                // Request timeout
                error!("Auth service request timeout (408)");
                AuthAction::Error("Authentication service timeout".to_string())
            }
            "429" => {
                // Too many requests
                warn!("Auth service rate limited (429 Too Many Requests)");
                AuthAction::Deny(429, "Too many authentication requests".to_string())
            }
            "500" | "502" | "503" | "504" => {
                // Server errors from auth service
                error!("Auth service error ({})", status);
                AuthAction::Error(format!("Authentication service error: {}", status))
            }
            _ => {
                // Any other response code is treated as a service error
                error!("Unexpected auth service response: {}", status);
                AuthAction::Error(format!("Unexpected authentication service response: {}", status))
            }
        }
    }

    /// Extract redirect URL from response headers
    pub fn extract_redirect_url(&self, headers: &[(&str, &str)]) -> Option<String> {
        // Look for Location header (case-insensitive)
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("location") {
                if !value.is_empty() && self.is_valid_redirect_url(value) {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Validate redirect URL for security
    fn is_valid_redirect_url(&self, url: &str) -> bool {
        // Basic validation to prevent open redirects
        if url.is_empty() || url.len() > 2048 {
            return false;
        }

        // Must be relative or same-origin
        if url.starts_with('/') {
            // Relative URL - safe
            return true;
        }

        if url.starts_with("https://") || url.starts_with("http://") {
            // Absolute URL - would need additional validation in production
            // For now, be conservative and reject
            warn!("Rejecting absolute redirect URL for security: {}", url);
            return false;
        }

        // Reject anything else (javascript:, data:, etc.)
        false
    }

    /// Build error response based on authentication failure type
    pub fn build_error_response(&self, auth_action: &AuthAction) -> (u16, Vec<(String, String)>, String) {
        match auth_action {
            AuthAction::Deny(status, message) => {
                let headers = vec![
                    ("content-type".to_string(), "text/plain".to_string()),
                    ("cache-control".to_string(), "no-cache, no-store".to_string()),
                ];
                (*status, headers, message.clone())
            }
            AuthAction::Redirect(location) => {
                let headers = vec![
                    ("location".to_string(), location.clone()),
                    ("cache-control".to_string(), "no-cache, no-store".to_string()),
                ];
                (302, headers, "Redirecting to authentication".to_string())
            }
            AuthAction::Error(message) => {
                let headers = vec![
                    ("content-type".to_string(), "text/plain".to_string()),
                    ("cache-control".to_string(), "no-cache, no-store".to_string()),
                ];
                (503, headers, message.clone())
            }
            AuthAction::Allow => {
                // This shouldn't happen when building error responses
                (200, vec![], "OK".to_string())
            }
        }
    }

    /// Determine if response indicates a temporary vs permanent failure
    pub fn is_temporary_failure(&self, auth_action: &AuthAction) -> bool {
        match auth_action {
            AuthAction::Error(_) => true,  // Service errors are temporary
            AuthAction::Deny(status, _) => {
                match status {
                    429 => true,  // Rate limiting is temporary
                    408 => true,  // Timeout is temporary
                    500..=599 => true,  // Server errors are temporary
                    _ => false,   // Client errors (401, 403) are not temporary
                }
            }
            AuthAction::Redirect(_) => false,  // Redirects are not failures
            AuthAction::Allow => false,        // Success is not a failure
        }
    }

    /// Get human-readable description of the authentication result
    pub fn get_result_description(&self, auth_action: &AuthAction) -> String {
        match auth_action {
            AuthAction::Allow => "Authentication successful".to_string(),
            AuthAction::Deny(401, _) => "Authentication required - please log in".to_string(),
            AuthAction::Deny(403, _) => "Access denied - insufficient permissions".to_string(),
            AuthAction::Deny(429, _) => "Rate limited - too many authentication attempts".to_string(),
            AuthAction::Deny(status, _) => format!("Authentication failed ({})", status),
            AuthAction::Redirect(_) => "Redirecting to authentication provider".to_string(),
            AuthAction::Error(_) => "Authentication service temporarily unavailable".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_auth_response_success() {
        let handler = ResponseHandler::new();
        
        assert_eq!(handler.handle_auth_response("202"), AuthAction::Allow);
        assert_eq!(handler.handle_auth_response("200"), AuthAction::Allow);
    }

    #[test]
    fn test_handle_auth_response_failures() {
        let handler = ResponseHandler::new();
        
        match handler.handle_auth_response("401") {
            AuthAction::Deny(401, _) => {},
            _ => panic!("Expected Deny action for 401"),
        }
        
        match handler.handle_auth_response("403") {
            AuthAction::Deny(403, _) => {},
            _ => panic!("Expected Deny action for 403"),
        }
    }

    #[test]
    fn test_handle_auth_response_redirects() {
        let handler = ResponseHandler::new();
        
        match handler.handle_auth_response("302") {
            AuthAction::Redirect(_) => {},
            _ => panic!("Expected Redirect action for 302"),
        }
    }

    #[test]
    fn test_handle_auth_response_errors() {
        let handler = ResponseHandler::new();
        
        match handler.handle_auth_response("500") {
            AuthAction::Error(_) => {},
            _ => panic!("Expected Error action for 500"),
        }
        
        match handler.handle_auth_response("999") {
            AuthAction::Error(_) => {},
            _ => panic!("Expected Error action for unknown status"),
        }
    }

    #[test]
    fn test_is_valid_redirect_url() {
        let handler = ResponseHandler::new();
        
        assert!(handler.is_valid_redirect_url("/oauth2/start"));
        assert!(handler.is_valid_redirect_url("/login?redirect=https%3A//example.com"));
        assert!(!handler.is_valid_redirect_url("https://evil.com/"));
        assert!(!handler.is_valid_redirect_url("javascript:alert(1)"));
        assert!(!handler.is_valid_redirect_url(""));
    }

    #[test]
    fn test_extract_redirect_url() {
        let handler = ResponseHandler::new();
        
        let headers = vec![
            ("content-type", "text/html"),
            ("location", "/oauth2/start"),
            ("cache-control", "no-cache"),
        ];
        
        assert_eq!(handler.extract_redirect_url(&headers), Some("/oauth2/start".to_string()));
        
        let no_location_headers = vec![("content-type", "text/html")];
        assert_eq!(handler.extract_redirect_url(&no_location_headers), None);
    }

    #[test]
    fn test_is_temporary_failure() {
        let handler = ResponseHandler::new();
        
        assert!(handler.is_temporary_failure(&AuthAction::Error("service down".to_string())));
        assert!(handler.is_temporary_failure(&AuthAction::Deny(429, "rate limited".to_string())));
        assert!(handler.is_temporary_failure(&AuthAction::Deny(503, "service unavailable".to_string())));
        
        assert!(!handler.is_temporary_failure(&AuthAction::Deny(401, "unauthorized".to_string())));
        assert!(!handler.is_temporary_failure(&AuthAction::Deny(403, "forbidden".to_string())));
        assert!(!handler.is_temporary_failure(&AuthAction::Allow));
    }
}
