#![cfg(target_os = "windows")]

use std::thread;
use std::time::{Duration, Instant};

use guic_terminal::{
    LocalPtySession, TerminalCloseMode, TerminalModel, TerminalProcessStatus, TerminalTransport,
    default_shell_command, discover_shell_profiles,
};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

fn pump_output(session: &mut LocalPtySession, model: &mut TerminalModel, output: &mut Vec<u8>) {
    let chunk = session.drain_output();
    if chunk.is_empty() {
        return;
    }
    output.extend(&chunk);
    model.write(&String::from_utf8_lossy(&chunk));
    let responses = model.take_response_bytes();
    if !responses.is_empty() {
        session
            .write(&responses)
            .expect("terminal query responses should be writable");
    }
}

fn prepare_shell(session: &mut LocalPtySession, model: &mut TerminalModel) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    let mut output = Vec::new();
    while Instant::now() < deadline {
        pump_output(session, model, &mut output);
        if !output.is_empty() {
            thread::sleep(Duration::from_millis(100));
            pump_output(session, model, &mut output);
            return;
        }
        thread::sleep(POLL_INTERVAL);
    }
    panic!(
        "timed out preparing the ConPTY shell; captured output: {:?}",
        String::from_utf8_lossy(&output)
    );
}

fn wait_for_output(
    session: &mut LocalPtySession,
    model: &mut TerminalModel,
    needle: &str,
) -> String {
    let deadline = Instant::now() + TEST_TIMEOUT;
    let mut output = Vec::new();
    while Instant::now() < deadline {
        pump_output(session, model, &mut output);
        let text = String::from_utf8_lossy(&output);
        if text.contains(needle) {
            return text.into_owned();
        }
        if !session.is_running() {
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    panic!(
        "timed out waiting for {needle:?}; captured output: {:?}",
        String::from_utf8_lossy(&output)
    );
}

fn wait_for_exit(session: &mut LocalPtySession) -> i32 {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while Instant::now() < deadline {
        if let TerminalProcessStatus::Exited(status) = session.process_status() {
            return status.code;
        }
        thread::sleep(POLL_INTERVAL);
    }
    panic!("timed out waiting for the ConPTY child to exit");
}

fn available_profile(id: &str) -> bool {
    discover_shell_profiles()
        .iter()
        .any(|profile| profile.id().as_ref() == id && profile.is_available())
}

#[test]
fn windows_shell_discovery_selects_an_available_default() {
    let profiles = discover_shell_profiles();
    assert!(
        profiles
            .iter()
            .any(|profile| profile.id().as_ref() == "cmd")
    );
    assert!(
        profiles
            .iter()
            .filter(|profile| profile.is_default())
            .all(|profile| profile.is_available()),
        "an unavailable shell must not be presented as the Windows default"
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile.is_default() && profile.is_available()),
        "Windows must expose one available default shell"
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile.command() == default_shell_command())
    );
}

#[test]
fn windows_conpty_cmd_supports_io_cwd_resize_restart_and_shutdown() {
    let cwd = std::env::temp_dir();
    let cwd_marker_name = format!("guic-conpty-cwd-marker-{}.tmp", std::process::id());
    let cwd_marker = cwd.join(&cwd_marker_name);
    std::fs::write(&cwd_marker, b"").expect("the cwd marker should be writable");
    let mut session = LocalPtySession::spawn_command_in_dir("cmd.exe", 80, 24, &cwd)
        .expect("cmd.exe should start through Windows ConPTY");
    let mut model = TerminalModel::new(80, 24);
    prepare_shell(&mut session, &mut model);

    session
        .write(b"echo GUIC_^CONPTY_READY\r\n")
        .expect("ConPTY input should be writable");
    wait_for_output(&mut session, &mut model, "GUIC_CONPTY_READY");

    session
        .write(
            format!(
                "if exist \"{cwd_marker_name}\" (echo GUIC_^CWD_MATCH) else (echo GUIC_^CWD_MISMATCH)\r\n"
            )
            .as_bytes(),
        )
        .expect("cmd.exe should accept a cwd check");
    let cwd_output = wait_for_output(&mut session, &mut model, "GUIC_CWD_");
    assert!(
        cwd_output.contains("GUIC_CWD_MATCH"),
        "the ConPTY child did not inherit cwd {}; output: {cwd_output:?}",
        cwd.display()
    );

    session
        .resize(101, 33)
        .expect("Windows ConPTY should accept a resize");
    model.resize(101, 33);
    thread::sleep(Duration::from_millis(100));
    session
        .write(
            b"powershell.exe -NoLogo -NoProfile -Command \"Write-Output ([Console]::WindowWidth.ToString() + 'x' + [Console]::WindowHeight.ToString())\"\r\n",
        )
        .expect("the resized ConPTY should remain writable");
    wait_for_output(&mut session, &mut model, "101x33");

    session.restart().expect("a ConPTY session should restart");
    model = TerminalModel::new(101, 33);
    prepare_shell(&mut session, &mut model);
    session
        .write(b"echo GUIC_^CONPTY_RESTARTED\r\n")
        .expect("the restarted ConPTY should accept input");
    wait_for_output(&mut session, &mut model, "GUIC_CONPTY_RESTARTED");

    session
        .close(TerminalCloseMode::Graceful)
        .expect("cmd.exe should accept a graceful exit request");
    assert_eq!(wait_for_exit(&mut session), 0);

    let mut forced = LocalPtySession::spawn_command("cmd.exe", 80, 24)
        .expect("cmd.exe should start for force-close validation");
    forced
        .close(TerminalCloseMode::Force)
        .expect("a ConPTY child should support force close");
    let _ = wait_for_exit(&mut forced);
    std::fs::remove_file(cwd_marker).expect("the cwd marker should be removable");
}

#[test]
fn windows_conpty_powershell_profiles_execute_when_available() {
    for (id, executable) in [("pwsh", "pwsh.exe"), ("powershell", "powershell.exe")] {
        if !available_profile(id) {
            continue;
        }
        let mut session = LocalPtySession::spawn_command(executable, 80, 24)
            .unwrap_or_else(|error| panic!("{executable} should start through ConPTY: {error}"));
        let mut model = TerminalModel::new(80, 24);
        prepare_shell(&mut session, &mut model);
        let marker = format!("GUIC_{}_READY", id.to_ascii_uppercase());
        let marker_suffix = format!("{}_READY", id.to_ascii_uppercase());
        session
            .write(format!("Write-Output ('GUIC_' + '{marker_suffix}')\r\n").as_bytes())
            .unwrap_or_else(|error| panic!("{executable} input should be writable: {error}"));
        wait_for_output(&mut session, &mut model, &marker);
        session
            .close(TerminalCloseMode::Graceful)
            .unwrap_or_else(|error| panic!("{executable} should accept `exit`: {error}"));
        assert_eq!(wait_for_exit(&mut session), 0);
    }
}

#[test]
fn windows_conpty_rejects_a_missing_working_directory() {
    let missing = std::env::temp_dir().join(format!(
        "guic-terminal-path-that-must-not-exist-{}",
        std::process::id()
    ));
    assert!(!missing.exists());
    let error = LocalPtySession::spawn_command_in_dir("cmd.exe", 80, 24, &missing)
        .err()
        .expect("a missing working directory must be rejected");
    assert!(error.to_string().contains("does not exist"));
}
