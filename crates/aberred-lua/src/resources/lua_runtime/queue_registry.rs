/// Central registry of all Lua command queues.
///
/// To add a new queue, add one row `(field_name, CmdType, clear_policy)` to
/// the `@master` arm below. `clear_policy` is `clear` (wiped by
/// `clear_all_commands` on scene switch — the default for queues whose
/// commands may reference about-to-be-despawned entities) or `preserve`
/// (left untouched — for scene-agnostic queues whose only drain site runs
/// after `switch_scene`, e.g. `map_commands`/`asset_commands`).
///
/// Drain methods (`drain_<field>_into`), `clear_all_commands`'s body, and
/// `LuaAppData`'s queue fields (runtime.rs) are all generated automatically
/// from this single list.
#[macro_export]
macro_rules! lua_queues {
    // ------------------------------------------------------------------
    // Single authoritative list of (queue_field, CmdType, clear_policy) rows.
    // Callers prepend dispatch tokens; @master appends the 23 rows and
    // re-invokes lua_queues! so the chosen @dispatch_* arm matches.
    // ------------------------------------------------------------------
    (@master $($rest:tt)*) => {
        $crate::lua_queues!{ $($rest)*
            (asset_commands,            AssetCmd,         preserve),
            (spawn_commands,            Box<SpawnCmd>,    clear),
            (audio_commands,            AudioLuaCmd,      clear),
            (signal_commands,           SignalCmd,        clear),
            (phase_commands,            PhaseCmd,         clear),
            (entity_commands,           EntityCmd,        clear),
            (group_commands,            GroupCmd,         clear),
            (camera_commands,           CameraCmd,        clear),
            (animation_commands,        AnimationCmd,     clear),
            (render_commands,           RenderCmd,        clear),
            (gui_theme_commands,        RenderCmd,        preserve),
            (clone_commands,            CloneCmd,         clear),
            (gameconfig_commands,       GameConfigCmd,    clear),
            (camera_follow_commands,    CameraFollowCmd,  clear),
            (input_commands,            InputCmd,         clear),
            (map_commands,              MapLuaCmd,        preserve),
            (collision_entity_commands, EntityCmd,        clear),
            (collision_signal_commands, SignalCmd,        clear),
            (collision_audio_commands,  AudioLuaCmd,      clear),
            (collision_spawn_commands,  Box<SpawnCmd>,    clear),
            (collision_clone_commands,  CloneCmd,         clear),
            (collision_phase_commands,  PhaseCmd,         clear),
            (collision_camera_commands, CameraCmd,        clear),
        }
    };

    (drain_methods) => {
        $crate::lua_queues!{@master @dispatch_drain}
    };

    // Pass the `LuaAppData` binding as `$d` because macro hygiene prevents
    // the expansion from seeing a caller-defined local named `data` directly.
    // `$d` must be `tt` (not `expr`) so it survives the @master round-trip and
    // composes with `.field` below; the trailing `,` separates it from the
    // appended (field, Type, policy) rows for @dispatch_clear's pattern.
    (clear_body $d:tt) => {
        $crate::lua_queues!{@master @dispatch_clear $d ,}
    };

    (@dispatch_drain $(($field:ident, $ty:ty, $policy:ident)),* $(,)?) => {
        ::paste::paste! {
            $(
                pub fn [<drain_ $field _into>](&self, out: &mut ::std::vec::Vec<$ty>) {
                    self.drain_queue_into(|d| &d.$field, out);
                }
            )*
        }
    };

    (@dispatch_clear $d:tt, $(($field:ident, $ty:ty, $policy:ident)),* $(,)?) => {
        $( $crate::lua_queues!{@clear_one $d, $field, $policy} )*
    };

    (@clear_one $d:tt, $field:ident, clear) => {
        $d.$field.borrow_mut().clear();
    };
    (@clear_one $d:tt, $field:ident, preserve) => {};

    // ------------------------------------------------------------------
    // Whole-item generation of LuaAppData: `$extra` carries the non-queue
    // fields verbatim (they don't come from the row list), spliced in
    // alongside the generated queue fields within the SAME struct item.
    // ------------------------------------------------------------------
    (app_data_struct { $($extra:tt)* }) => {
        $crate::lua_queues!{@master @dispatch_struct { $($extra)* }}
    };

    (@dispatch_struct { $($extra:tt)* } $(($field:ident, $ty:ty, $policy:ident)),* $(,)?) => {
        /// Shared state accessible from Lua function closures.
        /// This is stored in Lua's app_data and allows Lua functions to queue commands.
        ///
        /// Queue fields are generated from the single list in queue_registry.rs's
        /// `@master` arm — add a row there, not here, to add a new queue.
        #[derive(Default)]
        pub(super) struct LuaAppData {
            $( pub(super) $field: ::std::cell::RefCell<::std::vec::Vec<$ty>>, )*
            $($extra)*
        }
    };
}
