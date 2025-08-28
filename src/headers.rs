use log::{debug, warn};

/// Header processor for extracting and forwarding authentication-relevant headers
pub struct HeaderProcessor {}

impl HeaderProcessor {
    pub fn new() -> Self {
        Self {}
    }

    /// Extract authentication-relevant headers from the incoming request
    /// These headers will be forwarded to kube-auth-proxy for authentication
    pub fn extract_auth_headers(&self) -> Vec<(String, String)> {
        let headers = Vec::new();

        // Standard authentication headers to forward
        let auth_header_names = vec![
            "cookie",
            "authorization", 
            "x-forwarded-user",
            "x-forwarded-for",
            "x-forwarded-proto",
            "x-real-ip",
            "user-agent",
            "accept",
            "accept-language",
            "x-requested-with",
        ];

        for header_name in auth_header_names {
            // Note: In the actual WASM context, we'll use self.get_http_request_header()
            // This is a placeholder structure for the header extraction logic
            debug!("Would extract header: {}", header_name);
        }

        headers
    }

    /// Clean and validate header names for forwarding
    pub fn sanitize_header_name(&self, name: &str) -> Option<String> {
        let cleaned = name.to_lowercase().trim().to_string();
        
        // Skip empty headers
        if cleaned.is_empty() {
            return None;
        }
        
        // Skip pseudo headers (they're handled separately)
        if cleaned.starts_with(':') {
            return None;
        }
        
        // Skip potentially dangerous headers
        let blocked_headers = vec![
            "connection",
            "upgrade", 
            "proxy-connection",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
        ];
        
        if blocked_headers.contains(&cleaned.as_str()) {
            warn!("Blocking potentially dangerous header: {}", cleaned);
            return None;
        }
        
        Some(cleaned)
    }

    /// Validate header value for security
    pub fn validate_header_value(&self, value: &str) -> bool {
        // Check for control characters (except tab)
        if value.chars().any(|c| c.is_control() && c != '\t') {
            warn!("Header value contains control characters");
            return false;
        }
        
        // Check for excessively long values (potential DoS)
        if value.len() > 8192 {
            warn!("Header value too long: {} bytes", value.len());
            return false;
        }
        
        true
    }

    /// Build user context headers from auth service response
    pub fn build_user_headers(&self, auth_response_headers: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut user_headers = Vec::new();
        
        // Map of auth service headers to request headers we should set
        let header_mapping = vec![
            ("x-forwarded-user", "x-forwarded-user"),
            ("x-forwarded-email", "x-forwarded-email"), 
            ("x-forwarded-access-token", "x-forwarded-access-token"),
            ("x-forwarded-groups", "x-forwarded-groups"),
            ("gap-auth", "gap-auth"),
        ];
        
        for (auth_header, request_header) in header_mapping {
            if let Some((_, value)) = auth_response_headers.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(auth_header)) {
                
                if self.validate_header_value(value) {
                    user_headers.push((request_header.to_string(), value.to_string()));
                    debug!("Adding user header: {} = {}", request_header, value);
                }
            }
        }
        
        user_headers
    }

    /// Extract client IP from various headers with priority order
    pub fn extract_client_ip(&self, headers: &[(&str, &str)]) -> Option<String> {
        // Priority order for IP extraction
        let ip_headers = vec![
            "x-real-ip",
            "x-forwarded-for", 
            "x-client-ip",
            "cf-connecting-ip", // Cloudflare
            "true-client-ip",
        ];
        
        for ip_header in ip_headers {
            if let Some((_, value)) = headers.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(ip_header)) {
                
                // For X-Forwarded-For, take the first IP (original client)
                if ip_header == "x-forwarded-for" {
                    if let Some(first_ip) = value.split(',').next() {
                        let ip = first_ip.trim();
                        if !ip.is_empty() && self.is_valid_ip(ip) {
                            return Some(ip.to_string());
                        }
                    }
                } else {
                    let ip = value.trim();
                    if !ip.is_empty() && self.is_valid_ip(ip) {
                        return Some(ip.to_string());
                    }
                }
            }
        }
        
        None
    }

    /// Basic IP address validation
    fn is_valid_ip(&self, ip: &str) -> bool {
        // Basic validation - could be enhanced with proper IP parsing
        if ip.is_empty() || ip.len() > 45 { // Max IPv6 length
            return false;
        }
        
        // Check for obvious invalid patterns
        if ip.contains("..") || ip.starts_with('.') || ip.ends_with('.') {
            return false;
        }
        
        // More comprehensive validation would use std::net::IpAddr::from_str
        // but keeping it simple for WASM compatibility
        true
    }
}

/// Helper trait to provide header access in the actual WASM context
pub trait HeaderAccess {
    fn get_request_header(&self, name: &str) -> Option<String>;
    fn set_request_header(&mut self, name: &str, value: Option<&str>);
    fn get_response_header(&self, name: &str) -> Option<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_header_name() {
        let processor = HeaderProcessor::new();
        
        assert_eq!(processor.sanitize_header_name("Content-Type"), Some("content-type".to_string()));
        assert_eq!(processor.sanitize_header_name("  X-Custom  "), Some("x-custom".to_string()));
        assert_eq!(processor.sanitize_header_name(":authority"), None);
        assert_eq!(processor.sanitize_header_name("connection"), None);
        assert_eq!(processor.sanitize_header_name(""), None);
    }

    #[test]
    fn test_validate_header_value() {
        let processor = HeaderProcessor::new();
        
        assert!(processor.validate_header_value("valid-value"));
        assert!(processor.validate_header_value("value with spaces"));
        assert!(!processor.validate_header_value("value\nwith\ncontrol"));
        assert!(!processor.validate_header_value(&"x".repeat(10000))); // Too long
    }

    #[test]
    fn test_extract_client_ip() {
        let processor = HeaderProcessor::new();
        
        let headers = vec![
            ("x-forwarded-for", "192.168.1.1, 10.0.0.1"),
            ("x-real-ip", "203.0.113.1"),
        ];
        
        assert_eq!(processor.extract_client_ip(&headers), Some("192.168.1.1".to_string()));
    }

    #[test] 
    fn test_build_user_headers() {
        let processor = HeaderProcessor::new();
        
        let auth_headers = vec![
            ("x-forwarded-user", "alice@example.com"),
            ("x-forwarded-groups", "admin,developer"),
            ("gap-auth", "alice@example.com"),
        ];
        
        let user_headers = processor.build_user_headers(&auth_headers);
        assert_eq!(user_headers.len(), 3);
        assert!(user_headers.contains(&("x-forwarded-user".to_string(), "alice@example.com".to_string())));
    }
}
