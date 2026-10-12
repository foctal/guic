use anyhow::{Context, Result, bail};
use std::{
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn install_handler() -> Result<()> {
    ctrlc::set_handler(|| CANCELLED.store(true, Ordering::SeqCst))?;
    Ok(())
}

pub(crate) fn check_cancelled() -> Result<()> {
    if CANCELLED.load(Ordering::SeqCst) {
        bail!("operation cancelled");
    }
    Ok(())
}

pub(crate) fn status(command: &mut Command) -> Result<()> {
    // Keep stdout reserved for reports while allowing interactive applications.
    command
        .stdout(std::io::stderr())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    let status = wait(command)?;
    if !status.success() {
        bail!("child process exited with {status}");
    }
    Ok(())
}

pub(crate) fn capture(command: &mut Command) -> Result<Vec<u8>> {
    let output = tempfile::tempfile()?;
    command
        .stdout(output.try_clone()?)
        .stderr(Stdio::inherit())
        .stdin(Stdio::null());
    let status = wait(command)?;
    use std::io::{Read, Seek, Write};
    let mut output = output;
    output.rewind()?;
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    if !status.success() {
        std::io::stderr().write_all(&bytes)?;
        bail!("child process exited with {status}");
    }
    Ok(bytes)
}

fn wait(command: &mut Command) -> Result<ExitStatus> {
    wait_until(command, || Ok(false))?.context("unexpected process restart")
}

pub(crate) fn status_until(
    command: &mut Command,
    changed: impl FnMut() -> Result<bool>,
) -> Result<bool> {
    command
        .stdout(std::io::stderr())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    match wait_until(command, changed)? {
        None => Ok(true),
        Some(status) if status.success() => Ok(false),
        Some(status) => bail!("child process exited with {status}"),
    }
}

fn wait_until(
    command: &mut Command,
    mut changed: impl FnMut() -> Result<bool>,
) -> Result<Option<ExitStatus>> {
    check_cancelled()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().context("could not start child process")?;
    loop {
        let change = changed();
        if CANCELLED.load(Ordering::SeqCst) || !matches!(change, Ok(false)) {
            #[cfg(unix)]
            unsafe {
                // The child owns this process group, including tool descendants.
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            check_cancelled()?;
            change?;
            return Ok(None);
        }
        if let Some(status) = child.try_wait()? {
            check_cancelled()?;
            return Ok(Some(status));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub(crate) fn hook(hook: Option<&crate::config::Hook>) -> Result<()> {
    if let Some(hook) = hook {
        let mut command = Command::new(&hook.program);
        command.args(&hook.args).current_dir(&hook.cwd);
        status(&mut command).context("hook failed")?;
    }
    Ok(())
}
