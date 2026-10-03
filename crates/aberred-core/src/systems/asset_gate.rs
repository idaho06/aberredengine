//! [`AssetGate`]: keeps deterministic `Playing` from changing the set of
//! loaded assets.
//!
//! Asset I/O finishes at wall-clock times, so in a `.deterministic()` game
//! every asset loads during `Setup`. While `Playing`, a command that would
//! change the set of loaded assets is rejected; reloading a key that is
//! already loaded is dropped as a no-op; everything else (texture filters,
//! playback) passes.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rustc_hash::FxHashSet;
use std::sync::Arc;

use crate::protocol::asset_kind::AssetKind;
use crate::protocol::audio::AudioCmd;
use crate::protocol::render_assets::RenderAssetCmd;
use crate::resources::deterministic_mode::DeterministicMode;
use crate::resources::gamestate::{GameState, GameStates};
use crate::resources::loaded_assets::LoadedAssets;
use crate::resources::warn_once::first_seen;

/// What happens to an asset command during deterministic `Playing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Sent to its loading thread as usual.
    Forward,
    /// Changes nothing (a load of a loaded key, a removal of a missing one):
    /// dropped silently.
    NoOp,
    /// Would change the set of loaded assets: dropped and reported.
    Rejected,
}

/// The [`Verdict`] for a render command during deterministic `Playing`.
pub fn render_verdict(cmd: &RenderAssetCmd, loaded: &LoadedAssets) -> Verdict {
    if let Some((kind, key)) = cmd.load_target() {
        return load_verdict(loaded.contains(kind, key));
    }
    match cmd {
        RenderAssetCmd::SetTextureFilter { .. } => Verdict::Forward,
        RenderAssetCmd::RemoveTexture { key }
        | RenderAssetCmd::RenameTexture { old_key: key, .. } => {
            change_verdict(loaded.contains(AssetKind::Texture, key))
        }
        RenderAssetCmd::RemoveFont { key } | RenderAssetCmd::RenameFont { old_key: key, .. } => {
            change_verdict(loaded.contains(AssetKind::Font, key))
        }
        RenderAssetCmd::Texture { .. }
        | RenderAssetCmd::TextureFromMemory { .. }
        | RenderAssetCmd::TilemapTexture { .. }
        | RenderAssetCmd::RasterizeText { .. }
        | RenderAssetCmd::Font { .. }
        | RenderAssetCmd::Shader { .. }
        | RenderAssetCmd::ShaderFromMemory { .. } => {
            unreachable!("load commands have a load_target")
        }
    }
}

/// The [`Verdict`] for an audio command during deterministic `Playing`.
pub fn audio_verdict(cmd: &AudioCmd, loaded: &LoadedAssets) -> Verdict {
    if let Some((kind, key)) = cmd.load_target() {
        return load_verdict(loaded.contains(kind, key));
    }
    match cmd {
        AudioCmd::UnloadFx { id } => change_verdict(loaded.contains(AssetKind::Sound, id)),
        AudioCmd::UnloadMusic { id } => change_verdict(loaded.contains(AssetKind::Music, id)),
        AudioCmd::UnloadAllFx => change_verdict(loaded.any(AssetKind::Sound)),
        AudioCmd::UnloadAllMusic => change_verdict(loaded.any(AssetKind::Music)),
        _ => Verdict::Forward,
    }
}

/// A load changes the loaded set unless its key is already loaded.
fn load_verdict(already_loaded: bool) -> Verdict {
    if already_loaded {
        Verdict::NoOp
    } else {
        Verdict::Rejected
    }
}

/// A removal, rename or unload changes the loaded set only if its target is
/// loaded.
fn change_verdict(target_loaded: bool) -> Verdict {
    if target_loaded {
        Verdict::Rejected
    } else {
        Verdict::NoOp
    }
}

/// A short description of a render command for logs.
pub fn render_cmd_label(cmd: &RenderAssetCmd) -> String {
    match cmd {
        RenderAssetCmd::RemoveTexture { key } => format!("removal of Texture '{key}'"),
        RenderAssetCmd::RemoveFont { key } => format!("removal of Font '{key}'"),
        RenderAssetCmd::RenameTexture { old_key, .. } => format!("rename of Texture '{old_key}'"),
        RenderAssetCmd::RenameFont { old_key, .. } => format!("rename of Font '{old_key}'"),
        _ => load_label(cmd.load_target(), "render asset command"),
    }
}

/// A short description of an audio command for logs.
pub fn audio_cmd_label(cmd: &AudioCmd) -> String {
    match cmd {
        AudioCmd::UnloadFx { id } => format!("unload of Sound '{id}'"),
        AudioCmd::UnloadMusic { id } => format!("unload of Music '{id}'"),
        AudioCmd::UnloadAllFx => "unload of all Sound".to_owned(),
        AudioCmd::UnloadAllMusic => "unload of all Music".to_owned(),
        _ => load_label(cmd.load_target(), "audio command"),
    }
}

fn load_label(target: Option<(AssetKind, &str)>, other: &str) -> String {
    match target {
        Some((kind, key)) => format!("load of {kind:?} '{key}'"),
        None => other.to_owned(),
    }
}

impl Verdict {
    /// For a forwarder: whether to send the command on. A rejected command
    /// came from a raw `MessageWriter`, which has no return value to carry
    /// the error, so it panics in debug builds and logs an `error!` once per
    /// `label` in release builds. `AssetLoader` checks the gate itself and
    /// returns `Err` instead, so its commands are never rejected here.
    pub fn admit(self, label: impl FnOnce() -> String, reported: &mut FxHashSet<Arc<str>>) -> bool {
        match self {
            Verdict::Forward => true,
            Verdict::NoOp => false,
            Verdict::Rejected => {
                let label = label();
                let message = || {
                    format!(
                        "{label} rejected: deterministic Playing can't change the loaded \
                         assets. Load every asset during Setup."
                    )
                };
                if cfg!(debug_assertions) {
                    panic!("{}", message());
                }
                if first_seen(reported, &label) {
                    log::error!("{}", message());
                }
                false
            }
        }
    }
}

/// Decides each asset command's [`Verdict`]: always [`Verdict::Forward`]
/// unless the game is deterministic and `Playing`.
#[derive(SystemParam)]
pub struct AssetGate<'w> {
    deterministic: Option<Res<'w, DeterministicMode>>,
    state: Res<'w, GameState>,
    loaded: Res<'w, LoadedAssets>,
}

impl AssetGate<'_> {
    fn closed(&self) -> bool {
        self.deterministic.is_some() && *self.state.get() == GameStates::Playing
    }

    /// The verdict for a render command.
    pub fn render(&self, cmd: &RenderAssetCmd) -> Verdict {
        if self.closed() {
            render_verdict(cmd, &self.loaded)
        } else {
            Verdict::Forward
        }
    }

    /// The verdict for an audio command.
    pub fn audio(&self, cmd: &AudioCmd) -> Verdict {
        if self.closed() {
            audio_verdict(cmd, &self.loaded)
        } else {
            Verdict::Forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::asset_kind::AssetChange;
    use crate::resources::texturefilter::TextureFilter;

    fn s(v: &str) -> String {
        v.to_owned()
    }

    fn loaded_with(kind: AssetKind, key: &str) -> LoadedAssets {
        let mut loaded = LoadedAssets::default();
        loaded.apply(AssetChange::Loaded { kind, key: s(key) });
        loaded
    }

    fn texture(key: &str) -> RenderAssetCmd {
        RenderAssetCmd::Texture {
            key: s(key),
            path: s("p.png"),
            filter: TextureFilter::Nearest,
        }
    }

    #[test]
    fn render_loads_pass_only_as_no_ops_for_loaded_keys() {
        let loaded = loaded_with(AssetKind::Texture, "player");
        assert_eq!(render_verdict(&texture("player"), &loaded), Verdict::NoOp);
        assert_eq!(
            render_verdict(&texture("enemy"), &loaded),
            Verdict::Rejected
        );
        let font = RenderAssetCmd::Font {
            key: s("player"),
            path: s("f.ttf"),
            size: 8,
            skip_if_loaded: true,
        };
        assert_eq!(
            render_verdict(&font, &loaded),
            Verdict::Rejected,
            "kinds don't mix"
        );
    }

    #[test]
    fn render_changes_to_loaded_assets_are_rejected_and_filters_pass() {
        let mut loaded = loaded_with(AssetKind::Texture, "player");
        loaded.apply(AssetChange::Loaded {
            kind: AssetKind::Font,
            key: s("arcade"),
        });
        for cmd in [
            RenderAssetCmd::RemoveTexture { key: s("player") },
            RenderAssetCmd::RemoveFont { key: s("arcade") },
            RenderAssetCmd::RenameTexture {
                old_key: s("player"),
                new_key: s("hero"),
            },
            RenderAssetCmd::RenameFont {
                old_key: s("arcade"),
                new_key: s("b"),
            },
        ] {
            assert_eq!(render_verdict(&cmd, &loaded), Verdict::Rejected, "{cmd:?}");
        }
        let filter = RenderAssetCmd::SetTextureFilter {
            key: s("player"),
            filter: TextureFilter::Bilinear,
        };
        assert_eq!(render_verdict(&filter, &loaded), Verdict::Forward);
    }

    /// Removing or renaming something that isn't loaded changes nothing (a
    /// closing dynamic-text menu removes label textures it never created).
    #[test]
    fn changes_to_assets_that_are_not_loaded_are_no_ops() {
        let loaded = LoadedAssets::default();
        for cmd in [
            RenderAssetCmd::RemoveTexture { key: s("menu_0") },
            RenderAssetCmd::RemoveFont { key: s("arcade") },
            RenderAssetCmd::RenameFont {
                old_key: s("a"),
                new_key: s("b"),
            },
        ] {
            assert_eq!(render_verdict(&cmd, &loaded), Verdict::NoOp, "{cmd:?}");
        }
        for cmd in [
            AudioCmd::UnloadFx { id: s("jump") },
            AudioCmd::UnloadAllFx,
            AudioCmd::UnloadAllMusic,
        ] {
            assert_eq!(audio_verdict(&cmd, &loaded), Verdict::NoOp, "{cmd:?}");
        }
    }

    #[test]
    fn audio_loads_and_unloads_are_gated_and_playback_passes() {
        let loaded = loaded_with(AssetKind::Sound, "jump");
        let load = |id: &str| AudioCmd::LoadFx {
            id: s(id),
            path: s("j.wav"),
        };
        assert_eq!(audio_verdict(&load("jump"), &loaded), Verdict::NoOp);
        assert_eq!(audio_verdict(&load("coin"), &loaded), Verdict::Rejected);
        for cmd in [AudioCmd::UnloadFx { id: s("jump") }, AudioCmd::UnloadAllFx] {
            assert_eq!(audio_verdict(&cmd, &loaded), Verdict::Rejected, "{cmd:?}");
        }
        for cmd in [
            AudioCmd::PlayFx { id: s("jump") },
            AudioCmd::StopAllFx,
            AudioCmd::PlayMusic {
                id: s("bgm"),
                looped: true,
            },
            AudioCmd::VolumeMusic {
                id: s("bgm"),
                vol: 0.5,
            },
        ] {
            assert_eq!(audio_verdict(&cmd, &loaded), Verdict::Forward, "{cmd:?}");
        }
    }
}
