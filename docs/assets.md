# Assets

`guic-assets` provides a filesystem-backed GPUI asset source:

- `FileAssetSource` implements GPUI's `AssetSource` trait for local directories
- `AssetManifest` can register typed asset metadata and load registered files
- `register_directory()` helps bulk-register static assets for application use
- `register_directory_recursive()` supports nested packaged directories
- `register_directory_inferred()` infers `AssetKind` values from file extensions
- `register_manifest_json()` and `register_manifest_file()` load packaged manifests

See `examples/assets_demo` for a runnable example.
