//! Thin Win32 wrappers used by the Windows code paths (OpenVPN launch, graceful
//! disconnect, OpenVPN detection). Everything here is `#[cfg(windows)]`.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, WAIT_TIMEOUT};
use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, OpenProcess, SetEvent, TerminateProcess, WaitForSingleObject,
    PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

/// `CREATE_NO_WINDOW`: keeps console children (openvpn.exe) from flashing a
/// console window when spawned from the GUI process.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// True when the current process token is elevated. OpenVPN needs elevation to
/// add routes and configure the tunnel adapter when launched directly.
pub fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

/// Named manual-reset event handed to OpenVPN via `--service <name> 0`.
/// OpenVPN watches it and shuts down cleanly (restoring routes and DNS) once it
/// is signalled, which replaces SIGTERM on Windows.
#[derive(Debug)]
pub struct ExitEvent {
    name: String,
    handle: HANDLE,
}

// The handle is a kernel object reference; Win32 event APIs are thread-safe.
unsafe impl Send for ExitEvent {}
unsafe impl Sync for ExitEvent {}

impl ExitEvent {
    pub fn create(name: String) -> io::Result<Self> {
        let wide = to_wide(&name);
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, wide.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { name, handle })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn signal(&self) {
        unsafe {
            SetEvent(self.handle);
        }
    }
}

impl Drop for ExitEvent {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Owned process handle. Holding it also prevents the PID from being reused
/// while a forced termination is still pending.
#[derive(Debug)]
pub struct ProcessHandle(HANDLE);

unsafe impl Send for ProcessHandle {}
unsafe impl Sync for ProcessHandle {}

impl ProcessHandle {
    pub fn open(pid: u32) -> Option<Self> {
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid) };
        (!handle.is_null()).then_some(Self(handle))
    }

    /// Waits up to `grace` for the process to exit, then terminates it.
    pub fn wait_or_terminate(&self, grace: Duration) {
        let millis = u32::try_from(grace.as_millis()).unwrap_or(u32::MAX);
        unsafe {
            if WaitForSingleObject(self.0, millis) == WAIT_TIMEOUT {
                TerminateProcess(self.0, 1);
            }
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Reads a REG_SZ value from `HKLM\<subkey>` (64-bit view).
pub fn read_hklm_string(subkey: &str, value: Option<&str>) -> Option<String> {
    let subkey = to_wide(subkey);
    let value = value.map(to_wide);
    let value_ptr = value
        .as_ref()
        .map_or(std::ptr::null(), |value| value.as_ptr());
    let flags = RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY;

    let mut size: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value_ptr,
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS || size == 0 {
        return None;
    }

    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value_ptr,
            flags,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let value = OsString::from_wide(&buffer[..len])
        .to_string_lossy()
        .into_owned();
    (!value.is_empty()).then_some(value)
}

/// Locations of `openvpn.exe` reported by the OpenVPN Community installer,
/// followed by the default install directories.
pub fn openvpn_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(exe_path) = read_hklm_string(r"SOFTWARE\OpenVPN", Some("exe_path")) {
        candidates.push(PathBuf::from(exe_path));
    }
    if let Some(install_dir) = read_hklm_string(r"SOFTWARE\OpenVPN", None) {
        candidates.push(PathBuf::from(install_dir).join("bin").join("openvpn.exe"));
    }
    for variable in ["ProgramW6432", "ProgramFiles"] {
        if let Some(dir) = std::env::var_os(variable) {
            candidates.push(
                PathBuf::from(dir)
                    .join("OpenVPN")
                    .join("bin")
                    .join("openvpn.exe"),
            );
        }
    }
    candidates.push(PathBuf::from(r"C:\Program Files\OpenVPN\bin\openvpn.exe"));
    candidates
}
