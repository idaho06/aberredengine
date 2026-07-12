//! Global shutdown flag + panic hook (Phase 7a).
//!
//! Today's shutdown is purely message-based: `RenderMsg::Quit`,
//! `LogicMsg::Shutdown`, and `AudioCmd::Shutdown` remain the primary,
//! ordering-preserving path (they let each thread drain intents / tear down
//! audio / drop `LuaRuntime` before exiting) and this module does not
//! replace them. The gap this module closes is the *emergency* path: a
//! render-thread panic is only detected indirectly when a channel send/recv
//! starts failing, a logic-thread panic only when `tx_logic.send()` fails on
//! the render side, and an **audio-thread panic is detected by nobody** — the
//! logic thread keeps sending commands into a dead channel forever. Every
//! thread's loop should also check [`running`] so any panic, anywhere, flips
//! one flag that unwinds all three loops promptly.

use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

/// Chain onto the default panic hook (so panic messages/backtraces still
/// print) and additionally flip the shutdown flag. Call once, at the top of
/// `EngineBuilder::try_run`.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        stop();
    }));
}

/// Whether the engine should keep running. Checked by each thread's main
/// loop alongside its own message-based shutdown condition.
pub fn running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Request shutdown of all threads. Called by the panic hook; may also be
/// called directly for a non-panic emergency stop.
pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
}
