#[cfg(target_os = "macos")]
pub mod macos;
pub mod model;
pub mod observer;

#[cfg(target_os = "macos")]
pub use macos::append_launch_config as append_macos_launch_dns_config;
pub use model::{
    extract_dns_directives, normalize_dns_directive, normalize_match_domain,
    normalize_match_domains, DnsConfig, DnsEffectiveMode, DnsObservation, DnsPolicy,
    DnsRestoreStatus, MatchDomainError, MAX_MATCH_DOMAINS,
};
pub use observer::{DnsObserver, PassiveDnsObserver};
