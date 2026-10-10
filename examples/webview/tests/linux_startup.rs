#![cfg(target_os = "linux")]

use std::process::Command;

#[test]
fn missing_x11_display_reports_the_wayland_requirement_without_crashing() {
    let output = Command::new(env!("CARGO_BIN_EXE_guic-example-webview"))
        .env_remove("DISPLAY")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .output()
        .expect("run WebView example");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("enable XWayland"));
}

#[test]
fn unreachable_x11_display_reports_initialization_failure_without_crashing() {
    let output = Command::new(env!("CARGO_BIN_EXE_guic-example-webview"))
        .env("DISPLAY", ":65535")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .output()
        .expect("run WebView example");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("could not initialize GTK on X11"));
}
