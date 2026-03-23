use serde::{Deserialize, Serialize};

use crate::dns::DnsObservation;
use crate::errors::UserFacingError;

use super::ProfileSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRuntimeView {
    pub summary: ProfileSummary,
    pub dns_observation: DnsObservation,
    pub last_error: Option<UserFacingError>,
}
