use once_cell::sync::Lazy;
use std::time::Instant;

static MEASUREMENTS_ENABLED: Lazy<bool> = Lazy::new(|| std::env::var_os("GUIC_MEASURE").is_some());

/// Returns whether lightweight measurement logging is enabled.
pub fn measurements_enabled() -> bool {
    *MEASUREMENTS_ENABLED
}

/// Measures the execution time of a closure when measurement is enabled.
pub fn measure(name: &str, f: impl FnOnce()) {
    measure_if(name, true, f);
}

/// Measures the execution time of a closure when both the flag and environment
/// setting are enabled.
pub fn measure_if(name: &str, enabled: bool, f: impl FnOnce()) {
    if enabled && measurements_enabled() {
        let start = Instant::now();
        f();
        tracing::trace!("{name} finished in {:?}", start.elapsed());
    } else {
        f();
    }
}
