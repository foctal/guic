# Packaging Native Applications

GUIC does not publish, sign, notarize, or create installers. Those operations
remain application-owner responsibilities.

## Current Distribution Boundary

GUIC pins `guic-gpui` and `guic-gpui-platform` 0.2.0 from crates.io. These
packages contain the accessibility and platform APIs used by GUIC, so GPUI no
longer prevents GUIC crates from being packaged. Every public GUIC crate must
still pass a package dry run and metadata review before the maintainer publishes
it.

## macOS

1. Build a release binary on the minimum supported macOS version or compatible
   CI image.
2. Assemble an `.app` bundle with `Info.plist`, icons, embedded assets, and any
   required usage descriptions.
3. Sign the bundle and nested executables with the product's Developer ID.
4. Submit for notarization, staple the result, and verify with Gatekeeper on a
   clean machine.
5. Package as a signed DMG or installer package according to product policy.

## Windows

1. Build the release binary for the intended MSVC target.
2. Include icons, version resources, runtime assets, and any WebView runtime
   policy required by enabled features.
3. Sign binaries and the installer with the product's code-signing identity.
4. Create an MSIX, MSI, or another organization-approved installer.
5. Test install, upgrade, uninstall, ConPTY, long paths, non-ASCII user paths,
   and a standard non-administrator account.

## Linux

1. Build on a distribution compatible with the oldest supported glibc baseline.
2. Declare GPUI runtime libraries and optional WebKitGTK dependencies.
3. Package using the target ecosystem's native format, or use an AppImage or
   Flatpak with an explicit sandbox/filesystem policy.
4. Verify both Wayland and X11 sessions where supported, desktop entries,
   icons, MIME handlers, font fallback, and clean uninstall.

Run `./scripts/release-check.sh` before creating artifacts, then complete the
manual matrix in [Platform Support](platform-support.md).
