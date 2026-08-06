//! Raylib trace-log level parsing from `RUST_LOG`. Pure string/env parsing,
//! no ECS or window dependency -- used only by [`crate::bootstrap::setup_window`].

use raylib::ffi::TraceLogLevel;

pub(crate) fn raylib_log_level_from_env() -> TraceLogLevel {
    std::env::var("RUST_LOG")
        .ok()
        .as_deref()
        .map(raylib_log_level_from_rust_log)
        .unwrap_or(TraceLogLevel::LOG_INFO)
}

pub fn raylib_log_level_from_rust_log(rust_log: &str) -> TraceLogLevel {
    let default_directive = rust_log
        .split(',')
        .map(str::trim)
        .find(|directive| !directive.is_empty() && !directive.contains('='));

    let level = default_directive
        .and_then(|directive| directive.split('/').next())
        .map(|directive| directive.trim().to_ascii_lowercase());

    match level.as_deref() {
        Some("trace") => TraceLogLevel::LOG_TRACE,
        Some("debug") => TraceLogLevel::LOG_DEBUG,
        Some("info") => TraceLogLevel::LOG_INFO,
        Some("warn") | Some("warning") => TraceLogLevel::LOG_WARNING,
        Some("error") => TraceLogLevel::LOG_ERROR,
        Some("off") => TraceLogLevel::LOG_NONE,
        _ => TraceLogLevel::LOG_INFO,
    }
}
