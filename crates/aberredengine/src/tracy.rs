/// Create a named Tracy span that lives until the binding is dropped.
///
/// Usage: `tracy_span!("my_system");`  — place at the top of the function.
/// Expands to nothing when `feature = "tracy"` is not active.
macro_rules! tracy_span {
    ($name:literal) => {
        #[cfg(feature = "tracy")]
        let _tracy_span = ::tracy_client::span!($name);
    };
}

/// Emit a Tracy frame-mark (signals the end of a logical frame).
///
/// Call once per iteration of the main loop, after all systems have run.
/// Expands to nothing when `feature = "tracy"` is not active.
macro_rules! tracy_frame_mark {
    () => {
        #[cfg(feature = "tracy")]
        ::tracy_client::frame_mark();
    };
}

pub(crate) use tracy_frame_mark;
pub(crate) use tracy_span;
