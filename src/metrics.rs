use log::debug;
use std::collections::HashMap;

/// Metrics collector for BYOIDC WASM plugin observability
pub struct MetricsCollector {
    /// In-memory counters (in production, these would be exported to Prometheus/etc)
    counters: HashMap<String, u64>,
    /// Histogram buckets for latency measurements  
    latency_buckets: Vec<f64>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            latency_buckets: vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        }
    }

    /// Increment a counter metric
    pub fn increment_counter(&mut self, metric_name: &str, labels: &[(&str, &str)]) {
        let key = self.build_metric_key(metric_name, labels);
        let counter = self.counters.entry(key.clone()).or_insert(0);
        *counter += 1;
        debug!("Incremented counter {}: {}", key, *counter);
    }

    /// Record authentication request metrics
    pub fn record_auth_request(&mut self, status: &str, duration_ms: f64) {
        // Increment total requests counter
        self.increment_counter("byoidc_auth_requests_total", &[("status", status)]);
        
        // Record latency
        self.record_histogram("byoidc_auth_request_duration_seconds", duration_ms / 1000.0, &[]);
        
        debug!("Recorded auth request: status={}, duration={}ms", status, duration_ms);
    }

    /// Record authentication service errors
    pub fn record_auth_service_error(&mut self, error_type: &str) {
        self.increment_counter("byoidc_auth_service_errors_total", &[("type", error_type)]);
    }

    /// Record configuration reload events
    pub fn record_config_reload(&mut self, success: bool) {
        let status = if success { "success" } else { "error" };
        self.increment_counter("byoidc_config_reload_total", &[("status", status)]);
    }

    /// Record histogram/timing metrics
    pub fn record_histogram(&mut self, metric_name: &str, value: f64, labels: &[(&str, &str)]) {
        // In a real implementation, this would update histogram buckets
        // For now, just log the value
        let key = self.build_metric_key(metric_name, labels);
        debug!("Recorded histogram {}: {}", key, value);
    }

    /// Build metric key with labels for storage
    fn build_metric_key(&self, metric_name: &str, labels: &[(&str, &str)]) -> String {
        if labels.is_empty() {
            metric_name.to_string()
        } else {
            let labels_str: Vec<String> = labels
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!("{}:{}", metric_name, labels_str.join(","))
        }
    }

    /// Get current counter value
    pub fn get_counter(&self, metric_name: &str, labels: &[(&str, &str)]) -> u64 {
        let key = self.build_metric_key(metric_name, labels);
        self.counters.get(&key).copied().unwrap_or(0)
    }

    /// Export metrics in Prometheus format (simplified)
    pub fn export_prometheus_format(&self) -> String {
        let mut output = String::new();
        
        // Group metrics by base name
        let mut grouped_metrics: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        
        for (key, value) in &self.counters {
            if let Some(colon_pos) = key.find(':') {
                let metric_name = key[..colon_pos].to_string();
                let labels = key[colon_pos + 1..].to_string();
                grouped_metrics.entry(metric_name).or_insert_with(Vec::new).push((labels, *value));
            } else {
                grouped_metrics.entry(key.clone()).or_insert_with(Vec::new).push((String::new(), *value));
            }
        }
        
        // Generate Prometheus format
        for (metric_name, entries) in grouped_metrics {
            output.push_str(&format!("# HELP {} BYOIDC WASM Plugin metric\n", metric_name));
            output.push_str(&format!("# TYPE {} counter\n", metric_name));
            
            for (labels, value) in entries {
                if labels.is_empty() {
                    output.push_str(&format!("{} {}\n", metric_name, value));
                } else {
                    // Convert labels format: "key=value,key2=value2" -> "{key=\"value\",key2=\"value2\"}"
                    let prometheus_labels = labels
                        .split(',')
                        .map(|label| {
                            if let Some(eq_pos) = label.find('=') {
                                let key = &label[..eq_pos];
                                let value = &label[eq_pos + 1..];
                                format!("{}=\"{}\"", key, value)
                            } else {
                                format!("label=\"{}\"", label)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    
                    output.push_str(&format!("{}{{{}}} {}\n", metric_name, prometheus_labels, value));
                }
            }
            output.push('\n');
        }
        
        output
    }

    /// Reset all metrics (useful for testing)
    pub fn reset(&mut self) {
        self.counters.clear();
        debug!("Metrics reset");
    }

    /// Get summary of current metrics
    pub fn get_summary(&self) -> MetricsSummary {
        MetricsSummary {
            total_requests: self.get_counter("byoidc_auth_requests_total", &[]),
            successful_requests: self.get_counter("byoidc_auth_requests_total", &[("status", "202")]),
            failed_requests: self.get_counter("byoidc_auth_requests_total", &[("status", "401")]) 
                + self.get_counter("byoidc_auth_requests_total", &[("status", "403")]),
            service_errors: self.get_counter("byoidc_auth_service_errors_total", &[]),
            config_reloads: self.get_counter("byoidc_config_reload_total", &[("status", "success")]),
        }
    }
}

/// Summary of key metrics
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub service_errors: u64,
    pub config_reloads: u64,
}

impl MetricsSummary {
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.failed_requests as f64 / self.total_requests as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_counter() {
        let mut collector = MetricsCollector::new();
        
        collector.increment_counter("test_metric", &[("status", "success")]);
        collector.increment_counter("test_metric", &[("status", "success")]);
        collector.increment_counter("test_metric", &[("status", "error")]);
        
        assert_eq!(collector.get_counter("test_metric", &[("status", "success")]), 2);
        assert_eq!(collector.get_counter("test_metric", &[("status", "error")]), 1);
    }

    #[test]
    fn test_record_auth_request() {
        let mut collector = MetricsCollector::new();
        
        collector.record_auth_request("202", 50.0);
        collector.record_auth_request("401", 25.0);
        collector.record_auth_request("202", 75.0);
        
        assert_eq!(collector.get_counter("byoidc_auth_requests_total", &[("status", "202")]), 2);
        assert_eq!(collector.get_counter("byoidc_auth_requests_total", &[("status", "401")]), 1);
    }

    #[test]
    fn test_build_metric_key() {
        let collector = MetricsCollector::new();
        
        assert_eq!(collector.build_metric_key("test", &[]), "test");
        assert_eq!(
            collector.build_metric_key("test", &[("key", "value")]), 
            "test:key=value"
        );
        assert_eq!(
            collector.build_metric_key("test", &[("key1", "value1"), ("key2", "value2")]),
            "test:key1=value1,key2=value2"
        );
    }

    #[test]
    fn test_get_summary() {
        let mut collector = MetricsCollector::new();
        
        collector.record_auth_request("202", 50.0);
        collector.record_auth_request("202", 60.0);
        collector.record_auth_request("401", 30.0);
        collector.record_auth_service_error("timeout");
        
        let summary = collector.get_summary();
        assert_eq!(summary.total_requests, 3);
        assert_eq!(summary.successful_requests, 2);
        assert_eq!(summary.failed_requests, 1);
        assert_eq!(summary.service_errors, 1);
        assert_eq!(summary.success_rate(), 2.0 / 3.0);
    }

    #[test] 
    fn test_export_prometheus_format() {
        let mut collector = MetricsCollector::new();
        
        collector.increment_counter("test_counter", &[("status", "success")]);
        collector.increment_counter("test_counter", &[("status", "error")]);
        
        let prometheus_output = collector.export_prometheus_format();
        
        assert!(prometheus_output.contains("# HELP test_counter"));
        assert!(prometheus_output.contains("# TYPE test_counter counter"));
        assert!(prometheus_output.contains("status=\"success\""));
        assert!(prometheus_output.contains("status=\"error\""));
    }
}
