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
        if is_executable_file(candidate) && !discovered_paths.contains(candidate) {
            discovered_paths.push(candidate.clone());
        }
    }

    let selected_path = discovered_paths.first().cloned();

    OpenVpnDetection {
        discovered_paths,
        selected_path,
    }
}

/// How long `openvpn --version` may take before the binary counts as hung.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Runs `openvpn --version` to check that the binary can actually start.
///
/// A binary can exist and be executable yet fail to launch, typically after a
/// Homebrew update replaces a library it links against. Returns the version
/// (for example `2.7.9`) or a short description of why it could not start.
pub fn probe_openvpn(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not be started: {error}"))?;

    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PROBE_TIMEOUT => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("did not respond".into());
            }
            Err(error) => return Err(format!("could not be checked: {error}")),
        }
    };

    // `--version` prints a few hundred bytes, so reading after exit cannot block.
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }

    // OpenVPN's exit status for `--version` differs between releases, so the
    // banner line is the signal: "OpenVPN 2.7.9 aarch64-apple-darwin ...".
    let version = stdout.lines().next().and_then(|line| {
        let mut words = line.split_whitespace();
        (words.next() == Some("OpenVPN")).then(|| words.next().map(str::to_owned))?
    });
    if let Some(version) = version {
        return Ok(version);
    }

    let detail = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(200).collect::<String>());
    Err(match detail {
        Some(detail) => detail,
        None => format!("exited without output ({status})"),
    })
}

fn is_executable_file(path: &std::path::Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_rejects_directory_override() {
        let dir = tempfile::tempdir().unwrap();
        let detection = detect_openvpn_binaries(Some(dir.path().to_path_buf()));
        assert!(!detection
            .discovered_paths
            .contains(&dir.path().to_path_buf()));
        assert_ne!(detection.selected_path.as_deref(), Some(dir.path()));
    }

    #[cfg(unix)]
    fn fake_openvpn(script: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openvpn");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (dir, path)
    }

    #[cfg(unix)]
    #[test]
    fn probe_reads_the_version_whatever_the_exit_status() {
        for exit in [0, 1] {
            let (_dir, path) = fake_openvpn(&format!(
                "#!/bin/sh\necho 'OpenVPN 2.7.9 aarch64-apple-darwin25.6.0 [SSL (OpenSSL)]'\necho 'library versions: OpenSSL 4.1.0'\nexit {exit}\n"
            ));
            assert_eq!(probe_openvpn(&path), Ok("2.7.9".to_string()));
        }
    }

    #[cfg(unix)]
    #[test]
    fn probe_reports_why_a_binary_cannot_start() {
        let (_dir, path) = fake_openvpn(
            "#!/bin/sh\necho 'dyld[123]: Library not loaded: /opt/homebrew/opt/lz4/lib/liblz4.1.dylib' >&2\nkill -ABRT $$\n",
        );
        let problem = probe_openvpn(&path).unwrap_err();
        assert!(problem.contains("Library not loaded"), "{problem}");

        let (_dir, path) = fake_openvpn("#!/bin/sh\nexit 3\n");
        assert!(probe_openvpn(&path)
            .unwrap_err()
            .contains("exited without output"));

        let (dir, _path) = fake_openvpn("#!/bin/sh\n");
        assert!(probe_openvpn(&dir.path().join("missing"))
            .unwrap_err()
            .contains("could not be started"));
    }

    #[cfg(unix)]
    #[test]
    fn detection_requires_executable_override() {
        use std::os::unix::fs::PermissionsExt;

        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let detection = detect_openvpn_binaries(Some(path.clone()));
        assert!(!detection.discovered_paths.contains(&path));
        assert_ne!(detection.selected_path.as_ref(), Some(&path));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let detection = detect_openvpn_binaries(Some(path.clone()));
        assert_eq!(detection.selected_path, Some(path));
    }

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
