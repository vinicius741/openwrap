use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DnsEffectiveMode {
    ObserveOnly,
    SystemResolvers,
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
