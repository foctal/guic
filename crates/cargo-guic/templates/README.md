# NSIS template compatibility

`installer.nsi` comes from cargo-packager 0.11.8 at
`src/package/nsis/installer.nsi`, licensed under the adjacent `LICENSE-MIT`.
The backend dependency is pinned to that version.

GUIC replaces the protocol uninstallation block's preprocessor `!if` / `!endif`
with runtime LogicLib `${If}` / `${EndIf}`. A registry value read during uninstall
cannot be compared at installer compile time. The runtime guard removes a
protocol only if its command still points to this installation, preserving a
replacement handler registered by another application.

The adapter writes the embedded template to a temporary file in each operation's
output directory and retains it until packaging finishes. It does not modify the
Cargo registry source or accept arbitrary user-supplied installer templates.

On backend upgrades, compare this file with the new upstream template and keep
the runtime ownership guard. Validate both ownership cases with a dedicated
per-user test protocol and an isolated installation directory; never use an
existing user's protocol registration for this test.
