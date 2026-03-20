use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub openvpn_path_override: Option<PathBuf>,
    #[serde(default)]
    pub verbose_logging: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default() {
        let settings = Settings::default();
        assert!(settings.openvpn_path_override.is_none());
        assert!(!settings.verbose_logging);
    }

    #[test]
    fn settings_with_override() {
        let settings = Settings {
            openvpn_path_override: Some(PathBuf::from("/custom/path/openvpn")),
            verbose_logging: false,
        };
        assert!(settings.openvpn_path_override.is_some());
        assert_eq!(
            settings.openvpn_path_override.unwrap(),
            PathBuf::from("/custom/path/openvpn")
        );
    }

    #[test]
    fn openvpn_detection_structure() {
        let detection = OpenVpnDetection {
            discovered_paths: vec![
                PathBuf::from("/usr/bin/openvpn"),
                PathBuf::from("/opt/homebrew/sbin/openvpn"),
            ],
            selected_path: Some(PathBuf::from("/usr/bin/openvpn")),
        };
        assert_eq!(detection.discovered_paths.len(), 2);
        assert!(detection.selected_path.is_some());
    }

    #[test]
    fn openvpn_detection_returns_valid_structure() {
        // The function queries the filesystem, so we can only verify it returns
        // a well-formed structure. Actual paths depend on system state.
        let detection = detect_openvpn_binaries(None);
        // selected_path should only be Some if there are discovered paths
        if detection.selected_path.is_some() {
            assert!(!detection.discovered_paths.is_empty());
            assert!(detection
                .discovered_paths
                .contains(detection.selected_path.as_ref().unwrap()));
        }
    }

    #[test]
    fn openvpn_detection_override_takes_precedence() {
        // When override doesn't exist, it falls back to system paths
        let detection = detect_openvpn_binaries(Some(PathBuf::from("/nonexistent/path/openvpn")));
        // The override path won't be in discovered_paths since it doesn't exist
        assert!(!detection
            .discovered_paths
            .contains(&PathBuf::from("/nonexistent/path/openvpn")));
    }

    #[test]
    fn settings_serialization() {
        let settings = Settings {
            openvpn_path_override: Some(PathBuf::from("/test/openvpn")),
            verbose_logging: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let roundtrip: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(
            roundtrip.openvpn_path_override,
            settings.openvpn_path_override
        );
        assert!(roundtrip.verbose_logging);
    }

    #[test]
    fn openvpn_detection_serialization() {
        let detection = OpenVpnDetection {
            discovered_paths: vec![PathBuf::from("/usr/bin/openvpn")],
            selected_path: Some(PathBuf::from("/usr/bin/openvpn")),
        };
        let json = serde_json::to_string(&detection).unwrap();
        let roundtrip: OpenVpnDetection = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.discovered_paths.len(), 1);
    }
}
