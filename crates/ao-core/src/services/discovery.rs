use regex::Regex;
use std::sync::LazyLock;

use crate::models::DiscoveredServices;

static DASHBOARD_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Now listening on:\s+(https?://\S+)").unwrap());

static LOGIN_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Login to the dashboard at\s+(https?://\S+)").unwrap());

static RESOURCE_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""(\w[\w.\-]+)"\s+is listening on\s+(https?://\S+)"#).unwrap());

pub fn parse_log_content(log_content: &str) -> DiscoveredServices {
    let mut services = DiscoveredServices::default();

    // Dashboard URL (standard)
    if let Some(caps) = DASHBOARD_URL_RE.captures(log_content) {
        services.dashboard_url = Some(caps[1].to_string());
    }

    // Login URL (Aspire 9.0+) — overrides dashboard URL if present
    if let Some(caps) = LOGIN_URL_RE.captures(log_content) {
        services.dashboard_url = Some(caps[1].to_string());
    }

    // Resource/service URLs
    for caps in RESOURCE_URL_RE.captures_iter(log_content) {
        let name = caps[1].to_string();
        let url = caps[2].to_string();
        services.service_urls.insert(name, url);
    }

    services
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dashboard_url() {
        let log = "info: Now listening on: https://localhost:15234\n";
        let services = parse_log_content(log);
        assert_eq!(
            services.dashboard_url.as_deref(),
            Some("https://localhost:15234")
        );
    }

    #[test]
    fn parse_login_url_overrides_dashboard() {
        let log = "Now listening on: https://localhost:15234\n\
                    Login to the dashboard at https://localhost:15234/login?t=abc123\n";
        let services = parse_log_content(log);
        assert_eq!(
            services.dashboard_url.as_deref(),
            Some("https://localhost:15234/login?t=abc123")
        );
    }

    #[test]
    fn parse_resource_urls() {
        let log = r#""apiservice" is listening on https://localhost:52341
"frontend" is listening on https://localhost:52342
"#;
        let services = parse_log_content(log);
        assert_eq!(
            services.service_urls.get("apiservice"),
            Some(&"https://localhost:52341".to_string())
        );
        assert_eq!(
            services.service_urls.get("frontend"),
            Some(&"https://localhost:52342".to_string())
        );
    }

    #[test]
    fn parse_complete_log() {
        let log = r#"info: Now listening on: https://localhost:15234
Login to the dashboard at https://localhost:15234/login?t=abc
"webfrontend" is listening on https://localhost:5000
"apiservice" is listening on https://localhost:5001
"#;
        let services = parse_log_content(log);
        assert_eq!(
            services.dashboard_url.as_deref(),
            Some("https://localhost:15234/login?t=abc")
        );
        assert_eq!(services.service_urls.len(), 2);
    }

    #[test]
    fn parse_empty_log() {
        let services = parse_log_content("");
        assert!(services.dashboard_url.is_none());
        assert!(services.service_urls.is_empty());
    }
}
