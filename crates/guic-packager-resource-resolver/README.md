# guic-packager-resource-resolver

Find resources installed by `guic-packager`.

```rust,no_run
use guic_packager_resource_resolver::{resources_dir, PackageFormat};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let directory = resources_dir(PackageFormat::Nsis)?;
# Ok(())
# }
```

The optional `auto-detect-format` feature reads `CARGO_PACKAGER_FORMAT` at compile
time. Build the application through `before_each_package_command` so the packager
sets that variable for each format. `before_packaging_command` does not set it.

GUIC applications using `guic-assets` do not need this crate for resource lookup.
See [UPSTREAM.md](UPSTREAM.md) for provenance. Licensed under Apache-2.0 OR MIT.
