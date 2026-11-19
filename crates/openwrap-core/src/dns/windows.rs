//! Windows DNS policy.
//!
//! Unlike macOS, OpenVPN on Windows applies pushed DNS settings to the tunnel
//! adapter itself (`dhcp-option DNS` / `DOMAIN`, and the 2.6 `dns` directive,
//! which uses NRPT rules for split DNS). OpenWrap therefore does not run any
//! scripts here; it only adds directives that steer OpenVPN's own behavior.

use crate::dns::DnsPolicy;

pub fn apply_launch_config(config: &mut String, dns_policy: &DnsPolicy) {
    match dns_policy {
        // Keep normal network DNS available while letting OpenVPN apply VPN DNS
        // to the tunnel adapter: strip any profile-local `block-outside-dns`
        // and ignore a server-pushed `block-outside-dns`.
        DnsPolicy::SplitDnsPreferred => {
            strip_matching_lines(config, is_block_outside_dns_line);
            ensure_trailing_newline(config);
            config.push_str("pull-filter ignore \"block-outside-dns\"\n");
        }
        // Block DNS on all other adapters so every query uses the VPN resolvers.
        DnsPolicy::FullOverride => {
            let already_present = config.lines().any(is_block_outside_dns_line);
            if !already_present {
                ensure_trailing_newline(config);
                config.push_str("block-outside-dns\n");
            }
        }
        // Keep both profile-local and pushed DNS directives out of Windows;
        // OpenWrap still observes pushed values from the PUSH_REPLY log line.
        DnsPolicy::ObserveOnly => {
            strip_matching_lines(config, is_local_dns_override_line);
            ensure_trailing_newline(config);
            config.push_str("pull-filter ignore \"dhcp-option DNS\"\n");
            config.push_str("pull-filter ignore \"dhcp-option DOMAIN\"\n");
            config.push_str("pull-filter ignore \"dhcp-option ADAPTER_DOMAIN_SUFFIX\"\n");
            config.push_str("pull-filter ignore \"dns \"\n");
            config.push_str("pull-filter ignore \"block-outside-dns\"\n");
        }
    }
}

fn ensure_trailing_newline(config: &mut String) {
    if !config.is_empty() && !config.ends_with('\n') {
        config.push('\n');
    }
}

fn strip_matching_lines(config: &mut String, should_strip: fn(&str) -> bool) {
    if config.is_empty() {
        return;
    }
    let mut filtered = String::with_capacity(config.len());
    for line in config.lines() {
        if !should_strip(line) {
            filtered.push_str(line);
            filtered.push('\n');
        }
    }
    *config = filtered;
}

fn is_block_outside_dns_line(line: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|directive| directive.eq_ignore_ascii_case("block-outside-dns"))
}

fn is_local_dns_override_line(line: &str) -> bool {
    let mut tokens = line.split_whitespace();
    let Some(directive) = tokens.next() else {
        return false;
    };
    if directive.eq_ignore_ascii_case("block-outside-dns") || directive.eq_ignore_ascii_case("dns")
    {
        return true;
    }
    if directive.eq_ignore_ascii_case("dhcp-option") {
        if let Some(option) = tokens.next() {
            return option.eq_ignore_ascii_case("DNS")
                || option.eq_ignore_ascii_case("DNS6")
                || option.eq_ignore_ascii_case("DOMAIN")
                || option.eq_ignore_ascii_case("DOMAIN-SEARCH")
                || option.eq_ignore_ascii_case("ADAPTER_DOMAIN_SUFFIX");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::apply_launch_config;
    use crate::dns::DnsPolicy;

    #[test]
    fn split_dns_strips_and_ignores_block_outside_dns() {
        let mut config =
            "client\ndhcp-option DNS 10.0.0.53\nblock-outside-dns\nauth-nocache\n".to_string();
        apply_launch_config(&mut config, &DnsPolicy::SplitDnsPreferred);
        assert!(config.contains("dhcp-option DNS 10.0.0.53\n"));
        assert!(!config
            .lines()
            .any(|line| line.trim() == "block-outside-dns"));
        assert!(config.contains("pull-filter ignore \"block-outside-dns\"\n"));
    }

    #[test]
    fn full_override_blocks_outside_dns_without_duplicating() {
        let mut config = String::new();
        apply_launch_config(&mut config, &DnsPolicy::FullOverride);
        assert_eq!(config, "block-outside-dns\n");

        let mut existing = "client\nblock-outside-dns\n".to_string();
        apply_launch_config(&mut existing, &DnsPolicy::FullOverride);
        assert_eq!(
            existing
                .lines()
                .filter(|line| *line == "block-outside-dns")
                .count(),
            1
        );
    }

    #[test]
    fn observe_only_strips_local_and_filters_pushed_dns() {
        let mut config = "\
client
remote vpn.example.com 1194
dhcp-option DNS 10.0.0.53
dhcp-option DNS6 fd00::53
dhcp-option DOMAIN corp.example
dhcp-option DOMAIN-SEARCH sub.corp.example
dhcp-option ADAPTER_DOMAIN_SUFFIX corp.example
dhcp-option NTP 10.0.0.123
dns server 1 address 10.0.0.53
block-outside-dns
auth-nocache
"
        .to_string();

        apply_launch_config(&mut config, &DnsPolicy::ObserveOnly);

        assert!(config.contains("remote vpn.example.com 1194\n"));
        assert!(config.contains("dhcp-option NTP 10.0.0.123\n"));
        assert!(config.contains("auth-nocache\n"));
        assert!(!config.contains("dhcp-option DNS 10.0.0.53"));
        assert!(!config.contains("dhcp-option DNS6 fd00::53"));
        assert!(!config.contains("dhcp-option DOMAIN corp.example"));
        assert!(!config.contains("dhcp-option DOMAIN-SEARCH sub.corp.example"));
        assert!(!config.contains("dhcp-option ADAPTER_DOMAIN_SUFFIX corp.example"));
        assert!(!config.contains("dns server 1 address 10.0.0.53"));
        assert!(!config
            .lines()
            .any(|line| line.trim() == "block-outside-dns"));
        assert!(config.contains("pull-filter ignore \"dhcp-option DNS\"\n"));
        assert!(config.contains("pull-filter ignore \"dhcp-option DOMAIN\"\n"));
        assert!(config.contains("pull-filter ignore \"dhcp-option ADAPTER_DOMAIN_SUFFIX\"\n"));
        assert!(config.contains("pull-filter ignore \"dns \"\n"));
        assert!(config.contains("pull-filter ignore \"block-outside-dns\"\n"));
    }
}
