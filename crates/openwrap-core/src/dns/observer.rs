use crate::dns::model::DnsObservation;

pub trait DnsObserver: Send + Sync {
    fn from_profile(&self, directives: &[String]) -> DnsObservation;
    fn update_from_log(&self, observation: &mut DnsObservation, line: &str);
}

#[derive(Debug, Default)]
pub struct PassiveDnsObserver;

impl DnsObserver for PassiveDnsObserver {
    fn from_profile(&self, directives: &[String]) -> DnsObservation {
        DnsObservation {
            config_requested: directives.to_vec(),
            ..Default::default()
        }
    }

    fn update_from_log(&self, observation: &mut DnsObservation, line: &str) {
        if line.contains("dhcp-option DNS") || line.contains("PUSH_REPLY") {
            observation.runtime_pushed.push(line.to_string());
        }
    }
}

