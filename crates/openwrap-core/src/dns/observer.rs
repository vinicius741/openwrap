use crate::dns::model::DnsObservation;

pub trait DnsObserver: Send + Sync {
    fn from_profile(&self, directives: &[String]) -> DnsObservation;
    fn update_from_log(&self, observation: &mut DnsObservation, line: &str) -> bool;
}

#[derive(Debug, Default)]
pub struct PassiveDnsObserver;

impl DnsObserver for PassiveDnsObserver {
    fn from_profile(&self, directives: &[String]) -> DnsObservation {
        let mut observation = DnsObservation {
            config_requested: directives
                .iter()
                .filter_map(|directive| normalize_dns_directive(directive))
                .collect(),
            ..Default::default()
        };

        if !observation.config_requested.is_empty() {
            observation.warnings.push(
                "DNS is observe-only in OpenWrap; runtime DNS values are inferred from OpenVPN logs."
                    .into(),
            );
        }

        observation
    }

    fn update_from_log(&self, observation: &mut DnsObservation, line: &str) -> bool {
        let mut changed = false;
        let mut parsed_any = false;

        for directive in extract_dns_directives(line) {
            parsed_any = true;
            if !observation.runtime_pushed.contains(&directive) {
                observation.runtime_pushed.push(directive);
                changed = true;
            }
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

fn normalize_dns_directive(value: &str) -> Option<String> {
    let tokens = value.split_whitespace().collect::<Vec<_>>();
    match tokens.as_slice() {
        [first, rest @ ..] if first.eq_ignore_ascii_case("DNS") && !rest.is_empty() => {
            Some(format!("DNS {}", rest.join(" ")))
        }
        _ => None,
    }
}

fn extract_dns_directives(line: &str) -> Vec<String> {
    let mut directives = Vec::new();
    for segment in line.split(',') {
        let Some((_, remainder)) = segment.split_once("dhcp-option DNS ") else {
            continue;
        };
        let value = remainder
            .split_whitespace()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(value) = value {
            directives.push(format!("DNS {value}"));
        }
    }
    directives
}

fn push_warning(observation: &mut DnsObservation, warning: &str) -> bool {
    if observation.warnings.iter().any(|current| current == warning) {
        false
    } else {
        observation.warnings.push(warning.into());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{DnsObserver, PassiveDnsObserver};

    #[test]
    fn normalizes_profile_dns_intent() {
        let observer = PassiveDnsObserver;
        let observation = observer.from_profile(&["DNS 1.1.1.1".into(), "DOMAIN corp".into()]);
        assert_eq!(observation.config_requested, vec!["DNS 1.1.1.1"]);
        assert_eq!(observation.warnings.len(), 1);
    }

    #[test]
    fn extracts_runtime_dns_and_warns_on_ambiguous_pushes() {
        let observer = PassiveDnsObserver;
        let mut observation = observer.from_profile(&[]);
        assert!(observer.update_from_log(
            &mut observation,
            "PUSH_REPLY,route-gateway 10.0.0.1,dhcp-option DNS 10.0.0.2,dhcp-option DNS 10.0.0.3"
        ));
        assert_eq!(observation.runtime_pushed, vec!["DNS 10.0.0.2", "DNS 10.0.0.3"]);
        assert!(observer.update_from_log(&mut observation, "PUSH_REPLY,route-gateway 10.0.0.1"));
        assert_eq!(observation.warnings.len(), 1);
    }
}
