//! Shared "warn once per key" bookkeeping.
//!
//! Several resources (e.g. [`GuiThemeWarnCache`](crate::resources::guitheme::GuiThemeWarnCache),
//! [`FontMetricsWarnCache`](crate::resources::fontmetrics::FontMetricsWarnCache))
//! want to log a warning the first time a string key is reported
//! missing/invalid, then stay silent for that key on every subsequent call —
//! this is the shared comparison/insert logic behind `warn_once`-style
//! methods on those resources.

use std::sync::Arc;

use rustc_hash::FxHashSet;

/// Returns `true` the first time `key` is passed for a given `set`, `false`
/// on every subsequent call with the same `key`.
pub(crate) fn first_seen(set: &mut FxHashSet<Arc<str>>, key: &str) -> bool {
    if set.contains(key) {
        return false;
    }
    set.insert(Arc::from(key));
    true
}
