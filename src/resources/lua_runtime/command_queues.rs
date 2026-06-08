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
    fn drain_queue_into<T>(&self, get: impl Fn(&LuaAppData) -> &RefCell<Vec<T>>, out: &mut Vec<T>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            std::mem::swap(out, &mut *get(&data).borrow_mut());
        }
    }

    // -------------------------------------------------------------------------
    // Drain methods — all 22 generated from queue_registry.rs via lua_queues!
    // -------------------------------------------------------------------------

    crate::lua_queues!{drain_methods}

    // -------------------------------------------------------------------------
    // Queue management
    // -------------------------------------------------------------------------

    /// Clears all command queues without processing them.
    ///
    /// Call at the start of scene switches to discard stale commands from the
    /// previous scene that might reference despawned entities.
    pub fn clear_all_commands(&self) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            crate::lua_queues!{clear_body data}
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

    /// Updates the cached camera state snapshot that Lua reads via `engine.get_camera()` and
    /// `engine.get_camera_view_rect()`.
    ///
    /// Call this before invoking any Lua callback that may read camera state.
    ///
    /// Note: if a script calls both `get_camera()` and `engine.set_camera()` in the same
    /// callback, `get_camera()` returns the pre-override values because camera write commands
    /// are queued and applied in `process_lua_map_commands`, which runs after `lua_plugin::update`.
    pub fn update_camera_cache(
        &self,
        camera: &crate::resources::camera2d::Camera2DRes,
        screen: &crate::resources::screensize::ScreenSize,
    ) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            let rect = camera.world_visible_rect_snapped(screen);
            let mut snap = data.camera_snapshot.borrow_mut();
            snap.target_x = camera.0.target.x.round();
            snap.target_y = camera.0.target.y.round();
            snap.offset_x = camera.0.offset.x;
            snap.offset_y = camera.0.offset.y;
            snap.rotation = camera.0.rotation;
            snap.zoom = camera.0.zoom;
            snap.view_x = rect.x;
            snap.view_y = rect.y;
            snap.view_w = rect.width;
            snap.view_h = rect.height;
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
