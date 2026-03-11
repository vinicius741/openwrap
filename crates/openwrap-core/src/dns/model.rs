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
