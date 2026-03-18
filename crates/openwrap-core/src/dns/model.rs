use std::collections::BTreeSet;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DnsEffectiveMode {
    ObserveOnly,
    ScopedResolvers,
    GlobalOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DnsPolicy {
    SplitDnsPreferred,
    FullOverride,
    ObserveOnly,
}

impl Default for DnsPolicy {
    fn default() -> Self {
        Self::SplitDnsPreferred
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DnsConfig {
    pub servers: Vec<IpAddr>,
    pub match_domains: Vec<String>,
    pub search_domains: Vec<String>,
}

impl DnsConfig {
    pub fn from_directives(directives: &[String]) -> Self {
        let mut config = Self::default();

        for directive in directives
            .iter()
            .filter_map(|directive| normalize_dns_directive(directive))
        {
            let tokens = directive.split_whitespace().collect::<Vec<_>>();
            match tokens.as_slice() {
                ["DNS", value] => {
                    if let Ok(server) = value.parse::<IpAddr>() {
                        if !config.servers.contains(&server) {
                            config.servers.push(server);
                        }
                    }
                }
                ["DOMAIN", value] => push_domain(&mut config.match_domains, value),
                ["DOMAIN-SEARCH", values @ ..] => {
                    for value in values {
                        push_domain(&mut config.search_domains, value);
                    }
                }
                _ => {}
            }
        }

        config
    }

    pub fn has_servers(&self) -> bool {
        !self.servers.is_empty()
    }

    pub fn scoped_domains(&self) -> Vec<String> {
        let mut combined = BTreeSet::new();
        for domain in self.match_domains.iter().chain(self.search_domains.iter()) {
            combined.insert(domain.clone());
        }
        combined.into_iter().collect()
    }

    pub fn has_scoped_domains(&self) -> bool {
        !self.match_domains.is_empty() || !self.search_domains.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsObservation {
    pub config_requested: Vec<String>,
    pub runtime_pushed: Vec<String>,
    pub effective_mode: DnsEffectiveMode,
    pub warnings: Vec<String>,
}

impl Default for DnsObservation {
    fn default() -> Self {
        Self {
            config_requested: Vec::new(),
            runtime_pushed: Vec::new(),
            effective_mode: DnsEffectiveMode::ObserveOnly,
            warnings: Vec::new(),
        }
    }
}

pub fn normalize_dns_directive(value: &str) -> Option<String> {
    let tokens = value.split_whitespace().collect::<Vec<_>>();
    match tokens.as_slice() {
        [first, value] if first.eq_ignore_ascii_case("DNS") => value
            .parse::<IpAddr>()
            .ok()
            .map(|server| format!("DNS {server}")),
        [first, value] if first.eq_ignore_ascii_case("DOMAIN") => {
            normalize_domain(value).map(|domain| format!("DOMAIN {domain}"))
        }
        [first, values @ ..]
            if first.eq_ignore_ascii_case("DOMAIN-SEARCH") && !values.is_empty() =>
        {
            let domains = values
                .iter()
                .filter_map(|value| normalize_domain(value))
                .collect::<Vec<_>>();
            (!domains.is_empty()).then(|| format!("DOMAIN-SEARCH {}", domains.join(" ")))
        }
        _ => None,
    }
}

pub fn extract_dns_directives(line: &str) -> Vec<String> {
    let mut directives = Vec::new();
    for segment in line.split(',') {
        let segment = segment.trim();
        let Some((_, remainder)) = segment.split_once("dhcp-option ") else {
            continue;
        };
        if let Some(normalized) = normalize_dns_directive(remainder) {
            directives.push(normalized);
        }
    }
    directives
}

fn normalize_domain(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('.');
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('-')
        || trimmed.ends_with('-')
    {
        return None;
    }

    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
        .then(|| trimmed.to_ascii_lowercase())
}

fn push_domain(target: &mut Vec<String>, value: &str) {
    if let Some(domain) = normalize_domain(value) {
        if !target.contains(&domain) {
            target.push(domain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn dns_effective_mode_serialization() {
        let modes = vec![
            DnsEffectiveMode::ObserveOnly,
            DnsEffectiveMode::ScopedResolvers,
            DnsEffectiveMode::GlobalOverride,
        ];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let roundtrip: DnsEffectiveMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, roundtrip);
        }
    }

    #[test]
    fn dns_policy_default() {
        assert_eq!(DnsPolicy::default(), DnsPolicy::SplitDnsPreferred);
    }

    #[test]
    fn dns_policy_serialization() {
        let policies = vec![
            DnsPolicy::SplitDnsPreferred,
            DnsPolicy::FullOverride,
            DnsPolicy::ObserveOnly,
        ];
        for policy in policies {
            let json = serde_json::to_string(&policy).unwrap();
            let roundtrip: DnsPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(policy, roundtrip);
        }
    }

    #[test]
    fn dns_config_default() {
        let config = DnsConfig::default();
        assert!(config.servers.is_empty());
        assert!(config.match_domains.is_empty());
        assert!(config.search_domains.is_empty());
    }

    #[test]
    fn dns_config_from_directives_empty() {
        let config = DnsConfig::from_directives(&[]);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn dns_config_from_directives_with_dns_server() {
        let config = DnsConfig::from_directives(&[String::from("DNS 1.1.1.1")]);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0], IpAddr::from([1, 1, 1, 1]));
    }

    #[test]
    fn dns_config_from_directives_with_multiple_dns_servers() {
        let config =
            DnsConfig::from_directives(&[String::from("DNS 1.1.1.1"), String::from("DNS 8.8.8.8")]);
        assert_eq!(config.servers.len(), 2);
    }

    #[test]
    fn dns_config_from_directives_with_domain() {
        let config = DnsConfig::from_directives(&[String::from("DOMAIN example.com")]);
        assert_eq!(config.match_domains, vec!["example.com"]);
    }

    #[test]
    fn dns_config_from_directives_with_domain_search() {
        let config = DnsConfig::from_directives(&[String::from(
            "DOMAIN-SEARCH corp.example.com lab.example.com",
        )]);
        assert_eq!(config.search_domains.len(), 2);
        assert!(config
            .search_domains
            .contains(&"corp.example.com".to_string()));
        assert!(config
            .search_domains
            .contains(&"lab.example.com".to_string()));
    }

    #[test]
    fn dns_config_has_servers() {
        let mut config = DnsConfig::default();
        assert!(!config.has_servers());
        config.servers.push(IpAddr::from([1, 1, 1, 1]));
        assert!(config.has_servers());
    }

    #[test]
    fn dns_config_scoped_domains() {
        let mut config = DnsConfig::default();
        config.match_domains.push("example.com".to_string());
        config.search_domains.push("lab.example.com".to_string());
        let scoped = config.scoped_domains();
        assert_eq!(scoped.len(), 2);
        assert!(scoped.contains(&"example.com".to_string()));
        assert!(scoped.contains(&"lab.example.com".to_string()));
    }

    #[test]
    fn dns_config_has_scoped_domains() {
        let mut config = DnsConfig::default();
        assert!(!config.has_scoped_domains());
        config.match_domains.push("example.com".to_string());
        assert!(config.has_scoped_domains());
    }

    #[test]
    fn dns_observation_default() {
        let obs = DnsObservation::default();
        assert!(obs.config_requested.is_empty());
        assert!(obs.runtime_pushed.is_empty());
        assert_eq!(obs.effective_mode, DnsEffectiveMode::ObserveOnly);
        assert!(obs.warnings.is_empty());
    }

    #[test]
    fn normalize_dns_directive_dns() {
        assert_eq!(
            normalize_dns_directive("DNS 1.1.1.1"),
            Some("DNS 1.1.1.1".to_string())
        );
        assert_eq!(
            normalize_dns_directive("dns 8.8.8.8"),
            Some("DNS 8.8.8.8".to_string())
        );
    }

    #[test]
    fn normalize_dns_directive_domain() {
        assert_eq!(
            normalize_dns_directive("DOMAIN example.com"),
            Some("DOMAIN example.com".to_string())
        );
        assert_eq!(
            normalize_dns_directive("domain corp.example"),
            Some("DOMAIN corp.example".to_string())
        );
    }

    #[test]
    fn normalize_dns_directive_domain_search() {
        assert_eq!(
            normalize_dns_directive("DOMAIN-SEARCH corp.example lab.example"),
            Some("DOMAIN-SEARCH corp.example lab.example".to_string())
        );
    }

    #[test]
    fn normalize_dns_directive_invalid() {
        assert_eq!(normalize_dns_directive("NTP 1.2.3.4"), None);
        assert_eq!(normalize_dns_directive("DNS not-an-ip"), None);
        assert_eq!(normalize_dns_directive(""), None);
    }

    #[test]
    fn extract_dns_directives_parses_dhcp_options() {
        let line = "dhcp-option DNS 1.1.1.1, dhcp-option DOMAIN example.com";
        let directives = extract_dns_directives(line);
        assert_eq!(directives.len(), 2);
        assert!(directives.contains(&"DNS 1.1.1.1".to_string()));
        assert!(directives.contains(&"DOMAIN example.com".to_string()));
    }

    #[test]
    fn extract_dns_directives_handles_multiple() {
        let line = "dhcp-option DNS 1.1.1.1, dhcp-option DNS 8.8.8.8";
        let directives = extract_dns_directives(line);
        assert_eq!(directives.len(), 2);
    }

    #[test]
    fn extract_dns_directives_skips_non_dns_options() {
        let line = "dhcp-option DNS 1.1.1.1, dhcp-option NTP 1.2.3.4";
        let directives = extract_dns_directives(line);
        assert_eq!(directives.len(), 1);
    }

    #[test]
    fn extract_dns_directives_empty_line() {
        let directives = extract_dns_directives("no dhcp-options here");
        assert!(directives.is_empty());
    }
}
