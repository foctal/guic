//! Suspended creation prevents descendants from escaping before job assignment.
use anyhow::{Context, Result, ensure};
use std::{
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
        Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    },
};

pub(super) struct Job(HANDLE);
// SAFETY: the owned kernel handle is independent of the thread and never shared mutably.
unsafe impl Send for Job {}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
impl Job {
    pub(super) fn spawn(command: &mut Command) -> Result<(Child, Self)> {
        // SAFETY: all Windows structure sizes and pointer lifetimes match the API contracts.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            ensure!(
                !handle.is_null(),
                "could not create sidecar job: {}",
                std::io::Error::last_os_error()
            );
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            ensure!(
                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as _,
                    std::mem::size_of_val(&limits) as u32
                ) != 0,
                "could not set sidecar job limits"
            );
            command.creation_flags(CREATE_SUSPENDED);
            let mut child = command
                .spawn()
                .context("could not create suspended sidecar")?;
            let start = (|| -> Result<()> {
                ensure!(
                    AssignProcessToJobObject(handle, child.as_raw_handle()) != 0,
                    "could not assign sidecar job: {}",
                    std::io::Error::last_os_error()
                );
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                ensure!(
                    snapshot != INVALID_HANDLE_VALUE,
                    "could not enumerate sidecar threads"
                );
                let snapshot = Self(snapshot);
                let mut entry: THREADENTRY32 = std::mem::zeroed();
                entry.dwSize = std::mem::size_of_val(&entry) as u32;
                let mut found = Thread32First(snapshot.0, &mut entry);
                while found != 0 {
                    if entry.th32OwnerProcessID == child.id() {
                        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        ensure!(!thread.is_null(), "could not open suspended sidecar thread");
                        let thread = Self(thread);
                        ensure!(
                            ResumeThread(thread.0) != u32::MAX,
                            "could not resume sidecar thread"
                        );
                        return Ok(());
                    }
                    found = Thread32Next(snapshot.0, &mut entry);
                }
                anyhow::bail!("suspended sidecar thread was not found")
            })();
            if let Err(error) = start {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok((child, job))
        }
    }
}
