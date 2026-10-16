//! OS lifecycle only. A cancelled process cannot leave MCP relays or other descendants running.
#![allow(unsafe_code)]
use std::io;
use tokio::process::Command;
#[cfg(unix)]
pub fn configure(command: &mut Command) {
    command.process_group(0);
}
#[cfg(windows)]
pub fn configure(command: &mut Command) {
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
}
#[cfg(unix)]
pub struct Tree(u32);
#[cfg(unix)]
impl Tree {
    pub fn attach(pid: u32) -> io::Result<Self> {
        Ok(Self(pid))
    }
}
#[cfg(unix)]
impl Drop for Tree {
    fn drop(&mut self) {
        if let Ok(pid) = i32::try_from(self.0) {
            // SAFETY: process_group(0) created a fresh group owned by this child. No pointer arguments.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
}
#[cfg(windows)]
pub struct Tree(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for Tree {}
#[cfg(windows)]
impl Tree {
    pub fn attach(pid: u32) -> io::Result<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{JobObjects::*, Threading::*},
        };
        // SAFETY: every handle is checked, configured once, and closed on all paths; no borrowed pointers escape.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(io::Error::last_os_error());
            }
            let tree = Self(job);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false as i32, pid);
            if process.is_null() {
                return Err(io::Error::last_os_error());
            }
            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(tree)
        }
    }
}
#[cfg(windows)]
impl Drop for Tree {
    fn drop(&mut self) {
        // SAFETY: owned job, closed exactly once. KILL_ON_JOB_CLOSE terminates every descendant.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
