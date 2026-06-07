/// Central registry of all Lua command queues.
///
/// To add a new queue:
///   1. Add one entry to the `@master` arm below.
///   2. Add the corresponding field to `LuaAppData` in runtime.rs (struct + Default).
///
/// Drain methods and clear calls are generated automatically from the list here.
#[macro_export]
macro_rules! lua_queues {
    (drain_methods) => { $crate::lua_queues!{@master drain_methods} };
    (clear_body)    => { $crate::lua_queues!{@master clear_body}    };

    // Authoritative queue list — add/remove entries here only.
    (@master $mode:tt) => {
        $crate::lua_queues!{@dispatch $mode,
            (asset_commands,            AssetCmd,        Regular),
            (spawn_commands,            SpawnCmd,        Regular),
            (audio_commands,            AudioLuaCmd,     Regular),
            (signal_commands,           SignalCmd,       Regular),
            (phase_commands,            PhaseCmd,        Regular),
            (entity_commands,           EntityCmd,       Regular),
            (group_commands,            GroupCmd,        Regular),
            (camera_commands,           CameraCmd,       Regular),
            (animation_commands,        AnimationCmd,    Regular),
            (render_commands,           RenderCmd,       Regular),
            (clone_commands,            CloneCmd,        Regular),
            (gameconfig_commands,       GameConfigCmd,   Regular),
            (camera_follow_commands,    CameraFollowCmd, Regular),
            (input_commands,            InputCmd,        Regular),
            (map_commands,              MapLuaCmd,       Regular),
            (collision_entity_commands, EntityCmd,       Collision),
            (collision_signal_commands, SignalCmd,       Collision),
            (collision_audio_commands,  AudioLuaCmd,     Collision),
            (collision_spawn_commands,  SpawnCmd,        Collision),
            (collision_clone_commands,  CloneCmd,        Collision),
            (collision_phase_commands,  PhaseCmd,        Collision),
            (collision_camera_commands, CameraCmd,       Collision),
        }
    };

    (@dispatch drain_methods, $(($field:ident, $ty:ty, $_s:ident)),* $(,)?) => {
        ::paste::paste! {
            $(
                pub fn [<drain_ $field _into>](&self, out: &mut ::std::vec::Vec<$ty>) {
                    self.drain_queue_into(|d| &d.$field, out);
                }
            )*
        }
    };

    (@dispatch clear_body, $(($field:ident, $_ty:ty, $_s:ident)),* $(,)?) => {
        $( data.$field.borrow_mut().clear(); )*
    };
}
