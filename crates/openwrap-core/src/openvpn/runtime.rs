use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub openvpn_path_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenVpnDetection {
    pub discovered_paths: Vec<PathBuf>,
    pub selected_path: Option<PathBuf>,
}

pub fn detect_openvpn_binaries(override_path: Option<PathBuf>) -> OpenVpnDetection {
    let mut discovered_paths = Vec::new();

    for candidate in [
        override_path.as_ref(),
        Some(&PathBuf::from("/opt/homebrew/sbin/openvpn")),
        Some(&PathBuf::from("/usr/local/sbin/openvpn")),
        Some(&PathBuf::from("/usr/bin/openvpn")),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.exists() && !discovered_paths.contains(candidate) {
            discovered_paths.push(candidate.clone());
        }
    }

    let selected_path = override_path
        .filter(|path| path.exists())
        .or_else(|| discovered_paths.first().cloned());

    OpenVpnDetection {
        discovered_paths,
        selected_path,
    }
}
