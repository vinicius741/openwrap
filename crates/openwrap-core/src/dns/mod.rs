pub mod model;
pub mod observer;
#[cfg(target_os = "macos")]
pub mod macos;

pub use model::{DnsEffectiveMode, DnsObservation};
pub use observer::{DnsObserver, PassiveDnsObserver};
#[cfg(target_os = "macos")]
pub use macos::append_launch_config as append_macos_launch_dns_config;
