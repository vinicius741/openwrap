//! Windows DNS policy.
//!
//! Unlike macOS, OpenVPN on Windows applies pushed DNS settings to the tunnel
//! adapter itself (`dhcp-option DNS` / `DOMAIN`, and the 2.6 `dns` directive,
//! which uses NRPT rules for split DNS). OpenWrap therefore does not run any
//! scripts here; it only adds directives that steer OpenVPN's own behavior.

use crate::dns::DnsPolicy;

pub fn append_launch_config(config: &mut String, dns_policy: &DnsPolicy) {
    match dns_policy {
        // Let OpenVPN apply DNS to the tunnel adapter as it normally would.
        DnsPolicy::SplitDnsPreferred => {}
        // Block DNS on all other adapters so every query uses the VPN resolvers.
        DnsPolicy::FullOverride => config.push_str("block-outside-dns\n"),
        // Keep pushed DNS out of Windows entirely; OpenWrap still observes the
        // pushed values from the PUSH_REPLY log line.
        DnsPolicy::ObserveOnly => {
            config.push_str("pull-filter ignore \"dhcp-option DNS\"\n");
            config.push_str("pull-filter ignore \"dhcp-option DOMAIN\"\n");
            config.push_str("pull-filter ignore \"dns \"\n");
            config.push_str("pull-filter ignore \"block-outside-dns\"\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::append_launch_config;
    use crate::dns::DnsPolicy;

    #[test]
    fn split_dns_adds_nothing() {
        let mut config = String::new();
        append_launch_config(&mut config, &DnsPolicy::SplitDnsPreferred);
        assert!(config.is_empty());
    }

    #[test]
    fn full_override_blocks_outside_dns() {
        let mut config = String::new();
        append_launch_config(&mut config, &DnsPolicy::FullOverride);
        assert_eq!(config, "block-outside-dns\n");
    }

    #[test]
    fn observe_only_filters_pushed_dns() {
        let mut config = String::new();
        append_launch_config(&mut config, &DnsPolicy::ObserveOnly);
        assert!(config.contains("pull-filter ignore \"dhcp-option DNS\""));
        assert!(config.contains("pull-filter ignore \"block-outside-dns\""));
    }
}
