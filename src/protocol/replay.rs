//! Replay file wire format (determinism roadmap phase 05,
//! `docs/plans/determinism-05-replays.md`).
//!
//! A replay file is a [`ReplayHeader`], then a stream of length-prefixed
//! postcard-encoded [`ReplayEntry`] values, the last of which is always
//! [`ReplayEntry::End`]. Writing and reading both stream entry-by-entry
//! (`src/engine_app/replay.rs`) — nothing here buffers a whole session in
//! memory.

use serde::{Deserialize, Serialize};

use crate::protocol::tick_input::TickInput;

/// File magic, checked first on open.
pub const REPLAY_MAGIC: [u8; 4] = *b"ABRR";
/// Bumped whenever [`ReplayHeader`]/[`ReplayEntry`]'s shape or semantics
/// change in a way that breaks reading an older file. A replay recorded
/// under a different version is refused, not best-effort parsed.
pub const REPLAY_FORMAT_VERSION: u32 = 2;

/// A stable, version-controlled FNV-1a-style hash mixer.
///
/// Used for both the per-tick world-state hash
/// (`crate::systems::state_hash::hash_world_state`) and this module's
/// [`config_digest`] — a hand-rolled algorithm under our own versioning
/// discipline, not `rustc_hash::FxHasher` (whose algorithm isn't a stability
/// contract; it changed between rustc-hash 1.x and 2.x) or `std`'s
/// `RandomState` (unseeded, not reproducible run to run).
pub(crate) struct ReplayHasher(u64);

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl ReplayHasher {
    pub(crate) fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    pub(crate) fn write_u8(&mut self, v: u8) {
        self.write_bytes(&[v]);
    }

    pub(crate) fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub(crate) fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    pub(crate) fn write_i32(&mut self, v: i32) {
        self.write_u32(v as u32);
    }

    pub(crate) fn write_f32(&mut self, v: f32) {
        // Bit-exact scope (same build/arch, plain Rust f32) -- never
        // epsilon-compare, hash the raw bits including NaN payload/sign.
        self.write_u32(v.to_bits());
    }

    pub(crate) fn write_bool(&mut self, v: bool) {
        self.write_u8(v as u8);
    }

    /// Length-prefixed so `write_str("ab") + write_str("c")` can't collide
    /// with `write_str("a") + write_str("bc")`.
    pub(crate) fn write_str(&mut self, s: &str) {
        self.write_u64(s.len() as u64);
        self.write_bytes(s.as_bytes());
    }

    pub(crate) fn finish(&self) -> u64 {
        self.0
    }
}

/// Written once at the start of a replay file. Validated against the
/// engine/config it's being replayed into before any [`ReplayEntry`] is
/// read — see `validate_replay_header` (`src/engine_app/replay.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub magic: [u8; 4],
    pub format_version: u32,
    /// `env!("CARGO_PKG_VERSION")` at record time. v1 simplification — no
    /// git-hash plumbing; cross-build replay compatibility is out of scope
    /// (`docs/plans/determinism-00-overview.md`'s "same build, same arch").
    pub engine_build_id: String,
    pub seed: u64,
    pub sim_hz: f64,
    /// Hash of the sim-visible `GameConfig` fields (see [`config_digest`]).
    pub config_digest: u64,
    /// Initial scene name, `""` if the game doesn't use `SceneManager`.
    pub scene_id: String,
    /// User-supplied string, diagnostics only (not validated).
    pub game_version: String,
}

/// One entry in a replay's body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplayEntry {
    /// Run-length of consecutive empty (`TickInput::is_empty()`) ticks.
    EmptyRun(u32),
    /// One non-empty tick's full `TickInput`.
    Tick(TickInput),
    /// A periodic state-hash checkpoint (see `checkpoint_interval_ticks`,
    /// `src/engine_app/replay.rs`).
    Checkpoint { tick: u64, hash: u64 },
    /// Always the last entry: the session summary, written by
    /// `ReplayRecorder::finish`. Reading it is how playback recognizes a
    /// clean end of file — a separate trailer *type* would need its own
    /// framing discriminator to be told apart from an entry, so it lives in
    /// this enum instead.
    End {
        total_ticks: u64,
        final_hash: u64,
        /// Whether the recording session tripped
        /// [`DeterminismTaint`](crate::resources::determinism_taint::DeterminismTaint)
        /// — i.e. this file is not guaranteed bit-exact reproducible, and a
        /// divergence report from replaying it may be explained by that
        /// rather than by a real regression.
        tainted: bool,
    },
}

/// Sim-visible `GameConfig` fields, hashed in this fixed order via
/// [`ReplayHasher`]. Only fields that actually affect sim behavior belong
/// here — e.g. `window_title`/`vsync`/`fullscreen` are render-only and
/// excluded.
pub fn config_digest(config: &crate::resources::gameconfig::GameConfig) -> u64 {
    let mut h = ReplayHasher::new();
    h.write_u64(config.sim_hz.to_bits());
    h.write_u32(config.snapshot_skip);
    h.write_f32(config.gamepad_deadzone);
    h.write_u32(config.render_width);
    h.write_u32(config.render_height);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hasher_is_deterministic() {
        let mut a = ReplayHasher::new();
        a.write_u64(42);
        a.write_str("hello");
        a.write_f32(1.5);

        let mut b = ReplayHasher::new();
        b.write_u64(42);
        b.write_str("hello");
        b.write_f32(1.5);

        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn hasher_distinguishes_str_boundary() {
        let mut a = ReplayHasher::new();
        a.write_str("ab");
        a.write_str("c");

        let mut b = ReplayHasher::new();
        b.write_str("a");
        b.write_str("bc");

        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn hasher_nan_bits_are_distinguishable_from_zero() {
        let mut a = ReplayHasher::new();
        a.write_f32(f32::NAN);

        let mut b = ReplayHasher::new();
        b.write_f32(0.0);

        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn config_digest_changes_with_sim_hz() {
        let mut c1 = crate::resources::gameconfig::GameConfig::new();
        c1.sim_hz = 240.0;
        let mut c2 = crate::resources::gameconfig::GameConfig::new();
        c2.sim_hz = 120.0;
        assert_ne!(config_digest(&c1), config_digest(&c2));
    }
}
