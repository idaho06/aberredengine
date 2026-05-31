//! Command queue drain and cache update methods for the Lua runtime.
//!
//! These methods are called by Rust systems after Lua callbacks complete,
//! to process queued commands and update read-only caches.

use super::commands::*;
use super::runtime::{LuaAppData, LuaRuntime, action_to_str};
use super::spawn_data::*;
use crate::resources::worldsignals::SignalSnapshot;
use rustc_hash::FxHashSet;
use std::cell::RefCell;
use std::sync::Arc;

impl LuaRuntime {
    // -------------------------------------------------------------------------
    // Private helper
    // -------------------------------------------------------------------------

    /// Drains a single command queue into the caller-owned `out` buffer.
    ///
    /// Uses `std::mem::swap` to exchange the internal pointer/length/capacity
    /// triples at word level — zero element copies. After the call, `out` holds
    /// the queue's previous content and the queue holds `out`'s previous (empty)
    /// buffer, retaining capacity for next frame's pushes.
    fn drain_queue_into<T>(
        &self,
        get: impl Fn(&LuaAppData) -> &RefCell<Vec<T>>,
        out: &mut Vec<T>,
    ) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            std::mem::swap(out, &mut *get(&data).borrow_mut());
        }
    }

    // -------------------------------------------------------------------------
    // Regular drain methods
    // -------------------------------------------------------------------------

    pub fn drain_asset_commands_into(&self, out: &mut Vec<AssetCmd>) {
        self.drain_queue_into(|d| &d.asset_commands, out);
    }

    pub fn drain_spawn_commands_into(&self, out: &mut Vec<SpawnCmd>) {
        self.drain_queue_into(|d| &d.spawn_commands, out);
    }

    pub fn drain_audio_commands_into(&self, out: &mut Vec<AudioLuaCmd>) {
        self.drain_queue_into(|d| &d.audio_commands, out);
    }

    pub fn drain_signal_commands_into(&self, out: &mut Vec<SignalCmd>) {
        self.drain_queue_into(|d| &d.signal_commands, out);
    }

    pub fn drain_phase_commands_into(&self, out: &mut Vec<PhaseCmd>) {
        self.drain_queue_into(|d| &d.phase_commands, out);
    }

    pub fn drain_entity_commands_into(&self, out: &mut Vec<EntityCmd>) {
        self.drain_queue_into(|d| &d.entity_commands, out);
    }

    pub fn drain_group_commands_into(&self, out: &mut Vec<GroupCmd>) {
        self.drain_queue_into(|d| &d.group_commands, out);
    }

    pub fn drain_camera_commands_into(&self, out: &mut Vec<CameraCmd>) {
        self.drain_queue_into(|d| &d.camera_commands, out);
    }

    pub fn drain_animation_commands_into(&self, out: &mut Vec<AnimationCmd>) {
        self.drain_queue_into(|d| &d.animation_commands, out);
    }

    pub fn drain_render_commands_into(&self, out: &mut Vec<RenderCmd>) {
        self.drain_queue_into(|d| &d.render_commands, out);
    }

    pub fn drain_gameconfig_commands_into(&self, out: &mut Vec<GameConfigCmd>) {
        self.drain_queue_into(|d| &d.gameconfig_commands, out);
    }

    pub fn drain_camera_follow_commands_into(&self, out: &mut Vec<CameraFollowCmd>) {
        self.drain_queue_into(|d| &d.camera_follow_commands, out);
    }

    pub fn drain_map_commands_into(&self, out: &mut Vec<MapLuaCmd>) {
        self.drain_queue_into(|d| &d.map_commands, out);
    }

    pub fn drain_input_commands_into(&self, out: &mut Vec<InputCmd>) {
        self.drain_queue_into(|d| &d.input_commands, out);
    }

    pub fn drain_clone_commands_into(&self, out: &mut Vec<CloneCmd>) {
        self.drain_queue_into(|d| &d.clone_commands, out);
    }

    // -------------------------------------------------------------------------
    // Collision drain methods
    // -------------------------------------------------------------------------

    pub fn drain_collision_entity_commands_into(&self, out: &mut Vec<EntityCmd>) {
        self.drain_queue_into(|d| &d.collision_entity_commands, out);
    }

    pub fn drain_collision_signal_commands_into(&self, out: &mut Vec<SignalCmd>) {
        self.drain_queue_into(|d| &d.collision_signal_commands, out);
    }

    pub fn drain_collision_audio_commands_into(&self, out: &mut Vec<AudioLuaCmd>) {
        self.drain_queue_into(|d| &d.collision_audio_commands, out);
    }

    pub fn drain_collision_spawn_commands_into(&self, out: &mut Vec<SpawnCmd>) {
        self.drain_queue_into(|d| &d.collision_spawn_commands, out);
    }

    pub fn drain_collision_clone_commands_into(&self, out: &mut Vec<CloneCmd>) {
        self.drain_queue_into(|d| &d.collision_clone_commands, out);
    }

    pub fn drain_collision_phase_commands_into(&self, out: &mut Vec<PhaseCmd>) {
        self.drain_queue_into(|d| &d.collision_phase_commands, out);
    }

    pub fn drain_collision_camera_commands_into(&self, out: &mut Vec<CameraCmd>) {
        self.drain_queue_into(|d| &d.collision_camera_commands, out);
    }

    // -------------------------------------------------------------------------
    // Queue management
    // -------------------------------------------------------------------------

    /// Clears all command queues without processing them.
    ///
    /// Call at the start of scene switches to discard stale commands from the
    /// previous scene that might reference despawned entities.
    pub fn clear_all_commands(&self) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            // Regular queues
            data.asset_commands.borrow_mut().clear();
            data.spawn_commands.borrow_mut().clear();
            data.clone_commands.borrow_mut().clear();
            data.signal_commands.borrow_mut().clear();
            data.phase_commands.borrow_mut().clear();
            data.entity_commands.borrow_mut().clear();
            data.audio_commands.borrow_mut().clear();
            data.group_commands.borrow_mut().clear();
            data.camera_commands.borrow_mut().clear();
            data.render_commands.borrow_mut().clear();
            data.animation_commands.borrow_mut().clear();
            data.gameconfig_commands.borrow_mut().clear();
            data.camera_follow_commands.borrow_mut().clear();
            data.input_commands.borrow_mut().clear();
            data.map_commands.borrow_mut().clear();
            // Collision-scoped queues
            data.collision_entity_commands.borrow_mut().clear();
            data.collision_signal_commands.borrow_mut().clear();
            data.collision_audio_commands.borrow_mut().clear();
            data.collision_spawn_commands.borrow_mut().clear();
            data.collision_clone_commands.borrow_mut().clear();
            data.collision_phase_commands.borrow_mut().clear();
            data.collision_camera_commands.borrow_mut().clear();
        }
    }

    // -------------------------------------------------------------------------
    // Cache updates
    // -------------------------------------------------------------------------

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
