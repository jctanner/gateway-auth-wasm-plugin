use log::{debug, error};

/// HTTP client wrapper for making authenticated requests to kube-auth-proxy
pub struct HttpClient {}

impl HttpClient {
    pub fn new() -> Self {
        Self {}
    }

    /// Parse endpoint URL to extract scheme and host with port
    /// Example: "https://kube-auth-proxy.auth-system.svc.cluster.local:4180" 
    /// Returns: ("https", "kube-auth-proxy.auth-system.svc.cluster.local:4180")
    pub fn parse_endpoint(&self, endpoint: &str) -> Result<(String, String), String> {
        debug!("Parsing endpoint: {}", endpoint);
        
        if let Some(pos) = endpoint.find("://") {
            let scheme = endpoint[..pos].to_string();
            let host_part = endpoint[pos + 3..].to_string();
            
            // Validate scheme
            if scheme != "https" && scheme != "http" {
                return Err(format!("Unsupported scheme: {}", scheme));
            }
            
            // For security, warn if using HTTP
            if scheme == "http" {
                log::warn!("Using insecure HTTP for auth service communication");
            }
            
            // For cluster-based dispatch, strip the port from the authority
            // Envoy cluster handles the port mapping
            let host_without_port = if let Some(colon_pos) = host_part.find(':') {
                &host_part[..colon_pos]
            } else {
                &host_part
            };
            
            // Validate host part is not empty
            if host_without_port.is_empty() {
                return Err("Host part cannot be empty".to_string());
            }
            
            debug!("Parsed endpoint - scheme: {}, host: {} (original: {})", scheme, host_without_port, host_part);
            Ok((scheme, host_without_port.to_string()))
        } else {
            error!("Invalid endpoint format: missing scheme");
            Err("Invalid endpoint format: must include scheme (https://)".to_string())
        }
    }

    /// Extract hostname from host:port combination for certificate validation
    /// Example: "kube-auth-proxy.auth-system.svc.cluster.local:4180" -> "kube-auth-proxy.auth-system.svc.cluster.local"
    pub fn extract_hostname(&self, host_with_port: &str) -> String {
        if let Some(colon_pos) = host_with_port.rfind(':') {
            // Check if this looks like a port number (IPv6 addresses have multiple colons)
            let potential_port = &host_with_port[colon_pos + 1..];
            if potential_port.parse::<u16>().is_ok() {
                return host_with_port[..colon_pos].to_string();
            }
        }
        // No port found or IPv6 address, return as-is
        host_with_port.to_string()
    }

    /// Validate HTTP headers before sending request
    pub fn validate_headers(&self, headers: &[(&str, &str)]) -> Result<(), String> {
        for (name, value) in headers {
            // Basic header name validation
            if name.is_empty() {
                return Err("Header name cannot be empty".to_string());
            }
            
            // Check for required pseudo-headers for HTTP/2
            match *name {
                ":method" => {
                    if value != &"GET" && value != &"POST" {
                        return Err("Only GET and POST methods are supported".to_string());
                    }
                }
                ":scheme" => {
                    if value != &"https" && value != &"http" {
                        return Err("Only HTTP and HTTPS schemes are supported".to_string());
                    }
                }
                ":path" => {
                    if !value.starts_with('/') {
                        return Err("Path must start with '/'".to_string());
                    }
                }
                _ => {
                    // Regular header validation
                    if name.contains(' ') || name.contains('\t') {
                        return Err(format!("Invalid header name: {}", name));
                    }
                }
            }
            
            // Basic value validation (no control characters)
            if value.chars().any(|c| c.is_control() && c != '\t') {
                return Err(format!("Invalid header value for {}", name));
            }
        }
        
        Ok(())
    }

    /// Build default headers for auth service requests
    pub fn build_auth_headers(&self, method: &str, path: &str, authority: &str, scheme: &str) -> Vec<(&str, String)> {
        vec![
            (":method", method.to_string()),
            (":path", path.to_string()),
            (":authority", authority.to_string()),
            (":scheme", scheme.to_string()),
            ("user-agent", "BYOIDC-WASM-Plugin/1.0".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_endpoint_https() {
        let client = HttpClient::new();
        let result = client.parse_endpoint("https://example.com:443");
        assert!(result.is_ok());
        let (scheme, host) = result.unwrap();
        assert_eq!(scheme, "https");
        assert_eq!(host, "example.com:443");
    }

    #[test]
    fn test_parse_endpoint_invalid() {
        let client = HttpClient::new();
        let result = client.parse_endpoint("invalid-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_hostname() {
        let client = HttpClient::new();
        assert_eq!(
            client.extract_hostname("example.com:8080"),
            "example.com"
        );
        assert_eq!(
            client.extract_hostname("example.com"),
            "example.com"
        );
        assert_eq!(
            client.extract_hostname("[::1]:8080"),
            "[::1]"
        );
    }

    #[test]
    fn test_validate_headers() {
        let client = HttpClient::new();
        let headers = vec![
            (":method", "GET"),
            (":path", "/auth"),
            (":authority", "example.com"),
            (":scheme", "https"),
        ];
        assert!(client.validate_headers(&headers).is_ok());
    }
}
