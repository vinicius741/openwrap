use crate::dns::model::{DnsConfig, DnsEffectiveMode, DnsObservation, DnsPolicy};

const MISSING_DOMAIN_WARNING: &str =
    "VPN DNS servers were provided without VPN domains, so OpenWrap left normal system DNS unchanged. Switch this profile to FullOverride if all DNS should use the VPN.";

pub trait DnsObserver: Send + Sync {
    fn from_profile(&self, directives: &[String], policy: DnsPolicy) -> DnsObservation;
    fn update_from_log(&self, observation: &mut DnsObservation, line: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct PassiveDnsObserver;

impl DnsObserver for PassiveDnsObserver {
    fn from_profile(&self, directives: &[String], policy: DnsPolicy) -> DnsObservation {
        let config = DnsConfig::from_directives(directives);
        let mut observation = DnsObservation {
            config_requested: directives
                .iter()
                .filter_map(|directive| crate::dns::normalize_dns_directive(directive))
                .collect(),
            effective_mode: default_effective_mode(&policy),
            ..Default::default()
        };

        if !observation.config_requested.is_empty() {
            observation
                .warnings
                .push(describe_effective_mode(&policy).into());
        }

        if policy == DnsPolicy::SplitDnsPreferred
            && config.has_servers()
            && !config.has_scoped_domains()
        {
            observation.warnings.push(MISSING_DOMAIN_WARNING.into());
        }

        observation
    }

    fn update_from_log(&self, observation: &mut DnsObservation, line: &str) -> bool {
        let mut changed = false;
        let mut parsed_any = false;

        if let Some(warning) = line
            .split_once("OPENWRAP_DNS_WARNING:")
            .map(|(_, warning)| warning.trim())
            .filter(|warning| !warning.is_empty())
        {
            changed |= push_warning(observation, warning);
        }

        let directives = crate::dns::extract_dns_directives(line);
        let saw_scoped_domain = directives.iter().any(|directive| {
            directive.starts_with("DOMAIN ") || directive.starts_with("DOMAIN-SEARCH ")
        });

        for directive in directives {
            parsed_any = true;
            if !observation.runtime_pushed.contains(&directive) {
                observation.runtime_pushed.push(directive);
                changed = true;
            }
        }

        if saw_scoped_domain {
            changed |= remove_warning(observation, MISSING_DOMAIN_WARNING);
        }

        if line.contains("PUSH_REPLY") && !parsed_any {
            changed |= push_warning(
                observation,
                "OpenVPN reported pushed options, but OpenWrap could not safely confirm pushed DNS values.",
            );
        } else if line.contains("dhcp-option") && !line.contains("dhcp-option DNS") {
            changed |= push_warning(
                observation,
                "OpenVPN reported non-DNS DHCP options; OpenWrap does not trust them for DNS state.",
            );
        }

        changed
    }
}

#[cfg(target_os = "macos")]
fn default_effective_mode(policy: &DnsPolicy) -> DnsEffectiveMode {
    match policy {
        DnsPolicy::SplitDnsPreferred => DnsEffectiveMode::ScopedResolvers,
        DnsPolicy::FullOverride => DnsEffectiveMode::GlobalOverride,
        DnsPolicy::ObserveOnly => DnsEffectiveMode::ObserveOnly,
    }
}

#[cfg(not(target_os = "macos"))]
fn default_effective_mode(_policy: &DnsPolicy) -> DnsEffectiveMode {
    DnsEffectiveMode::ObserveOnly
}

fn push_warning(observation: &mut DnsObservation, warning: &str) -> bool {
    if observation
        .warnings
        .iter()
        .any(|current| current == warning)
    {
        false
    } else {
        observation.warnings.push(warning.into());
        true
    }
}

fn remove_warning(observation: &mut DnsObservation, warning: &str) -> bool {
    let original_len = observation.warnings.len();
    observation.warnings.retain(|current| current != warning);
    observation.warnings.len() != original_len
}

fn describe_effective_mode(policy: &DnsPolicy) -> &'static str {
    match policy {
        DnsPolicy::ObserveOnly => {
            "OpenWrap will not change system DNS for this profile; any DNS shown here is inferred from the profile or OpenVPN runtime logs."
        }
        DnsPolicy::SplitDnsPreferred => {
            "OpenWrap uses VPN DNS only for configured VPN domains on macOS and leaves normal internet DNS on the local network."
        }
        DnsPolicy::FullOverride => {
            "OpenWrap routes all system DNS through the VPN on macOS and restores the previous resolver settings on disconnect."
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dns::{DnsEffectiveMode, DnsPolicy};

    use super::{DnsObserver, PassiveDnsObserver, MISSING_DOMAIN_WARNING};

    #[test]
    fn normalizes_profile_dns_intent() {
        let observer = PassiveDnsObserver;
        let observation = observer.from_profile(
            &[
                "DNS 1.1.1.1".into(),
                "DOMAIN corp.example".into(),
                "DOMAIN-SEARCH corp.example lab.example".into(),
            ],
            DnsPolicy::SplitDnsPreferred,
        );
        assert!(observation
            .config_requested
            .contains(&"DNS 1.1.1.1".to_string()));
        assert!(observation
            .config_requested
            .contains(&"DOMAIN corp.example".to_string()));
        assert!(observation
            .config_requested
            .contains(&"DOMAIN-SEARCH corp.example lab.example".to_string()));
    }

    #[test]
    fn extracts_runtime_dns_and_warns_on_ambiguous_pushes() {
        let observer = PassiveDnsObserver;
        let mut observation = observer.from_profile(&[], DnsPolicy::SplitDnsPreferred);
        assert!(observer.update_from_log(
            &mut observation,
            "PUSH_REPLY,route-gateway 10.0.0.1,dhcp-option DNS 10.0.0.2,dhcp-option DOMAIN corp.example,dhcp-option DOMAIN-SEARCH corp.example lab.example"
        ));
        assert_eq!(
            observation.runtime_pushed,
            vec![
                "DNS 10.0.0.2",
                "DOMAIN corp.example",
                "DOMAIN-SEARCH corp.example lab.example"
            ]
        );
        assert!(observer.update_from_log(&mut observation, "PUSH_REPLY,route-gateway 10.0.0.1"));
        assert_eq!(observation.warnings.len(), 1);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reports_policy_effective_mode_on_macos() {
        let observer = PassiveDnsObserver;
        let observation = observer.from_profile(
            &["DNS 10.0.1.50".into(), "DOMAIN corp.example".into()],
            DnsPolicy::SplitDnsPreferred,
        );

        assert_eq!(
            observation.effective_mode,
            DnsEffectiveMode::ScopedResolvers
        );
    }

    #[test]
    fn warns_when_split_dns_has_no_domains() {
        let observer = PassiveDnsObserver;
        let mut observation =
            observer.from_profile(&["DNS 10.0.1.50".into()], DnsPolicy::SplitDnsPreferred);

        assert!(observation
            .warnings
            .iter()
            .any(|warning| warning == MISSING_DOMAIN_WARNING));

        assert!(observer.update_from_log(
            &mut observation,
            "PUSH_REPLY,dhcp-option DNS 10.0.1.50,dhcp-option DOMAIN corp.example"
        ));
        assert!(!observation
            .warnings
            .iter()
            .any(|warning| warning == MISSING_DOMAIN_WARNING));
    }
}
