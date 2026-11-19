//! Thin Win32 wrappers used by the Windows code paths (OpenVPN launch, graceful
//! disconnect, OpenVPN detection). Everything here is `#[cfg(windows)]`.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::RawHandle;
use std::path::PathBuf;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, DuplicateHandle, ERROR_ACCESS_DENIED, ERROR_SUCCESS, HANDLE, WAIT_FAILED,
    WAIT_OBJECT_0,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenProcess, SetEvent, TerminateProcess, WaitForSingleObject,
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
        if unsafe { SetEvent(self.handle) } == 0 {
            eprintln!(
                "Warning: failed to signal OpenVPN exit event {}: {}",
                self.name,
                io::Error::last_os_error()
            );
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
    /// Duplicates an existing child process handle without re-opening by PID,
    /// avoiding PID reuse races and `OpenProcess` access failures.
    pub fn from_raw_handle(raw: RawHandle) -> Option<Self> {
        let current = unsafe { GetCurrentProcess() };
        let mut duplicated: HANDLE = std::ptr::null_mut();
        let ok = unsafe {
            DuplicateHandle(
                current,
                raw as HANDLE,
                current,
                &mut duplicated,
                PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                0,
                0,
            )
        };
        if ok != 0 && !duplicated.is_null() {
            Some(Self(duplicated))
        } else {
            eprintln!(
                "Warning: DuplicateHandle for OpenVPN child failed: {}",
                io::Error::last_os_error()
            );
            None
        }
    }

    pub fn open(pid: u32) -> Option<Self> {
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid) };
        (!handle.is_null()).then_some(Self(handle))
    }

    /// Waits up to `grace` for the process to exit, then terminates it.
    pub fn wait_or_terminate(&self, grace: Duration) {
        let millis = u32::try_from(grace.as_millis()).unwrap_or(u32::MAX);
        let wait_result = unsafe { WaitForSingleObject(self.0, millis) };
        if wait_result == WAIT_OBJECT_0 {
            return;
        }
        if wait_result == WAIT_FAILED {
            eprintln!(
                "Warning: WaitForSingleObject on OpenVPN process failed: {}",
                io::Error::last_os_error()
            );
        }
        if unsafe { TerminateProcess(self.0, 1) } == 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(ERROR_ACCESS_DENIED as i32) {
                eprintln!("Warning: TerminateProcess on OpenVPN failed: {err}");
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

/// Job object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
/// Assigning `openvpn.exe` to this job guarantees Windows terminates it if
/// OpenWrap crashes or exits unexpectedly before a graceful disconnect finishes.
#[derive(Debug)]
pub struct JobObject(HANDLE);

unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

impl JobObject {
    pub fn create_kill_on_close() -> io::Result<Self> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            let err = io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            return Err(err);
        }
        Ok(Self(handle))
    }

    pub fn assign_raw_handle(&self, raw: RawHandle) -> io::Result<()> {
        if unsafe { AssignProcessToJobObject(self.0, raw as HANDLE) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for JobObject {
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

#[cfg(test)]
mod tests {
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{openvpn_candidates, to_wide, ExitEvent, JobObject, ProcessHandle};

    #[test]
    fn to_wide_null_terminates_utf16() {
        let wide = to_wide("abc");
        assert_eq!(wide, vec![b'a' as u16, b'b' as u16, b'c' as u16, 0]);
    }

    #[test]
    fn exit_event_can_be_created_and_signaled() {
        let name = format!("openwrap-test-event-{}", uuid::Uuid::new_v4());
        let event = ExitEvent::create(name.clone()).unwrap();
        assert_eq!(event.name(), name);
        event.signal();
    }

    #[test]
    fn process_handle_terminates_after_grace_period() {
        let mut child = Command::new("ping.exe")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let handle = ProcessHandle::from_raw_handle(child.as_raw_handle())
            .expect("should duplicate child process handle");
        handle.wait_or_terminate(Duration::from_millis(50));
        let status = child.wait().unwrap();
        assert!(!status.success());
    }

    #[test]
    fn job_object_terminates_assigned_process_on_drop() {
        let job = JobObject::create_kill_on_close().expect("should create job object");
        let mut child = Command::new("ping.exe")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        job.assign_raw_handle(child.as_raw_handle())
            .expect("should assign child process to job");
        // Windows terminates processes on JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        // with exit code 0, so verify the 29 s ping exits immediately on drop.
        let started = Instant::now();
        drop(job);
        let _ = child.wait().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "dropping JobObject did not promptly terminate assigned child"
        );
    }

    #[test]
    fn openvpn_candidates_include_default_program_files_path() {
        let candidates = openvpn_candidates();
        assert!(candidates
            .iter()
            .any(|path| path
                == &std::path::PathBuf::from(r"C:\Program Files\OpenVPN\bin\openvpn.exe")));
    }
}
