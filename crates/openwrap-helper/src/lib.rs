//! The setuid helper is a macOS/Unix-only component. On Windows the app runs
//! OpenVPN directly (elevated), so the crate compiles to an empty library there.
#![cfg(unix)]

pub mod connect;
pub mod reconcile;
pub mod request;
pub mod system;
pub mod tests;

pub use connect::run_connect;
pub use reconcile::run_reconcile_dns;
pub use request::{read_json_request, validate_request};
pub use request::{ConnectRequest, ReconcileDnsRequest};
