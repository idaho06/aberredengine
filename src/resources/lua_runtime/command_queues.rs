//! Command queue drain and cache update methods for the Lua runtime.
//!
//! These methods are called by Rust systems after Lua callbacks complete,
//! to process queued commands and update read-only caches.

use super::commands::*;
use super::runtime::{LuaAppData, LuaRuntime, action_to_str};
use super::spawn_data::*;
use crate::resources::worldsignals::SignalSnapshot;
use rustc_hash::FxHashSet;
use std::sync::Arc;

impl LuaRuntime {
    /// Drains all queued asset commands.
    pub fn drain_asset_commands(&self) -> Vec<AssetCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.asset_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued spawn commands.
    pub fn drain_spawn_commands(&self) -> Vec<SpawnCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.spawn_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued audio commands.
    pub fn drain_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.audio_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued signal commands.
    pub fn drain_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.signal_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued phase commands.
    pub fn drain_phase_commands(&self) -> Vec<PhaseCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.phase_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued entity commands.
    pub fn drain_entity_commands(&self) -> Vec<EntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.entity_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued group commands.
    pub fn drain_group_commands(&self) -> Vec<GroupCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.group_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued tilemap commands.
    pub fn drain_tilemap_commands(&self) -> Vec<TilemapCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.tilemap_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued camera commands.
    pub fn drain_camera_commands(&self) -> Vec<CameraCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.camera_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued animation commands.
    pub fn drain_animation_commands(&self) -> Vec<AnimationCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.animation_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued render commands.
    pub fn drain_render_commands(&self) -> Vec<RenderCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.render_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued game config commands.
    pub fn drain_gameconfig_commands(&self) -> Vec<GameConfigCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.gameconfig_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued camera follow commands.
    pub fn drain_camera_follow_commands(&self) -> Vec<CameraFollowCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.camera_follow_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued input rebinding commands.
    pub fn drain_input_commands(&self) -> Vec<InputCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.input_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Updates the cached input bindings snapshot that Lua can read via `engine.get_binding()`.
    pub fn update_bindings_cache(
        &self,
        bindings: &crate::resources::input_bindings::InputBindings,
    ) {
        use crate::resources::input_bindings::binding_to_str;
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            let mut snap = data.bindings_snapshot.borrow_mut();
            snap.clear();
            for (action, bl) in &bindings.map {
                if let Some(first) = bl.first() {
                    let key_str = binding_to_str(*first);
                    let action_str = action_to_str(*action).to_string();
                    snap.insert(action_str, key_str.to_string());
                }
            }
        }
    }

    /// Drains all queued collision entity commands.
    pub fn drain_collision_entity_commands(&self) -> Vec<EntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_entity_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision signal commands.
    pub fn drain_collision_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_signal_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision audio commands.
    pub fn drain_collision_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_audio_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision spawn commands.
    pub fn drain_collision_spawn_commands(&self) -> Vec<SpawnCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_spawn_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued clone commands.
    pub fn drain_clone_commands(&self) -> Vec<CloneCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.clone_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued collision clone commands.
    pub fn drain_collision_clone_commands(&self) -> Vec<CloneCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_clone_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clears all command queues without processing them.
    ///
    /// Call at the start of scene switches to discard stale commands from the
    /// previous scene that might reference despawned entities.
    pub fn clear_all_commands(&self) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            data.entity_commands.borrow_mut().clear();
            data.spawn_commands.borrow_mut().clear();
            data.clone_commands.borrow_mut().clear();
            data.signal_commands.borrow_mut().clear();
            data.phase_commands.borrow_mut().clear();
            data.audio_commands.borrow_mut().clear();
            data.group_commands.borrow_mut().clear();
            data.camera_commands.borrow_mut().clear();
            data.tilemap_commands.borrow_mut().clear();
            data.render_commands.borrow_mut().clear();
            data.gameconfig_commands.borrow_mut().clear();
            data.camera_follow_commands.borrow_mut().clear();
            data.input_commands.borrow_mut().clear();
        }
    }

    /// Drains all queued collision phase commands.
    pub fn drain_collision_phase_commands(&self) -> Vec<PhaseCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_phase_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision camera commands.
    pub fn drain_collision_camera_commands(&self) -> Vec<CameraCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_camera_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Updates the cached world signal snapshot that Lua can read.
    pub fn update_signal_cache(&self, snapshot: Arc<SignalSnapshot>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.signal_snapshot.borrow_mut() = snapshot;
        }
    }

    /// Updates the cached tracked groups that Lua can read.
    pub fn update_tracked_groups_cache(&self, groups: &FxHashSet<String>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.tracked_groups.borrow_mut() = groups.clone();
        }
    }

    /// Updates the cached game configuration snapshot that Lua can read.
    pub fn update_gameconfig_cache(&self, config: &crate::resources::gameconfig::GameConfig) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            let mut snapshot = data.gameconfig_snapshot.borrow_mut();
            snapshot.fullscreen = config.fullscreen;
            snapshot.vsync = config.vsync;
            snapshot.target_fps = config.target_fps;
            snapshot.render_width = config.render_width;
            snapshot.render_height = config.render_height;
            snapshot.background_r = config.background_color.r;
            snapshot.background_g = config.background_color.g;
            snapshot.background_b = config.background_color.b;
        }
    }
}
