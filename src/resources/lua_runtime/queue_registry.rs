/// Central registry of all Lua command queues.
///
/// To add a new queue:
///   1. Add one row `(field_name, CmdType)` to the `@master` arm below.
///   2. Add the corresponding `RefCell<Vec<CmdType>>` field to `LuaAppData` in
///      runtime.rs (struct + Default), with the same field name.
///
/// Drain methods (`drain_<field>_into`) and `clear_all_commands`'s body are
/// both generated automatically from the single list in the `@master` arm.
#[macro_export]
macro_rules! lua_queues {
    // ------------------------------------------------------------------
    // Single authoritative list of (queue_field, CmdType) pairs.
    // Callers prepend dispatch tokens; @master appends the 22 rows and
    // re-invokes lua_queues! so the chosen @dispatch_* arm matches.
    // ------------------------------------------------------------------
    (@master $($rest:tt)*) => {
        $crate::lua_queues!{ $($rest)*
            (asset_commands,            AssetCmd),
            (spawn_commands,            SpawnCmd),
            (audio_commands,            AudioLuaCmd),
            (signal_commands,           SignalCmd),
            (phase_commands,            PhaseCmd),
            (entity_commands,           EntityCmd),
            (group_commands,            GroupCmd),
            (camera_commands,           CameraCmd),
            (animation_commands,        AnimationCmd),
            (render_commands,           RenderCmd),
            (clone_commands,            CloneCmd),
            (gameconfig_commands,       GameConfigCmd),
            (camera_follow_commands,    CameraFollowCmd),
            (input_commands,            InputCmd),
            (map_commands,              MapLuaCmd),
            (collision_entity_commands, EntityCmd),
            (collision_signal_commands, SignalCmd),
            (collision_audio_commands,  AudioLuaCmd),
            (collision_spawn_commands,  SpawnCmd),
            (collision_clone_commands,  CloneCmd),
            (collision_phase_commands,  PhaseCmd),
            (collision_camera_commands, CameraCmd),
        }
    };

    (drain_methods) => {
        $crate::lua_queues!{@master @dispatch_drain}
    };

    // Pass the `LuaAppData` binding as `$d` because macro hygiene prevents
    // the expansion from seeing a caller-defined local named `data` directly.
    // `$d` must be `tt` (not `expr`) so it survives the @master round-trip and
    // composes with `.field` below; the trailing `,` separates it from the
    // appended (field, Type) rows for @dispatch_clear's pattern.
    (clear_body $d:tt) => {
        $crate::lua_queues!{@master @dispatch_clear $d ,}
    };

    (@dispatch_drain $(($field:ident, $ty:ty)),* $(,)?) => {
        ::paste::paste! {
            $(
                pub fn [<drain_ $field _into>](&self, out: &mut ::std::vec::Vec<$ty>) {
                    self.drain_queue_into(|d| &d.$field, out);
                }
            )*
        }
    };

    (@dispatch_clear $d:tt, $(($field:ident, $ty:ty)),* $(,)?) => {
        $( $d.$field.borrow_mut().clear(); )*
    };
}
