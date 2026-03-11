#[cfg(target_os = "macos")]
pub mod macos;
pub mod model;
pub mod observer;

#[cfg(target_os = "macos")]
pub use macos::append_launch_config as append_macos_launch_dns_config;
pub use model::{
    extract_dns_directives, normalize_dns_directive, DnsConfig, DnsEffectiveMode, DnsObservation,
    DnsPolicy,
};
pub use observer::{DnsObserver, PassiveDnsObserver};
