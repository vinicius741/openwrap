//! Show a file or folder in the platform file manager (Finder / Explorer).

use std::path::Path;
use std::process::Command;

use crate::error::CommandError;

pub fn reveal_in_file_manager(path: &Path) -> Result<(), CommandError> {
    platform_reveal(path).map_err(|error| CommandError::from(openwrap_core::AppError::Io(error)))
}

#[cfg(target_os = "macos")]
fn platform_reveal(path: &Path) -> std::io::Result<()> {
    Command::new("/usr/bin/open").arg("-R").arg(path).status()?;
    Ok(())
}

#[cfg(windows)]
fn platform_reveal(path: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;

    // explorer.exe parses `/select,"<path>"` directly and fails if Rust wraps
    // the entire flag+path in outer quotes when the path contains spaces.
    // explorer.exe also reports a non-zero exit code even on success, so only
    // spawn failures are treated as errors.
    let raw_arg = windows_explorer_select_arg(path)?;
    Command::new(windows_explorer_path())
        .raw_arg(raw_arg)
        .spawn()?;
    Ok(())
}

#[cfg(windows)]
fn windows_explorer_path() -> std::path::PathBuf {
    std::env::var_os("SystemRoot")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("explorer.exe")
}

#[cfg(any(windows, test))]
fn windows_explorer_select_arg(path: &Path) -> std::io::Result<String> {
    let raw = path.to_string_lossy();
    let normalized = if let Some(unc) = raw
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| raw.strip_prefix(r"\\?\unc\"))
    {
        format!(r"\\{}", unc.replace('/', "\\"))
    } else {
        raw.strip_prefix(r"\\?\").unwrap_or(&raw).replace('/', "\\")
    };
    if normalized.contains('"') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path contains an unsupported quote character",
        ));
    }
    Ok(format!("/select,\"{normalized}\""))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_reveal(path: &Path) -> std::io::Result<()> {
    let target = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::windows_explorer_select_arg;

    #[test]
    fn formats_explorer_select_arg_with_spaces_and_verbatim_prefix() {
        let arg = windows_explorer_select_arg(Path::new(
            r"\\?\C:\Users\Jane Doe\AppData\Local\OpenWrap/logs/openvpn.log",
        ))
        .unwrap();
        assert_eq!(
            arg,
            r#"/select,"C:\Users\Jane Doe\AppData\Local\OpenWrap\logs\openvpn.log""#
        );
    }

    #[test]
    fn formats_explorer_select_arg_for_verbatim_unc_paths() {
        let arg = windows_explorer_select_arg(Path::new(
            r"\\?\UNC\fileserver\share\OpenWrap/logs/openvpn.log",
        ))
        .unwrap();
        assert_eq!(
            arg,
            r#"/select,"\\fileserver\share\OpenWrap\logs\openvpn.log""#
        );
    }

    #[test]
    fn rejects_embedded_quotes_in_explorer_select_arg() {
        let err =
            windows_explorer_select_arg(Path::new(r#"C:\Users\bad"path\file.log"#)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
