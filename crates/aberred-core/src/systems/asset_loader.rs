//! [`AssetLoader`]: one system param for queuing render and audio asset loads.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::protocol::asset_kind::AssetKind;
use crate::protocol::audio::AudioCmd;
use crate::protocol::render_assets::RenderAssetCmd;
use crate::resources::fontmetrics::FontMetricsStore;
use crate::resources::texturedims::TextureDimsStore;
use crate::resources::texturefilter::TextureFilter;
use crate::systems::asset_gate::{AssetGate, Verdict};

/// Why [`AssetLoader`] refused a load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// A `.deterministic()` game asked for an asset that isn't loaded while
    /// `Playing`. Load every asset during `Setup`.
    AssetChangeDuringDeterministicPlay {
        /// The kind of asset requested.
        kind: AssetKind,
        /// The key requested.
        key: String,
    },
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssetChangeDuringDeterministicPlay { kind, key } => write!(
                f,
                "{kind:?} '{key}' isn't loaded, and a deterministic game can't load \
                 assets while Playing; load it during Setup"
            ),
        }
    }
}

impl std::error::Error for AssetError {}

/// Queues texture, font, shader, sound and music loads from any logic-side
/// system, and reports which textures and fonts have finished loading.
///
/// Loads are asynchronous: textures, fonts and shaders load on the render
/// thread, sounds and music on the audio thread. A key queued this tick
/// reports `false` from [`is_texture_loaded`](Self::is_texture_loaded) /
/// [`is_font_loaded`](Self::is_font_loaded) until the render thread replies,
/// usually a tick or two later.
///
/// Render assets and audio assets have separate key spaces: a texture and a
/// sound may share a key without clashing.
///
/// In a `.deterministic()` game, every asset loads during `Setup`. While
/// `Playing`, `load_*` returns `Ok` without queuing anything for a key that
/// is already loaded, and [`AssetError::AssetChangeDuringDeterministicPlay`]
/// for one that isn't. Outside deterministic mode, `load_*` always queues
/// and returns `Ok`.
///
/// `AssetLoader` holds the writers for [`RenderAssetCmd`] and [`AudioCmd`], so
/// a system that takes it must not also take `MessageWriter<RenderAssetCmd>`,
/// `MessageWriter<AudioCmd>` or `GameCtx` (which holds an audio writer). Bevy
/// rejects such a system at startup with a conflicting-access panic. The
/// same goes for `ResMut<GameState>` and `ResMut<LoadedAssets>`, which it
/// reads. Use
/// [`audio`](Self::audio) and [`render`](Self::render) to write any other
/// command, e.g. `assets.audio().write(AudioCmd::PlayFx { id: "jump".into() })`.
#[derive(SystemParam)]
pub struct AssetLoader<'w> {
    render: MessageWriter<'w, RenderAssetCmd>,
    audio: MessageWriter<'w, AudioCmd>,
    texture_dims: Res<'w, TextureDimsStore>,
    font_metrics: Res<'w, FontMetricsStore>,
    gate: AssetGate<'w>,
}

impl<'w> AssetLoader<'w> {
    /// Queues a texture load from `path` under `key`, with
    /// [`TextureFilter::Nearest`] sampling.
    pub fn load_texture(
        &mut self,
        key: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<(), AssetError> {
        self.load_texture_with(key, path, TextureFilter::Nearest)
    }

    /// Queues a texture load from `path` under `key`, sampled with `filter`.
    pub fn load_texture_with(
        &mut self,
        key: impl Into<String>,
        path: impl Into<String>,
        filter: TextureFilter,
    ) -> Result<(), AssetError> {
        self.queue_render(RenderAssetCmd::Texture {
            key: key.into(),
            path: path.into(),
            filter,
        })
    }

    /// Queues a font load from `path` at `size` pixels under `key`. An already
    /// loaded `key` is reloaded.
    pub fn load_font(
        &mut self,
        key: impl Into<String>,
        path: impl Into<String>,
        size: i32,
    ) -> Result<(), AssetError> {
        self.queue_render(RenderAssetCmd::Font {
            key: key.into(),
            path: path.into(),
            size,
            skip_if_loaded: false,
        })
    }

    /// Queues a shader load under `key`. A `None` path uses raylib's default
    /// shader for that stage.
    pub fn load_shader(
        &mut self,
        key: impl Into<String>,
        vs_path: Option<&str>,
        fs_path: Option<&str>,
    ) -> Result<(), AssetError> {
        self.queue_render(RenderAssetCmd::Shader {
            key: key.into(),
            vs_path: vs_path.map(str::to_owned),
            fs_path: fs_path.map(str::to_owned),
        })
    }

    /// Queues a sound effect load from `path` under `key`.
    pub fn load_sound(
        &mut self,
        key: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<(), AssetError> {
        self.queue_audio(AudioCmd::LoadFx {
            id: key.into(),
            path: path.into(),
        })
    }

    /// Queues a music stream load from `path` under `key`.
    pub fn load_music(
        &mut self,
        key: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<(), AssetError> {
        self.queue_audio(AudioCmd::LoadMusic {
            id: key.into(),
            path: path.into(),
        })
    }

    fn queue_render(&mut self, cmd: RenderAssetCmd) -> Result<(), AssetError> {
        if queues(self.gate.render(&cmd), cmd.load_target())? {
            self.render.write(cmd);
        }
        Ok(())
    }

    fn queue_audio(&mut self, cmd: AudioCmd) -> Result<(), AssetError> {
        if queues(self.gate.audio(&cmd), cmd.load_target())? {
            self.audio.write(cmd);
        }
        Ok(())
    }

    /// Pixel size `(width, height)` of the loaded texture `key`, or `None`
    /// until the render thread has loaded it.
    pub fn texture_size(&self, key: &str) -> Option<(i32, i32)> {
        self.texture_dims.get(key)
    }

    /// Whether the render thread has loaded the texture `key`.
    pub fn is_texture_loaded(&self, key: &str) -> bool {
        self.texture_size(key).is_some()
    }

    /// Whether the render thread has loaded the font `key`.
    pub fn is_font_loaded(&self, key: &str) -> bool {
        self.font_metrics.0.contains_key(key)
    }

    /// The audio command writer, for playback and unload commands.
    pub fn audio(&mut self) -> &mut MessageWriter<'w, AudioCmd> {
        &mut self.audio
    }

    /// The render asset command writer, for commands without a helper here
    /// (in-memory loads, removal, renames, texture filters).
    pub fn render(&mut self) -> &mut MessageWriter<'w, RenderAssetCmd> {
        &mut self.render
    }
}

/// Whether a load with this verdict is queued, or the error for a rejected
/// one. `target` is the load's own `load_target()`, always `Some`.
fn queues(verdict: Verdict, target: Option<(AssetKind, &str)>) -> Result<bool, AssetError> {
    match verdict {
        Verdict::Forward => Ok(true),
        Verdict::NoOp => Ok(false),
        Verdict::Rejected => {
            let (kind, key) = target.expect("AssetLoader only queues load commands");
            Err(AssetError::AssetChangeDuringDeterministicPlay {
                kind,
                key: key.to_owned(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::fontmetrics::test_support::lowercase_alphabet_metrics;
    use crate::resources::gamestate::GameState;
    use crate::resources::loaded_assets::LoadedAssets;
    use bevy_ecs::system::RunSystemOnce;

    fn loader_world() -> World {
        let mut world = World::new();
        world.insert_resource(Messages::<RenderAssetCmd>::default());
        world.insert_resource(Messages::<AudioCmd>::default());
        world.insert_resource(TextureDimsStore::default());
        world.insert_resource(FontMetricsStore::default());
        world.init_resource::<GameState>();
        world.init_resource::<LoadedAssets>();
        world
    }

    fn render_cmds(world: &mut World) -> Vec<RenderAssetCmd> {
        world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .drain()
            .collect()
    }

    fn audio_cmds(world: &mut World) -> Vec<AudioCmd> {
        world.resource_mut::<Messages<AudioCmd>>().drain().collect()
    }

    #[test]
    fn render_loads_queue_render_asset_cmds() {
        let mut world = loader_world();
        world
            .run_system_once(|mut assets: AssetLoader| {
                assets.load_texture("player", "player.png").unwrap();
                assets
                    .load_texture_with("bg", "bg.png", TextureFilter::Bilinear)
                    .unwrap();
                assets.load_font("arcade", "arcade.ttf", 32).unwrap();
                assets.load_shader("glow", None, Some("glow.fs")).unwrap();
            })
            .unwrap();

        let cmds = render_cmds(&mut world);
        assert_eq!(cmds.len(), 4);
        assert!(matches!(
            &cmds[0],
            RenderAssetCmd::Texture { key, path, filter: TextureFilter::Nearest }
                if key == "player" && path == "player.png"
        ));
        assert!(matches!(
            &cmds[1],
            RenderAssetCmd::Texture { key, path, filter: TextureFilter::Bilinear }
                if key == "bg" && path == "bg.png"
        ));
        assert!(matches!(
            &cmds[2],
            RenderAssetCmd::Font { key, path, size: 32, skip_if_loaded: false }
                if key == "arcade" && path == "arcade.ttf"
        ));
        assert!(matches!(
            &cmds[3],
            RenderAssetCmd::Shader { key, vs_path: None, fs_path: Some(fs) }
                if key == "glow" && fs == "glow.fs"
        ));
        assert!(audio_cmds(&mut world).is_empty());
    }

    #[test]
    fn audio_loads_queue_audio_cmds() {
        let mut world = loader_world();
        world
            .run_system_once(|mut assets: AssetLoader| {
                assets.load_sound("jump", "jump.wav").unwrap();
                assets.load_music("bgm", "music.ogg").unwrap();
            })
            .unwrap();

        let cmds = audio_cmds(&mut world);
        assert_eq!(cmds.len(), 2);
        assert!(matches!(
            &cmds[0],
            AudioCmd::LoadFx { id, path } if id == "jump" && path == "jump.wav"
        ));
        assert!(matches!(
            &cmds[1],
            AudioCmd::LoadMusic { id, path } if id == "bgm" && path == "music.ogg"
        ));
        assert!(render_cmds(&mut world).is_empty());
    }

    #[test]
    fn passthroughs_write_raw_commands() {
        let mut world = loader_world();
        world
            .run_system_once(|mut assets: AssetLoader| {
                assets.audio().write(AudioCmd::PlayFx { id: "jump".into() });
                assets.render().write(RenderAssetCmd::RemoveTexture {
                    key: "player".into(),
                });
            })
            .unwrap();

        assert!(matches!(
            audio_cmds(&mut world).as_slice(),
            [AudioCmd::PlayFx { id }] if id == "jump"
        ));
        assert!(matches!(
            render_cmds(&mut world).as_slice(),
            [RenderAssetCmd::RemoveTexture { key }] if key == "player"
        ));
    }

    fn probe(assets: AssetLoader) -> (bool, Option<(i32, i32)>, bool) {
        (
            assets.is_texture_loaded("player"),
            assets.texture_size("player"),
            assets.is_font_loaded("arcade"),
        )
    }

    #[test]
    fn predicates_follow_the_dims_and_metrics_stores() {
        let mut world = loader_world();
        assert_eq!(world.run_system_once(probe).unwrap(), (false, None, false));

        world
            .resource_mut::<TextureDimsStore>()
            .insert("player", 16, 24);
        world
            .resource_mut::<FontMetricsStore>()
            .0
            .insert("arcade".into(), lowercase_alphabet_metrics());

        assert_eq!(
            world.run_system_once(probe).unwrap(),
            (true, Some((16, 24)), true)
        );
    }
}
