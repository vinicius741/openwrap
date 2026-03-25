mod connect;
mod errors;
mod events;
mod runtime;
mod state;
mod tests;

pub use connect::submit_credentials;
pub use errors::{
    apply_reconcile_result, apply_terminal_error, dns_restore_error, process_exit_error,
};
pub use events::{handle_exit, handle_log, schedule_retry, ExitAction};
pub use runtime::{
    cleanup_auth_file, cleanup_runtime_artifacts, cleanup_runtime_bridge, prepare_runtime_dir,
    quote_openvpn_arg, write_auth_file, write_launch_config,
};
pub use state::{
    ActiveSession, ConnectionManager, ConnectionPlan, CoreEvent, ManagerState, PendingCredentials,
};
