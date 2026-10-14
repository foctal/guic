# Windows validation for the v0.4.0 preparation

Date: 2026-10-14. Base revision: `572b3753e7c61e9b5f516ae95e2ebdfcd0d63a0f`
on `dev`; the initial working tree was clean. These results include the local
uncommitted fixes described below. No release, publication, commit, tag, push,
or pull request was performed. Package versions were not changed.

## Environment and evidence

- Windows 11 Home, version 10.0.26200, x86_64.
- Rust/Cargo 1.95.0, `x86_64-pc-windows-msvc`, Visual Studio 2022 MSVC.
- GPUI: `guic-gpui` 0.4.0; packaging backend: cargo-packager 0.11.8.
- Processes ran with medium integrity and the Administrators SID disabled for
  granting access. This checks non-elevated execution, not a separate account
  that has never belonged to Administrators.
- Build and test logs are retained locally in `target/windows-validation/`.
  Packaging records and candidates are under `target/guic/`; these are ignored
  local artifacts, not checked-in release assets.
- Initial GUI checks were blocked by a locked desktop (`GetCursorPos: access
  denied (0x80070005)`). After the user unlocked it, Windows Computer Use
  successfully exercised the sample. GUI evidence is listed separately below.

## Problems fixed

1. Windows cache and persistent local data resolved to the same directory.
   Cache now uses `local_data/cache`, so clearing it does not clear persistent
   data, logs, or the single-instance lock. A Windows regression test checks
   that persistent directories are not inside the cache directory.
2. The desktop-shell sample inherited GPUI's last-window-close exit policy on
   Windows and Linux, defeating tray residency. Successful tray installation
   now selects explicit quit mode. Failed tray installation retains the default
   exit policy. Left tray clicks now consume the existing click stream and
   reopen the window. Closing the last window and reopening by secondary launch
   were verified in the GUI; direct tray clicks remain unverified.
3. Git symlinks checked out as ordinary Windows files caused crate archives to
   contain `../../LICENSE` instead of the license text. The eleven affected
   crates now contain ordinary `LICENSE-APACHE` files. The package-check script
   checks their full contents against the root license as well as archive entry
   presence. SPDX license metadata remains unchanged.
4. Actual NSIS uninstallation left its protocol registration pointing to a removed
   executable. The pinned cargo-packager template used a preprocessor comparison
   for a registry value available only at runtime. The adapter now embeds a
   minimally corrected, attributed MIT-licensed template with a runtime ownership
   guard. Reinstallation/uninstallation confirmed both removal of the owned
   registration and preservation of a replacement handler. A regression test
   checks the runtime guard in the embedded template.
5. Windows CI previously omitted desktop-shell builds and the sidecar integration
   test target. Both are now included.
6. Closing and reopening the sample destroyed its native menu: Windows destroys
   an attached HMENU with its window, while the backend retained that shared
   handle. A UI-thread subclass now detaches before destruction and removes the
   backend subclass; an owned attachment also detaches when dropped. Real Win32
   regression coverage checks reopen, two simultaneous windows, owner drop, and
   duplicate attachment failure. GUI reopen and command invocation passed.
7. The sample's Discard action allowed closing but retained its dirty flag,
   causing another prompt after reopening. Approval now clears the example flag;
   GUI close/reopen confirmed it stays cleared.
8. GPUI 0.4.0's default DirectComposition path produced a transparent native
   menubar. Its supported `GPUI_DISABLE_DIRECT_COMPOSITION=1` rendering path
   displayed the menu correctly. The sample now selects this at startup unless
   explicitly overridden. The library does not mutate environment or renderers;
   this requirement is documented for other consumers. Changing the window style
   after creation did not fix it and that experimental change was removed.

## Automated checks

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --all-targets --all-features` | Passed on native MSVC |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Passed after fixes, without compiler warnings |
| `cargo test --workspace --all-features --locked` after all fixes | 440 passed, 0 failed, 1 pre-existing manual packaging test ignored |
| `cargo test -p cargo-guic --locked` after the final NSIS fix | 48 passed, 0 failed, the same manual packaging test ignored |
| `cargo test -p guic-native --all-features` after menu lifetime fixes | 21 unit tests and the Windows sidecar integration test passed |
| `cargo check -p guic --no-default-features` | Passed |
| `cargo check -p guic --no-default-features --features native` | Passed |
| `cargo test -p guic --no-default-features --doc` | 4 passed |
| `cargo doc --workspace --all-features --no-deps` | Passed |
| `cargo run -p xtask -- schema --check` | Passed |
| Default-feature desktop-shell and cargo-guic builds | Passed |
| PowerShell equivalents of `scripts/package-check.sh` | All eleven lists/content checks and four independent archive builds passed |
| Actual independent `.crate` license contents | All four equal the root license text |

No tests were disabled to pass validation. The pre-existing ignored test is
`manual_native_backend_candidate`; local NSIS construction and installation
were performed separately. Dependent crate archives still require prerequisite
publication, as already documented by the package-check script.
The complete workspace suite was rerun after the NSIS and GUI-discovered fixes.
Workspace Clippy, the default desktop-shell build, and the minimal native feature
check also passed. Real installer ownership tests were run for the NSIS fix.

New regression coverage executes native Windows APIs and processes:

- Eight simultaneous acquisitions elect exactly one owner and deliver all eight
  distinct requests; an invalid stale endpoint is replaced.
- Forwarding preserves Windows UTF-16, including unpaired surrogates, quoted
  arguments, an absolute working directory, and URL payloads.
- A helper is compiled and launched from a path containing spaces and Japanese
  characters. Empty arguments, embedded quotes, trailing backslashes, literal
  shell metacharacters, stdin, stdout, stderr, and exit code 23 are checked.
- Cancellation, timeout, and dropping the owner terminate the helper, its child,
  and its grandchild. Process handles are retained before cleanup to avoid PID
  reuse invalidating the assertions. Invalid working-directory startup fails.
- Existing tests cover menu routing enablement/scopes, lifecycle guards and
  ordering, backup recovery, geometry fitting, theme preferences, sidecar source
  changes and immutable snapshots, collision/path rejection, and signing policy.
  GPUI test-context results are not GUI acceptance.

## Runtime and local distribution checks

The debug desktop shell started and wrote window geometry in the platform user
directory. A secondary launch from another working directory, with spaces,
Japanese text, and a URL argument, returned zero while retaining the primary
endpoint. After unlock, the GUI status label also showed `space argument`,
Japanese text, a URL string in arguments, and the secondary process's temporary
working directory exactly as sent. The URL array remained empty, as expected
for command-line URL strings.

Two unsigned NSIS candidates were built: a small console fixture with a helper,
and desktop-shell with the same helper. Both were installed without elevation
into dedicated directories under `target/windows-validation/`. The console
fixture executed its adjacent helper from an unrelated working directory and
preserved the tested arguments. The installed desktop shell started from a path
containing spaces and Japanese text; its helper also executed successfully.
The test-only desktop identity was `org.guic.windows-validation-desktop`.

The desktop installer registered the isolated `guic-validation-smoke` scheme in
HKCU with a quoted executable path and `"%1"`. Launching that scheme through the
OS left a single installed application process. The forwarded URL was not
visually inspected. The regular `guic-example` registration was not replaced.

The installed primary was forcibly terminated, then restarted with its stale
endpoint still present. The replacement process remained alive, replaced the
endpoint, and acknowledged a secondary launch with exit code zero. Separately,
invalid primary preference/window JSON with valid backups allowed startup. A
backup rectangle at `[50000, -50000, 2000, 1000]` with no saved display was
restored and the live application saved `[560, 0, 2000, 1000]` on the available
display. This verifies actual geometry correction and backup loading, not a
physical monitor removal or a visual DPI check.

Both installers and uninstallers returned zero. Test application processes were
terminated and test state was retained separately with the validation artifacts.
The intentionally forced exits do not count as clean shutdown verification.
The initial desktop uninstaller returned zero despite leaving its URL registration
behind. After the template correction, an unchanged registration was removed.
A second installation with a deliberately replaced test handler preserved that
handler on uninstall. The dedicated test registration was then removed, and
the installed executable was absent. Exit codes alone were not used as proof
of correct uninstall behavior.

After the GUI fixes, the final source was rebuilt and packaged successfully as
another unsigned NSIS candidate (`gui-final-package-build.log`). The final
installer is under `artifacts/attempt-a0Ktti/nsis/output/` in the desktop artifact
root; its SHA-256 is
`8da3f232ee3a48a7f0813be1aecf86eac6b1587a4539f31f90adb1094f1b0648`.
This final candidate was not reinstalled; installation/uninstallation evidence
above applies to the preceding candidate with the same NSIS template fix.
An additional bundle call without an explicit record correctly rejected two
compatible records; the successful build command had already produced the final
installer. No ambiguous record was silently selected.

The final GUI build restarted successfully, reopened after closing its window,
and exited through its native Quit command. No validation app/helper process
remained. New GUI-session state was moved into the ignored
`target/windows-validation/captured-state/` directory, separately from the
earlier packaged-app evidence.

## Remaining acceptance work and constraints

- The GUI verified native menu opening and Show/Restart/Quit routing, dirty
  quit and close prompts, Keep working cancellation, Discard followed by close,
  reopening through activation, and normal quit. Light/Dark changes, saved Light
  and Dark restoration, System preference persistence, maximization, restart
  while maximized, and restoration to the prior windowed bounds were checked.
  Ordinary restarts replaced the old PID with one new application process.
- Checked/disabled menu states and registered keyboard shortcuts were initially
  absent from the sample; see the 2026-10-15 follow-up below. Tray clicks, menus,
  and icon removal were subsequently confirmed manually by the user; the
  automation surface did not expose a targetable notification-area window.
- Live following of OS appearance changes, physical DPI changes, and
  multiple-monitor/removal scenarios were not tested. Theme preference and
  geometry logic passed automated tests. The renderer fallback was checked on
  this GPU only; other drivers need acceptance testing.
- Direct OS shutdown/logoff was not performed. GPUI's bounded final quit hook
  cannot veto shutdown; application-managed guards are a separate path.
- Code inspection of GPUI 0.4.0's Windows restart implementation shows a
  PowerShell helper that interpolates the executable path into a double-quoted
  script string and launches without forwarding argv. Paths containing
  PowerShell interpolation characters and argument-dependent restart workflows
  remain a dependency-level concern. Ordinary restart passed, but this repository
  does not claim arbitrary-path/argv restart guarantees.
- Corrupt state without a valid backup returns an error; it is not silently
  discarded. Unknown state versions are also rejected.
- Real Authenticode signing and trust/timestamp verification were not performed.
  SDK signing tools and certificates exist on the machine, but no signing
  identity was selected or used. Signing authorization/order/failure behavior
  was checked through the existing fake-runner tests. Both candidates are
  explicitly unsigned and retain `acceptance: not-verified`.
- Startup failure after Job Object assignment and abrupt parent death with a
  live sidecar were not fault-injected. Ordinary sidecar drop/cancel/timeout
  cleanup was tested with real descendants.
- Independent package verification warns about the already-locked, yanked
  `spin` 0.9.8 dependency through `flume` / `guic-gpui-scheduler`. Dependency
  versions were not changed in this validation.
- WebView runtime distribution was excluded as requested. No macOS or Linux
  builds were rerun on this Windows machine.

Intermediate failures were distinguished from implementation defects: one
packaging record became incompatible after source/manifest changes during its
build, and a test rebuild could not replace a running cargo-guic executable on
Windows. Rebuilding from stable inputs and using a separate validation CLI copy
resolved these; the final test and package runs succeeded.

## GUI follow-up on 2026-10-15

The desktop-shell sample now exposes a checked command and a disabled command,
with an execution counter. Window-focused `Ctrl+Shift+T` and `Ctrl+Shift+D`
shortcuts resolve through the same router and handler as native commands. These
are not system-wide hotkeys or native menu accelerators. The sample gives its
content initial focus so key events reach that handler immediately after opening.

Direct Windows Computer Use observations:

- Initial state was unchecked with zero executions. `Ctrl+Shift+T` changed it
  to checked with exactly one execution.
- Opening the native menu showed a checkmark and a greyed-out disabled item;
  the accessibility tree also marked that item disabled.
- Clicking the disabled native item left the count at one and did not execute
  the disabled-command error sentinel.
- Clicking the checked item changed the model to unchecked with exactly two
  executions. `Ctrl+Shift+D` left both values unchanged and did not execute the
  sentinel.
- With the theme button focused, `Ctrl+Shift+T` executed once and
  `Ctrl+Shift+D` did not execute. After closing the window and reopening it
  through a secondary activation, the original process retained its state;
  the secondary process exited with code zero. The toggle shortcut worked
  immediately in the reopened window and incremented the counter once.
  Reopening the native menu confirmed that its checkmark was removed and the
  disabled item remained greyed out.

User-reported physical Windows checks (not agent-observed GUI automation):

- After closing the window, left-clicking the GUIC Desktop Shell tray icon
  reopened it.
- Right-clicking the tray and selecting Show window worked.
- Right-clicking and selecting Quit exited the application and removed its icon.

The user reported all three checks normal. The tray backend also opens its menu
on left click by default; no failure was reported, so that behavior was not
changed speculatively. The app was absent from the process list before the next
agent-run GUI check.

The debug build and targeted Clippy (`desktop-shell` and `guic-native`, all
targets/features, warnings denied) passed. The native suite passed 21 unit tests
and the Windows sidecar integration test. The core keyboard filter passed five
tests and the components menu filter passed sixteen tests. Formatting also
passed. Logs use the `menu-followup-` prefix
under `target/windows-validation/`. The 440-test workspace result above belongs
to the preceding validation; it has not been relabeled as a follow-up run.

No additional native-library defect was found in these follow-up checks.
The final instance exited through native Quit, with no remaining desktop-shell
process and no stderr output. Newly created settings were moved to the
`org.guic.desktop-shell-menu-followup-roaming` and
`org.guic.desktop-shell-menu-followup-local` directories under the ignored
`target/windows-validation/captured-state/` evidence directory. The follow-up
sample changes were tested in the debug build; the earlier NSIS candidate was
not rebuilt for these additional demonstration commands.
