use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HelperEvent {
    Started { pid: Option<u32> },
    Stdout { line: String },
    Stderr { line: String },
}
