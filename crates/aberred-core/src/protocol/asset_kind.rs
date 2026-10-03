//! [`AssetKind`]: which kind of asset a load command or load reply refers to.

/// The kind of a loaded asset. Each kind has its own key space: a texture and
/// a font may share a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    /// A texture, loaded on the render thread.
    Texture,
    /// A font, loaded on the render thread.
    Font,
    /// A shader, loaded on the render thread.
    Shader,
    /// A sound effect, loaded on the audio thread.
    Sound,
    /// A music stream, loaded on the audio thread.
    Music,
}

/// What one load reply settles: the asset's kind and key, and the error text
/// when the load failed. Returned by `LogicMsg::load_reply` and
/// `AudioMessage::load_reply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOutcome {
    /// The kind of the asset the load was for.
    pub kind: AssetKind,
    /// The key the asset is (or was to be) stored under.
    pub key: String,
    /// `None` when the load succeeded.
    pub error: Option<String>,
}

impl LoadOutcome {
    /// A successful load of `key`.
    pub fn loaded(kind: AssetKind, key: &str) -> Self {
        Self {
            kind,
            key: key.to_owned(),
            error: None,
        }
    }

    /// A failed load of `key`.
    pub fn failed(kind: AssetKind, key: &str, error: &str) -> Self {
        Self {
            kind,
            key: key.to_owned(),
            error: Some(error.to_owned()),
        }
    }
}

/// How one reply from the render or audio thread changes the set of loaded
/// assets. Returned by `LogicMsg::asset_change` and
/// `AudioMessage::asset_change`; applied by `LoadedAssets::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetChange {
    /// `key` finished loading.
    Loaded { kind: AssetKind, key: String },
    /// `key` was removed or unloaded.
    Removed { kind: AssetKind, key: String },
    /// A loaded `old_key` now lives under `new_key`.
    Renamed {
        kind: AssetKind,
        old_key: String,
        new_key: String,
    },
    /// Every asset of `kind` was unloaded.
    Cleared(AssetKind),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::audio::{AudioCmd, AudioMessage};
    use crate::protocol::render_assets::RenderAssetCmd;
    use crate::protocol::render_logic::LogicMsg;
    use crate::resources::texturefilter::TextureFilter;

    fn k() -> String {
        "k".to_owned()
    }

    /// Each load command's kind matches the kind of the reply that settles it.
    #[test]
    fn every_load_command_is_settled_by_a_reply_of_the_same_kind() {
        let render = [
            (
                RenderAssetCmd::Texture {
                    key: k(),
                    path: k(),
                    filter: TextureFilter::Nearest,
                },
                LogicMsg::TextureLoaded {
                    key: k(),
                    width: 1,
                    height: 1,
                },
            ),
            (
                RenderAssetCmd::Font {
                    key: k(),
                    path: k(),
                    size: 8,
                    skip_if_loaded: false,
                },
                LogicMsg::FontLoaded {
                    key: k(),
                    metrics: Default::default(),
                },
            ),
            (
                RenderAssetCmd::Shader {
                    key: k(),
                    vs_path: None,
                    fs_path: None,
                },
                LogicMsg::ShaderLoaded { key: k() },
            ),
        ];
        for (cmd, reply) in &render {
            let (kind, key) = cmd.load_target().unwrap();
            assert_eq!(
                reply.load_reply(),
                Some(LoadOutcome::loaded(kind, key)),
                "{cmd:?}"
            );
        }

        let audio = [
            (
                AudioCmd::LoadFx { id: k(), path: k() },
                AudioMessage::FxLoaded { id: k() },
                AudioMessage::FxLoadFailed {
                    id: k(),
                    error: "e".into(),
                },
            ),
            (
                AudioCmd::LoadMusic { id: k(), path: k() },
                AudioMessage::MusicLoaded { id: k() },
                AudioMessage::MusicLoadFailed {
                    id: k(),
                    error: "e".into(),
                },
            ),
        ];
        for (cmd, ok, failed) in &audio {
            let (kind, key) = cmd.load_target().unwrap();
            assert_eq!(
                ok.load_reply(),
                Some(LoadOutcome::loaded(kind, key)),
                "{cmd:?}"
            );
            assert_eq!(
                failed.load_reply(),
                Some(LoadOutcome::failed(kind, key, "e")),
                "{cmd:?}"
            );
        }
    }

    #[test]
    fn replies_map_to_their_change_in_loaded_assets() {
        use AssetChange::*;
        use AssetKind::*;
        let cases = [
            (
                LogicMsg::FontLoaded {
                    key: k(),
                    metrics: Default::default(),
                }
                .asset_change(),
                Some(Loaded {
                    kind: Font,
                    key: k(),
                }),
            ),
            (
                LogicMsg::ShaderLoaded { key: k() }.asset_change(),
                Some(Loaded {
                    kind: Shader,
                    key: k(),
                }),
            ),
            (
                LogicMsg::TextureRemoved { key: k() }.asset_change(),
                Some(Removed {
                    kind: Texture,
                    key: k(),
                }),
            ),
            (
                LogicMsg::FontRemoved { key: k() }.asset_change(),
                Some(Removed {
                    kind: Font,
                    key: k(),
                }),
            ),
            (
                LogicMsg::FontRenamed {
                    old_key: k(),
                    new_key: "n".into(),
                }
                .asset_change(),
                Some(Renamed {
                    kind: Font,
                    old_key: k(),
                    new_key: "n".into(),
                }),
            ),
            (
                LogicMsg::AssetLoadFailed {
                    kind: Texture,
                    key: k(),
                    error: "e".into(),
                }
                .asset_change(),
                None,
            ),
            (
                AudioMessage::MusicLoaded { id: k() }.asset_change(),
                Some(Loaded {
                    kind: Music,
                    key: k(),
                }),
            ),
            (
                AudioMessage::FxUnloaded { id: k() }.asset_change(),
                Some(Removed {
                    kind: Sound,
                    key: k(),
                }),
            ),
            (
                AudioMessage::FxUnloadedAll.asset_change(),
                Some(Cleared(Sound)),
            ),
            (
                AudioMessage::MusicUnloadedAll.asset_change(),
                Some(Cleared(Music)),
            ),
            (AudioMessage::MusicFinished { id: k() }.asset_change(), None),
        ];
        for (actual, expected) in cases {
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn a_render_failure_settles_the_kind_it_names() {
        let failed = LogicMsg::AssetLoadFailed {
            kind: AssetKind::Shader,
            key: k(),
            error: "bad".into(),
        };
        assert_eq!(
            failed.load_reply(),
            Some(LoadOutcome::failed(AssetKind::Shader, "k", "bad"))
        );
    }
}
