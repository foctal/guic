# Application Persistence

`guic_core::JsonStore` stores settings, recent files, workspace metadata, and
per-window state as human-readable JSON.

```rust
use guic_core::{JsonStore, LoadSource};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
struct Settings {
    theme: String,
}

let store = JsonStore::new("state/settings.json");
let settings = store.load_or_default::<Settings>()?;
store.save(&settings)?;

if let Some(recovered) = store.load_recovering::<Settings>()? {
    if recovered.source == LoadSource::Backup {
        // Tell the user that the previous generation was recovered.
    }
}
# Ok::<(), guic_core::PersistenceError>(())
```

Each save serializes to a unique adjacent temporary file, flushes and
synchronizes it, moves the previous primary value to `.bak`, and replaces the
primary file. On Unix, the parent directory is synchronized after replacement.
On platforms without a safe standard-library directory sync, the backup remains
the recovery boundary.

Use one writer per store. Applications should serialize concurrent save
requests, keep credentials in an OS credential vault rather than JSON, and
avoid persisting live process handles or terminal scrollback as session state.
