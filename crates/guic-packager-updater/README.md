# guic-packager-updater

Blocking update checks, Minisign verification, and native installation for
applications packaged with `guic-packager`. GUIC exposes this crate through
`guic::updater` when its `updater` feature is enabled.

```rust,no_run
use guic_packager_updater::{check_update, Config};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let config = Config {
    endpoints: vec!["https://updates.example.com/latest.json".parse()?],
    pubkey: "YOUR_BASE64_ENCODED_PUBLIC_KEY".into(),
    ..Default::default()
};
if let Some(update) = check_update("1.0.0".parse()?, config)? {
    update.download_and_install()?;
}
# Ok(())
# }
```

Run these blocking operations outside the GUI event loop. Serve feeds and payloads
via HTTPS. Release builds reject HTTP unless explicitly enabled with
`dangerous_insecure_transport_protocol`; debug builds allow it for local testing.
`download` verifies signatures. `install` expects already verified bytes.

On macOS, the updater validates the archive, stages on the destination volume, and
atomically swaps the app bundle. If permissions require administrator access, it
uses the system authorization dialog. Cancellation and pre-swap failures leave
the installed bundle unchanged. Filesystems without atomic swap support fail
without a destructive fallback. Authorization requires UTF-8 paths. Validate
signed/notarized application updates and relaunch behavior before shipping.

Windows supports NSIS and WiX installers; Linux requires AppImage. Installation
and restart behavior must be tested with the application's actual distribution.

## Migration from cargo-packager-updater 0.2.3

`guic::updater` and the update feed format are retained. The fork uses the supplied
newer upstream API: `Config` adds `dangerous_insecure_transport_protocol`.
Existing struct literals should add this field as `false`, or use
`..Default::default()`. The re-exported HTTP client is now reqwest 0.13.
Types from the upstream crate and this fork are not interchangeable. Import
updater types consistently through `guic::updater` or `guic_packager_updater`.

The macOS authorization-dialog test is ignored by default and requires an
interactive desktop. It uses a temporary application bundle.

See [UPSTREAM.md](UPSTREAM.md) for provenance. Licensed under Apache-2.0 OR MIT.

Include `THIRD-PARTY-LICENSES.txt` with binary distributions containing the
certificate data dependencies.
