/// Central registry of all Lua command queues.
///
/// To add a new queue:
///   1. Add one entry to the `@master` arm below.
///   2. Add the corresponding field to `LuaAppData` in runtime.rs (struct + Default).
///
/// Drain methods and clear calls are generated automatically from the list here.
#[macro_export]
macro_rules! lua_queues {
    (drain_methods) => { $crate::lua_queues!{@dispatch_drain
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
    }};

    // Pass the `LuaAppData` binding as `$d` because macro hygiene prevents
    // the expansion from seeing a caller-defined local named `data` directly.
    (clear_body $d:expr) => { $crate::lua_queues!{@dispatch_clear $d,
        asset_commands, spawn_commands, audio_commands, signal_commands,
        phase_commands, entity_commands, group_commands, camera_commands,
        animation_commands, render_commands, clone_commands, gameconfig_commands,
        camera_follow_commands, input_commands, map_commands,
        collision_entity_commands, collision_signal_commands, collision_audio_commands,
        collision_spawn_commands, collision_clone_commands, collision_phase_commands,
        collision_camera_commands,
    }};

    (@dispatch_drain $(($field:ident, $ty:ty)),* $(,)?) => {
        ::paste::paste! {
            $(
                pub fn [<drain_ $field _into>](&self, out: &mut ::std::vec::Vec<$ty>) {
                    self.drain_queue_into(|d| &d.$field, out);
                }
            )*
        }
    };

    (@dispatch_clear $d:expr, $($field:ident),* $(,)?) => {
        $( $d.$field.borrow_mut().clear(); )*
    };
}
