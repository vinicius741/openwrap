use serde::{Deserialize, Serialize};

/// Root-owned file, written by the app's privileged helper install, that holds
/// the uid of the account the helper serves. Only root can create or change
/// files in `/Library/PrivilegedHelperTools`.
pub const HELPER_OWNER_FILE: &str =
    "/Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper.owner";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HelperEvent {
    Started { pid: Option<u32> },
    Stdout { line: String },
    Stderr { line: String },
}
