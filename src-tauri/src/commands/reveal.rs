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
    // `/select,<path>` must be a single argument; explorer.exe reports a
    // non-zero exit code even on success, so only spawn failures are errors.
    let mut select = std::ffi::OsString::from("/select,");
    select.push(path.as_os_str());
    Command::new("explorer.exe").arg(select).spawn()?;
    Ok(())
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
