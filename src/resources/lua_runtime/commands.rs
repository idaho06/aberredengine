//! Command enums for Lua-Rust communication.
//!
//! These enums represent commands that Lua scripts can queue for execution
//! by Rust systems. Commands are processed after Lua callbacks return.

/// Commands that Lua can queue for asset loading.
/// These are processed by Rust systems that have access to the necessary resources.
#[derive(Debug, Clone)]
pub enum AssetCmd {
    /// Load a texture from a file path
    LoadTexture { id: String, path: String },
    /// Load a font from a file path with a specific size
    LoadFont { id: String, path: String, size: i32 },
    /// Load a music track from a file path
    LoadMusic { id: String, path: String },
    /// Load a sound effect from a file path
    LoadSound { id: String, path: String },
    /// Load a tilemap from a directory path
    LoadTilemap { id: String, path: String },
}

/// Audio commands that Lua can queue.
#[derive(Debug, Clone)]
pub enum AudioLuaCmd {
    /// Play a music track
    PlayMusic { id: String, looped: bool },
    /// Play a sound effect
    PlaySound { id: String },
    /// Stop all music
    StopAllMusic,
    /// Stop all sounds
    StopAllSounds,
}

/// Commands to modify WorldSignals from Lua.
#[derive(Debug, Clone)]
pub enum SignalCmd {
    SetScalar { key: String, value: f32 },
    SetInteger { key: String, value: i32 },
    SetString { key: String, value: String },
    SetFlag { key: String },
    ClearFlag { key: String },
    ClearScalar { key: String },
    ClearInteger { key: String },
    ClearString { key: String },
    SetEntity { key: String, entity_id: u64 },
    RemoveEntity { key: String },
}

/// Commands for phase transitions from Lua.
#[derive(Debug, Clone)]
pub enum PhaseCmd {
    /// Request a phase transition for a specific entity
    TransitionTo { entity_id: u64, phase: String },
}

/// Commands for manipulating entity components from Lua.
#[derive(Debug, Clone)]
pub enum EntityCmd {
    /// Release an entity from StuckTo - removes StuckTo and adds RigidBody with stored velocity
    ReleaseStuckTo { entity_id: u64 },
    /// Set a flag on an entity's Signals component
    SignalSetFlag { entity_id: u64, flag: String },
    /// Clear a flag on an entity's Signals component
    SignalClearFlag { entity_id: u64, flag: String },
    /// Set entity velocity (RigidBody)
    SetVelocity { entity_id: u64, vx: f32, vy: f32 },
    /// Insert a StuckTo component
    InsertStuckTo {
        entity_id: u64,
        target_id: u64,
        follow_x: bool,
        follow_y: bool,
        offset_x: f32,
        offset_y: f32,
        stored_vx: f32,
        stored_vy: f32,
    },
    /// Restart the entity's current animation from frame 0
    RestartAnimation { entity_id: u64 },
    /// Set the entity's animation to a specific animation key (and restart from frame 0)
    SetAnimation {
        entity_id: u64,
        animation_key: String,
    },
    /// Insert a LuaTimer component
    InsertLuaTimer {
        entity_id: u64,
        duration: f32,
        callback: String,
    },
    /// Remove a LuaTimer component
    RemoveLuaTimer { entity_id: u64 },
    /// Insert TweenPosition component
    InsertTweenPosition {
        entity_id: u64,
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        duration: f32,
        easing: String,
        loop_mode: String,
    },
    /// Insert TweenRotation component
    InsertTweenRotation {
        entity_id: u64,
        from: f32,
        to: f32,
        duration: f32,
        easing: String,
        loop_mode: String,
    },
    /// Insert TweenScale component
    InsertTweenScale {
        entity_id: u64,
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
        duration: f32,
        easing: String,
        loop_mode: String,
    },
    /// Remove TweenPosition component
    RemoveTweenPosition { entity_id: u64 },
    /// Remove TweenRotation component
    RemoveTweenRotation { entity_id: u64 },
    /// Remove TweenScale component
    RemoveTweenScale { entity_id: u64 },
    /// Set entity rotation
    SetRotation { entity_id: u64, degrees: f32 },
    /// Set entity scale
    SetScale { entity_id: u64, sx: f32, sy: f32 },
    /// Set a scalar signal on an entity's Signals component
    SignalSetScalar {
        entity_id: u64,
        key: String,
        value: f32,
    },
    /// Set a string signal on an entity's Signals component
    SignalSetString {
        entity_id: u64,
        key: String,
        value: String,
    },
    /// Add or update a named force on the entity's RigidBody
    AddForce {
        entity_id: u64,
        name: String,
        x: f32,
        y: f32,
        enabled: bool,
    },
    /// Remove a named force from the entity's RigidBody
    RemoveForce { entity_id: u64, name: String },
    /// Enable or disable a specific force on the entity's RigidBody
    SetForceEnabled {
        entity_id: u64,
        name: String,
        enabled: bool,
    },
    /// Update the value of an existing force on the entity's RigidBody
    SetForceValue {
        entity_id: u64,
        name: String,
        x: f32,
        y: f32,
    },
    /// Set friction on entity's RigidBody
    SetFriction { entity_id: u64, friction: f32 },
    /// Set max_speed on entity's RigidBody (None to remove limit)
    SetMaxSpeed {
        entity_id: u64,
        max_speed: Option<f32>,
    },
    /// Freeze entity (skip physics calculations)
    FreezeEntity { entity_id: u64 },
    /// Unfreeze entity (resume physics calculations)
    UnfreezeEntity { entity_id: u64 },
    /// Set entity speed while maintaining velocity direction
    SetSpeed { entity_id: u64, speed: f32 },
    /// Set entity position (MapPosition)
    SetPosition { entity_id: u64, x: f32, y: f32 },
    /// Despawn an entity
    Despawn { entity_id: u64 },
    /// Set an integer signal on an entity's Signals component
    SignalSetInteger {
        entity_id: u64,
        key: String,
        value: i32,
    },
}

/// Commands for tracked groups from Lua.
#[derive(Debug, Clone)]
pub enum GroupCmd {
    //TODO: Rename to TrackedGroupCmd
    /// Track a group for entity counting
    TrackGroup { name: String },
    /// Stop tracking a group
    UntrackGroup { name: String },
    /// Clear all tracked groups
    ClearTrackedGroups,
}

/// Commands for tilemap operations from Lua.
#[derive(Debug, Clone)]
pub enum TilemapCmd {
    /// Spawn tiles from a loaded tilemap
    SpawnTiles { id: String },
}

/// Commands for camera operations from Lua.
#[derive(Debug, Clone)]
pub enum CameraCmd {
    /// Set the 2D camera with target, offset, rotation and zoom
    SetCamera2D {
        target_x: f32,
        target_y: f32,
        offset_x: f32,
        offset_y: f32,
        rotation: f32,
        zoom: f32,
    },
}

/// Commands for registering animations from Lua.
#[derive(Debug, Clone)]
pub enum AnimationCmd {
    /// Register an animation resource in the AnimationStore
    RegisterAnimation {
        id: String,
        tex_key: String,
        pos_x: f32,
        pos_y: f32,
        displacement: f32,
        frame_count: usize,
        fps: f32,
        looped: bool,
    },
}
