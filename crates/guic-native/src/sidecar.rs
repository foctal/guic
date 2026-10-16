use anyhow::{Context, Result, ensure};
use guic_core::CancellationToken;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Installed sidecar lookup rooted beside the application executable.
///
/// This matches cargo-guic's macOS Contents/MacOS, Windows installation directory,
/// and Linux usr/bin layouts, including the mounted AppImage filesystem. Neither
/// PATH nor the working directory participates in lookup.
#[derive(Clone, Debug)]
pub struct Sidecars {
    root: PathBuf,
}
impl Sidecars {
    /// Locates sidecars next to the current executable.
    pub fn discover() -> Result<Self> {
        Self::beside(std::env::current_exe()?)
    }
    /// Uses the parent of an explicitly supplied application executable.
    pub fn beside(executable: impl AsRef<Path>) -> Result<Self> {
        let executable = executable.as_ref().canonicalize()?;
        ensure!(executable.is_file(), "application executable is not a file");
        Ok(Self {
            root: executable
                .parent()
                .context("executable has no parent")?
                .to_owned(),
        })
    }
    /// Resolves a configured plain sidecar name, adding .exe on Windows.
    pub fn resolve(&self, name: &str) -> Result<PathBuf> {
        ensure!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
            "invalid sidecar name"
        );
        let path = self.root.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        let resolved = path
            .canonicalize()
            .with_context(|| format!("sidecar {name} is missing"))?;
        ensure!(
            resolved.parent() == Some(self.root.as_path()) && resolved.is_file(),
            "sidecar escaped its installation directory"
        );
        Ok(resolved)
    }
    /// Creates a command without invoking a shell. Arguments preserve OS encoding.
    pub fn command(&self, name: &str) -> Result<SidecarCommand> {
        Ok(SidecarCommand {
            command: Command::new(self.resolve(name)?),
        })
    }
}

/// Sidecar command builder. Standard streams default to null to avoid UI stalls.
pub struct SidecarCommand {
    command: Command,
}
impl SidecarCommand {
    /// Adds one literal argument; shell expansion is never performed.
    pub fn arg(&mut self, argument: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(argument);
        self
    }
    /// Sets an environment variable for the child only.
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.command.env(key, value);
        self
    }
    /// Sets the child working directory.
    pub fn current_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(path);
        self
    }
    /// Starts with piped input and output. Drain both output pipes concurrently
    /// before waiting if the child writes more than the operating-system buffers.
    pub fn spawn_piped(&mut self) -> Result<SidecarChild> {
        self.command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.spawn_configured()
    }
    /// Starts with null input and output, retaining an owned process handle.
    pub fn spawn(&mut self) -> Result<SidecarChild> {
        self.command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.spawn_configured()
    }
    fn spawn_configured(&mut self) -> Result<SidecarChild> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            self.command.process_group(0);
        }
        #[cfg(windows)]
        let (child, job) = crate::windows_job::Job::spawn(&mut self.command)?;
        #[cfg(not(windows))]
        let child = self.command.spawn().context("could not start sidecar")?;
        Ok(SidecarChild {
            id: child.id(),
            child: Some(child),
            status: None,
            #[cfg(windows)]
            job: Some(job),
        })
    }
}

/// Owned child process, terminated and reaped on drop.
///
/// Blocking waits must run on GPUI's background executor. Unix children run in
/// an owned process group; Windows uses a Job Object assigned before execution.
pub struct SidecarChild {
    id: u32,
    child: Option<Child>,
    status: Option<ExitStatus>,
    #[cfg(windows)]
    job: Option<crate::windows_job::Job>,
}
impl SidecarChild {
    /// Original process ID; it remains available after exit for diagnostics.
    pub fn id(&self) -> u32 {
        self.id
    }
    /// Takes the piped standard input, when requested at spawn time.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.as_mut()?.stdin.take()
    }
    /// Takes the piped standard output, when requested at spawn time.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.as_mut()?.stdout.take()
    }
    /// Takes the piped standard error, when requested at spawn time.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.as_mut()?.stderr.take()
    }
    /// Waits for exit, cancellation, or a timeout. Cancellation and timeout both
    /// terminate and reap the process before returning an error.
    pub fn wait(
        &mut self,
        cancellation: &CancellationToken,
        timeout: Option<Duration>,
    ) -> Result<ExitStatus> {
        let started = Instant::now();
        loop {
            if cancellation.is_cancelled()
                || timeout.is_some_and(|limit| started.elapsed() >= limit)
            {
                self.terminate()?;
                anyhow::bail!("sidecar wait cancelled or timed out");
            }
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    /// Returns a completed status without blocking.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        let child = self.child.as_mut().context("sidecar already reaped")?;
        #[cfg(unix)]
        {
            // SAFETY: waitid initializes siginfo without reaping this owned child.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            #[cfg(target_os = "macos")]
            let pid = info.si_pid;
            #[cfg(not(target_os = "macos"))]
            let pid = unsafe { info.si_pid() };
            if pid == 0 {
                return Ok(None);
            }
            // SAFETY: the unreaped leader still reserves this process-group ID.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
        }
        let status = child.try_wait()?;
        if status.is_some() {
            #[cfg(windows)]
            {
                self.job.take();
            }
            self.status = status;
        }
        Ok(status)
    }
    /// Terminates the owned process group/job and reaps the direct child.
    /// Descendants must not deliberately detach from the application's process group.
    /// Repeated calls succeed.
    pub fn terminate(&mut self) -> Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        let running = self.try_wait()?.is_none();
        let child = self.child.as_mut().context("sidecar already reaped")?;
        if running {
            #[cfg(unix)]
            {
                // SAFETY: the child has not exited/reaped, so its ID cannot be reused.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            #[cfg(windows)]
            {
                self.job.take();
            }
            if let Err(error) = child.kill()
                && child.try_wait()?.is_none()
            {
                return Err(error.into());
            }
        }
        self.status = Some(child.wait()?);
        self.child = None;
        Ok(())
    }
}
impl Drop for SidecarChild {
    fn drop(&mut self) {
        if let Err(error) = self.terminate() {
            tracing::error!("could not terminate sidecar: {error}");
        }
    }
}

/// Application-owned sidecars registered for coordinated lifecycle cleanup.
#[derive(Clone, Default)]
pub struct SidecarProcesses(Arc<Mutex<Vec<SidecarChild>>>);
impl SidecarProcesses {
    /// Takes ownership of a child until cleanup. Returns its process ID.
    pub fn retain(&self, child: SidecarChild) -> Result<u32> {
        let id = child.id();
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("sidecar lock poisoned"))?
            .push(child);
        Ok(id)
    }
    /// Reaps completed children and returns their process IDs and exit statuses.
    pub fn reap_finished(&self) -> Result<Vec<(u32, ExitStatus)>> {
        let mut children = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("sidecar lock poisoned"))?;
        let mut exited = Vec::new();
        let mut error = None;
        children.retain_mut(|child| match child.try_wait() {
            Ok(Some(status)) => {
                exited.push((child.id(), status));
                false
            }
            Ok(None) => true,
            Err(e) => {
                error = Some(e);
                true
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(exited)
    }
    /// Periodically reaps retained children on a worker. Retain this task unless
    /// using `DesktopState`, which owns a monitor after `install`.
    pub fn monitor(&self, cx: &gpui::App) -> gpui::Task<()> {
        let processes = self.clone();
        cx.spawn(async move |cx| {
            loop {
                let processes = processes.clone();
                if let Err(error) = cx
                    .background_executor()
                    .spawn(async move { processes.reap_finished() })
                    .await
                {
                    tracing::error!("could not reap sidecars: {error}");
                }
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
        })
    }
    /// Terminates all retained children, preserving failures for another attempt.
    pub fn terminate_all(&self) -> Result<()> {
        let mut children = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("sidecar lock poisoned"))?;
        let mut error = None;
        children.retain_mut(|child| match child.terminate() {
            Ok(()) => false,
            Err(e) => {
                error = Some(e);
                true
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }
    /// Adds background cleanup for quit and restart; closing one window keeps sidecars alive.
    pub fn bind(&self, lifecycle: &crate::Lifecycle) {
        let processes = self.clone();
        lifecycle.cleanup(move |reason, cx| {
            let processes = processes.clone();
            let task = cx.background_executor().spawn(async move {
                if !matches!(reason, crate::ShutdownReason::CloseWindow(_)) {
                    processes.terminate_all()?;
                }
                Ok(())
            });
            Box::pin(task)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup_rejects_escape_and_missing_files() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let app = directory.path().join("app");
        std::fs::write(&app, "app")?;
        let sidecars = Sidecars::beside(app)?;
        for invalid in ["../other", "/bin/sh", "helper.exe", "", "missing"] {
            assert!(sidecars.resolve(invalid).is_err());
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_reaps_process_and_arguments_are_literal() -> Result<()> {
        use std::io::Read;
        let mut command = SidecarCommand {
            command: Command::new("/bin/echo"),
        };
        command.arg("$(never-executed) with spaces");
        let mut child = command.spawn_piped()?;
        let mut output = String::new();
        child
            .take_stdout()
            .expect("piped output")
            .read_to_string(&mut output)?;
        assert!(
            child
                .wait(&CancellationToken::new(), Some(Duration::from_secs(2)))?
                .success()
        );
        assert_eq!(output, "$(never-executed) with spaces\n");
        let mut command = SidecarCommand {
            command: Command::new("/bin/sleep"),
        };
        command.arg("60");
        let mut child = command.spawn()?;
        let token = CancellationToken::new();
        token.cancel();
        assert!(child.wait(&token, None).is_err());
        assert!(child.child.is_none());
        child.terminate()?;
        Ok(())
    }
}
