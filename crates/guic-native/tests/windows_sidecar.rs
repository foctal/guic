#![cfg(windows)]

use anyhow::{Context, Result, ensure};
use guic_core::CancellationToken;
use guic_native::Sidecars;
use std::{
    io::{Read, Write},
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn windows_sidecar_arguments_streams_and_process_tree_cleanup() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("space \u{65e5}\u{672c}\u{8a9e}");
    std::fs::create_dir(&root)?;
    let helper = root.join("helper.exe");
    let output = Command::new("rustc")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sidecar_helper.rs"
        ))
        .args(["--edition=2024", "-o"])
        .arg(&helper)
        .output()?;
    ensure!(
        output.status.success(),
        "helper compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sidecars = Sidecars::beside(&helper)?;
    assert_eq!(sidecars.resolve("helper")?, helper.canonicalize()?);
    let arguments = [
        "",
        "two words",
        "\u{65e5}\u{672c}\u{8a9e}",
        "quote\"inside",
        "trailing space\\",
        "\\\"",
        "$literal; & data",
    ];
    let mut command = sidecars.command("helper")?;
    command.arg("echo").current_dir(&root);
    for argument in arguments {
        command.arg(argument);
    }
    let mut child = command.spawn_piped()?;
    child
        .take_stdin()
        .context("stdin")?
        .write_all(b"stdin marker\n")?;
    let mut stdout = child.take_stdout().context("stdout")?;
    let mut stderr = child.take_stderr().context("stderr")?;
    let status = child.wait(&CancellationToken::new(), Some(Duration::from_secs(10)))?;
    let mut out = String::new();
    let mut err = String::new();
    stdout.read_to_string(&mut out)?;
    stderr.read_to_string(&mut err)?;
    assert_eq!(status.code(), Some(23));
    assert_eq!(out, format!("{}\nstdin marker\n", arguments.join("\n")));
    assert_eq!(err, "stderr marker\n");
    child.terminate()?;

    for mode in ["cancel", "timeout", "drop"] {
        let pid_file = root.join(format!("{mode}.txt"));
        let mut child = sidecars
            .command("helper")?
            .arg("tree")
            .arg(&pid_file)
            .arg("2")
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let pids = loop {
            let text = std::fs::read_to_string(&pid_file).unwrap_or_default();
            let pids: Vec<u32> = text.lines().filter_map(|line| line.parse().ok()).collect();
            if pids.len() == 3 {
                break pids;
            }
            ensure!(
                Instant::now() < deadline,
                "helper descendants did not start"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        // Hold handles before termination so PID reuse cannot invalidate assertions.
        let handles: Vec<_> = pids.into_iter().map(Process::open).collect::<Result<_>>()?;
        match mode {
            "cancel" => {
                let token = CancellationToken::new();
                token.cancel();
                assert!(child.wait(&token, None).is_err());
            }
            "timeout" => assert!(
                child
                    .wait(&CancellationToken::new(), Some(Duration::ZERO))
                    .is_err()
            ),
            _ => {}
        }
        drop(child);
        for process in handles {
            process.assert_exited()?;
        }
    }
    assert!(
        sidecars
            .command("helper")?
            .current_dir(root.join("missing"))
            .spawn()
            .is_err()
    );
    Ok(())
}

struct Process(windows_sys::Win32::Foundation::HANDLE);
impl Process {
    fn open(pid: u32) -> Result<Self> {
        // SAFETY: OpenProcess returns an owned handle, closed by Drop.
        let handle =
            unsafe { windows_sys::Win32::System::Threading::OpenProcess(0x00100000, 0, pid) };
        ensure!(!handle.is_null(), "could not open helper process {pid}");
        Ok(Self(handle))
    }
    fn assert_exited(&self) -> Result<()> {
        // SAFETY: this instance owns a live process handle with SYNCHRONIZE access.
        let result =
            unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(self.0, 5000) };
        ensure!(
            result == 0,
            "sidecar process survived cleanup: wait result {result}"
        );
        Ok(())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
