use super::*;
use aberred_core::components::timer::TimerMode;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_signals",
        "Add empty Signals component",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            this.cmd.has_signals = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_scalar",
        "Add a scalar signal",
        [("key", "string"), ("value", "number")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, f32)| {
            this.cmd.signal_scalars.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_integer",
        "Add an integer signal",
        [("key", "string"), ("value", "integer")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, i32)| {
            this.cmd.signal_integers.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_flag",
        "Add a flag signal",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_string",
        "Add a string signal",
        [("key", "string"), ("value", "string")],
        |_, this: &mut LuaEntityBuilder, (key, value): (String, String)| {
            this.cmd.signal_strings.push((key, value));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_binding",
        "Bind text to a WorldSignal value",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            this.cmd.signal_binding = Some((key, None));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_signal_binding_format",
        "Set format string for signal binding (use {} as placeholder)",
        [("format", "string")],
        |_, this: &mut LuaEntityBuilder, format: String| {
            let Some((_, ref mut fmt)) = this.cmd.signal_binding else {
                return Err(LuaError::runtime(
                    "with_signal_binding_format() requires with_signal_binding() first",
                ));
            };
            *fmt = Some(format);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_phase",
        "Add phase state machine\n\nExample:\n```lua\nengine.spawn()\n    :with_phase({\n        initial = \"idle\",\n        phases = {\n            idle = {\n                on_enter = \"on_idle_enter\",\n                on_update = \"on_idle_update\",\n                on_exit = \"on_idle_exit\"\n            },\n            moving = { on_enter = \"on_moving_enter\" }\n        }\n    })\n    :build()\n```",
        [("table", "table")],
        |_, this: &mut LuaEntityBuilder, table: LuaTable| {
            let initial: String = table.get("initial")?;
            let mut phases = rustc_hash::FxHashMap::default();
            if let Ok(phases_table) = table.get::<LuaTable>("phases") {
                for pair in phases_table.pairs::<String, LuaTable>() {
                    let (phase_name, callbacks_table) = pair?;
                    let callbacks = PhaseCallbackData {
                        on_enter: callbacks_table.get("on_enter").ok(),
                        on_update: callbacks_table.get("on_update").ok(),
                        on_exit: callbacks_table.get("on_exit").ok(),
                    };
                    phases.insert(phase_name, callbacks);
                }
            }
            this.cmd.phase_data = Some(PhaseData { initial, phases });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_lua_timer",
        "Add a repeating Lua timer callback",
        [("duration", "number"), ("callback", "string")],
        |_, this: &mut LuaEntityBuilder, (duration, callback): (f32, String)| {
            set_lua_timer(this, duration, callback, TimerMode::Repeat);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_lua_timer_once",
        "Add a one-shot Lua timer callback; the timer removes itself after firing",
        [("duration", "number"), ("callback", "string")],
        |_, this: &mut LuaEntityBuilder, (duration, callback): (f32, String)| {
            set_lua_timer(this, duration, callback, TimerMode::Once);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_ttl",
        "Set time-to-live (auto-despawn)",
        [("seconds", "number")],
        |_, this: &mut LuaEntityBuilder, seconds: f32| {
            this.cmd.ttl = Some(seconds);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_grid_layout",
        "Spawn entities from a JSON grid layout",
        [
            ("path", "string"),
            ("group", "string"),
            ("zindex", "number")
        ],
        |_, this: &mut LuaEntityBuilder, (path, group, zindex): (String, String, f32)| {
            this.cmd.grid_layout = Some((path, group, zindex));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_lua_collision_rule",
        "Add collision callback between two groups",
        [
            ("group_a", "string"),
            ("group_b", "string"),
            ("callback", "string")
        ],
        |_, this: &mut LuaEntityBuilder, (group_a, group_b, callback): (String, String, String)| {
            this.cmd.lua_collision_rule = Some(LuaCollisionRuleData {
                group_a,
                group_b,
                callback,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_particle_emitter",
        "Add particle emitter",
        [("table", "table")],
        |_, this: &mut LuaEntityBuilder, table: LuaTable| {
            use super::super::spawn_data::{
                ParticleEmitterData, ParticleEmitterShapeData, ParticleTtlData,
            };

            let mut data = ParticleEmitterData::default();

            if let Ok(templates_table) = table.get::<LuaTable>("templates") {
                let mut keys = Vec::new();
                for key in templates_table.sequence_values::<String>().flatten() {
                    keys.push(key);
                }
                data.template_keys = keys;
            }

            match table.get::<LuaValue>("shape")? {
                LuaValue::Nil => {}
                LuaValue::String(s) if s.to_string_lossy() == "point" => {
                    data.shape = ParticleEmitterShapeData::Point;
                }
                LuaValue::Table(shape_table) => {
                    let kind: String = shape_table
                        .get("kind")
                        .or_else(|_| shape_table.get("type"))
                        .unwrap_or_default();
                    match kind.as_str() {
                        "rect" => {
                            let width: f32 = shape_table.get("width").unwrap_or(0.0);
                            let height: f32 = shape_table.get("height").unwrap_or(0.0);
                            data.shape = ParticleEmitterShapeData::Rect { width, height };
                        }
                        "point" => data.shape = ParticleEmitterShapeData::Point,
                        _ => {
                            return Err(LuaError::runtime(format!(
                                "with_particle_emitter: unknown shape kind '{kind}' \
                                 (expected 'point' or 'rect')"
                            )));
                        }
                    }
                }
                other => {
                    return Err(LuaError::runtime(format!(
                        "with_particle_emitter: unknown shape {other:?} \
                         (expected 'point' or {{kind='rect', width, height}})"
                    )));
                }
            }

            if let Ok(offset_table) = table.get::<LuaTable>("offset") {
                data.offset_x = offset_table.get("x").unwrap_or(0.0);
                data.offset_y = offset_table.get("y").unwrap_or(0.0);
            }

            // Read as f64: a direct u32 read would silently truncate 2.5 to 2.
            let count = |field: &str| -> LuaResult<Option<u32>> {
                let err = || {
                    LuaError::runtime(format!(
                        "with_particle_emitter: {field} must be a non-negative integer"
                    ))
                };
                match table.get::<Option<f64>>(field).map_err(|_| err())? {
                    None => Ok(None),
                    Some(n) if n >= 0.0 && n.fract() == 0.0 && n <= u32::MAX as f64 => {
                        Ok(Some(n as u32))
                    }
                    Some(_) => Err(err()),
                }
            };
            if let Some(v) = count("particles_per_emission")? {
                data.particles_per_emission = v;
            }
            if let Ok(v) = table.get::<f32>("emissions_per_second") {
                data.emissions_per_second = v;
            }
            if let Some(v) = count("emissions_remaining")? {
                data.emissions_remaining = v;
            }

            if let Ok(arc_table) = table.get::<LuaTable>("arc") {
                let min: f32 = arc_table.get(1).unwrap_or(0.0);
                let max: f32 = arc_table.get(2).unwrap_or(360.0);
                if min <= max {
                    data.arc_min_deg = min;
                    data.arc_max_deg = max;
                } else {
                    data.arc_min_deg = max;
                    data.arc_max_deg = min;
                }
            }

            if let Ok(speed_table) = table.get::<LuaTable>("speed") {
                let min: f32 = speed_table.get(1).unwrap_or(50.0);
                let max: f32 = speed_table.get(2).unwrap_or(100.0);
                if min <= max {
                    data.speed_min = min;
                    data.speed_max = max;
                } else {
                    data.speed_min = max;
                    data.speed_max = min;
                }
            }

            match table.get::<LuaValue>("ttl")? {
                LuaValue::Nil => {}
                LuaValue::String(s) if s.to_string_lossy() == "none" => {
                    data.ttl = ParticleTtlData::None;
                }
                LuaValue::Number(n) => {
                    data.ttl = ParticleTtlData::Fixed((n as f32).max(0.0));
                }
                LuaValue::Integer(n) => {
                    data.ttl = ParticleTtlData::Fixed((n as f32).max(0.0));
                }
                LuaValue::Table(ttl_table) => {
                    let min: f32 = ttl_table.get("min").unwrap_or(0.0);
                    let max: f32 = ttl_table.get("max").unwrap_or(0.0);
                    let (min, max) = if min <= max { (min, max) } else { (max, min) };
                    data.ttl = ParticleTtlData::Range {
                        min: min.max(0.0),
                        max: max.max(0.0),
                    };
                }
                other => {
                    return Err(LuaError::runtime(format!(
                        "with_particle_emitter: unknown ttl {other:?} \
                         (expected 'none', a number, or {{min, max}})"
                    )));
                }
            }

            this.cmd.particle_emitter = Some(data);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tilemap",
        "Spawn a tilemap root. All tile entities become ChildOf children so the root's position/scale/rotation transforms the whole tilemap.",
        [("path", "string")],
        |_, this: &mut LuaEntityBuilder, path: String| {
            this.cmd.tilemap_path = Some(path);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_lua_setup",
        "Attach a one-shot Lua setup callback. The named function is called once (Added<LuaSetup>) with the entity context. Fires the frame after spawn; child entities added inside the callback appear the following frame.",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            this.cmd.lua_setup = Some(callback);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_camera_target",
        "Mark entity as camera follow target (higher priority wins). zoom is the desired camera zoom when this target wins (default 1.0).",
        [("priority", "integer?"), ("zoom", "number?")],
        |_, this: &mut LuaEntityBuilder, (priority, zoom): (Option<u8>, Option<f32>)| {
            this.cmd.camera_target = Some(priority.unwrap_or(0));
            this.cmd.camera_target_zoom = zoom;
            Ok(())
        }
    );
}

fn set_lua_timer(this: &mut LuaEntityBuilder, duration: f32, callback: String, mode: TimerMode) {
    this.cmd.lua_timer = Some(LuaTimerSpawn {
        duration,
        callback,
        mode,
    });
}

#[cfg(test)]
mod tests {
    use super::super::super::spawn_data::{
        LuaTimerSpawn, ParticleEmitterData, ParticleEmitterShapeData, ParticleTtlData,
    };
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use aberred_core::components::timer::TimerMode;

    fn built(chain: &str) -> super::super::SpawnCmd {
        built_spawn_cmd(&format!("engine.spawn(){chain}:build()"))
    }

    fn emitter(table_lua: &str) -> ParticleEmitterData {
        built(&format!(":with_particle_emitter({table_lua})"))
            .particle_emitter
            .unwrap()
    }

    #[test]
    fn signals_accumulate_in_call_order() {
        let cmd = built(
            ":with_signals()\
             :with_signal_scalar('speed', 1.5):with_signal_scalar('drag', 0.25)\
             :with_signal_integer('hp', 3)\
             :with_signal_flag('alive'):with_signal_flag('armed')\
             :with_signal_string('name', 'bob')",
        );
        assert!(cmd.has_signals);
        assert_eq!(
            cmd.signal_scalars,
            [("speed".to_string(), 1.5), ("drag".to_string(), 0.25)]
        );
        assert_eq!(cmd.signal_integers, [("hp".to_string(), 3)]);
        assert_eq!(cmd.signal_flags, ["alive", "armed"]);
        assert_eq!(
            cmd.signal_strings,
            [("name".to_string(), "bob".to_string())]
        );
    }

    #[test]
    fn signal_binding_format_requires_binding_and_is_stored() {
        assert_runtime_error(
            "engine.spawn():with_signal_binding_format('{}')",
            "with_signal_binding_format() requires with_signal_binding() first",
        );
        let cmd = built(":with_signal_binding('score')");
        assert_eq!(cmd.signal_binding, Some(("score".to_string(), None)));
        let cmd = built(":with_signal_binding('score'):with_signal_binding_format('Score: {}')");
        assert_eq!(
            cmd.signal_binding,
            Some(("score".to_string(), Some("Score: {}".to_string())))
        );
    }

    #[test]
    fn with_phase_reads_initial_and_optional_callbacks() {
        let phase = built(
            ":with_phase({ initial = 'idle', phases = { \
                idle = { on_enter = 'idle_enter', on_update = 'idle_update', on_exit = 'idle_exit' }, \
                moving = { on_enter = 'moving_enter' } } })",
        )
        .phase_data
        .unwrap();
        assert_eq!(phase.initial, "idle");
        assert_eq!(phase.phases.len(), 2);
        let idle = &phase.phases["idle"];
        assert_eq!(idle.on_enter.as_deref(), Some("idle_enter"));
        assert_eq!(idle.on_update.as_deref(), Some("idle_update"));
        assert_eq!(idle.on_exit.as_deref(), Some("idle_exit"));
        let moving = &phase.phases["moving"];
        assert_eq!(moving.on_enter.as_deref(), Some("moving_enter"));
        assert!(moving.on_update.is_none() && moving.on_exit.is_none());
    }

    #[test]
    fn with_phase_requires_initial_and_tolerates_missing_phases() {
        assert_runtime_error(
            "engine.spawn():with_phase({ phases = {} })",
            "in method 'with_phase'",
        );
        let phase = built(":with_phase({ initial = 'idle' })")
            .phase_data
            .unwrap();
        assert_eq!(phase.initial, "idle");
        assert!(phase.phases.is_empty());
    }

    #[test]
    fn scalar_behaviors_store_their_arguments() {
        let cmd = built(
            ":with_lua_timer(0.5, 'on_tick')\
             :with_ttl(2)\
             :with_grid_layout('levels/l1.json', 'bricks', 3)\
             :with_lua_collision_rule('player', 'enemy', 'on_hit')\
             :with_tilemap('maps/overworld')\
             :with_lua_setup('setup_player')",
        );
        assert_eq!(
            cmd.lua_timer,
            Some(LuaTimerSpawn {
                duration: 0.5,
                callback: "on_tick".to_string(),
                mode: TimerMode::Repeat,
            })
        );
        assert_eq!(cmd.ttl, Some(2.0));
        assert_eq!(
            cmd.grid_layout,
            Some(("levels/l1.json".to_string(), "bricks".to_string(), 3.0))
        );
        let rule = cmd.lua_collision_rule.unwrap();
        assert_eq!(
            (
                rule.group_a.as_str(),
                rule.group_b.as_str(),
                rule.callback.as_str()
            ),
            ("player", "enemy", "on_hit")
        );
        assert_eq!(cmd.tilemap_path.as_deref(), Some("maps/overworld"));
        assert_eq!(cmd.lua_setup.as_deref(), Some("setup_player"));
    }

    #[test]
    fn with_lua_timer_once_stores_a_once_timer() {
        let cmd = built(":with_lua_timer_once(1.5, 'boom')");
        assert_eq!(
            cmd.lua_timer,
            Some(LuaTimerSpawn {
                duration: 1.5,
                callback: "boom".to_string(),
                mode: TimerMode::Once,
            })
        );
    }

    #[test]
    fn with_camera_target_defaults_priority_zero_and_zoom_unset() {
        let cmd = built(":with_camera_target()");
        assert_eq!((cmd.camera_target, cmd.camera_target_zoom), (Some(0), None));
        let cmd = built(":with_camera_target(5, 2.0)");
        assert_eq!(
            (cmd.camera_target, cmd.camera_target_zoom),
            (Some(5), Some(2.0))
        );
        let cmd = built(":with_camera_target(nil, 0.5)");
        assert_eq!(
            (cmd.camera_target, cmd.camera_target_zoom),
            (Some(0), Some(0.5))
        );
        assert_runtime_error(
            "engine.spawn():with_camera_target(256)",
            "with_camera_target",
        );
    }

    #[test]
    fn particle_emitter_empty_table_uses_defaults() {
        let e = emitter("{}");
        assert!(e.template_keys.is_empty());
        assert!(matches!(e.shape, ParticleEmitterShapeData::Point));
        assert_eq!((e.offset_x, e.offset_y), (0.0, 0.0));
        assert_eq!(e.particles_per_emission, 1);
        assert_eq!(e.emissions_per_second, 10.0);
        assert_eq!(e.emissions_remaining, 100);
        assert_eq!((e.arc_min_deg, e.arc_max_deg), (0.0, 360.0));
        assert_eq!((e.speed_min, e.speed_max), (50.0, 100.0));
        assert!(matches!(e.ttl, ParticleTtlData::None));
    }

    #[test]
    fn particle_emitter_reads_all_fields() {
        let e = emitter(
            "{ templates = {'spark', 'smoke'}, shape = {kind='rect', width=8, height=4}, \
               offset = {x=1, y=-2}, particles_per_emission = 3, emissions_per_second = 20, \
               emissions_remaining = 5, arc = {30, 60}, speed = {10, 20}, ttl = 1.5 }",
        );
        assert_eq!(e.template_keys, ["spark", "smoke"]);
        assert!(matches!(
            e.shape,
            ParticleEmitterShapeData::Rect {
                width: 8.0,
                height: 4.0
            }
        ));
        assert_eq!((e.offset_x, e.offset_y), (1.0, -2.0));
        assert_eq!(
            (
                e.particles_per_emission,
                e.emissions_per_second,
                e.emissions_remaining
            ),
            (3, 20.0, 5)
        );
        assert_eq!((e.arc_min_deg, e.arc_max_deg), (30.0, 60.0));
        assert_eq!((e.speed_min, e.speed_max), (10.0, 20.0));
        assert!(matches!(e.ttl, ParticleTtlData::Fixed(t) if t == 1.5));
    }

    #[test]
    fn particle_emitter_shape_accepts_type_alias_and_point_string() {
        let e = emitter("{ shape = {type='rect', width=2, height=3} }");
        assert!(matches!(
            e.shape,
            ParticleEmitterShapeData::Rect {
                width: 2.0,
                height: 3.0
            }
        ));
        let e = emitter("{ shape = 'point' }");
        assert!(matches!(e.shape, ParticleEmitterShapeData::Point));
        let e = emitter("{ shape = {kind='point'} }");
        assert!(matches!(e.shape, ParticleEmitterShapeData::Point));
    }

    #[test]
    fn particle_emitter_rejects_unknown_shape() {
        for shape in [
            "{kind='circle', radius=4}",
            "{width=2, height=3}",
            "'rect'",
            "42",
        ] {
            assert_runtime_error(
                &format!("engine.spawn():with_particle_emitter({{ shape = {shape} }})"),
                "with_particle_emitter: unknown shape",
            );
        }
    }

    #[test]
    fn particle_emitter_rejects_unknown_ttl() {
        for ttl in ["'forever'", "true"] {
            assert_runtime_error(
                &format!("engine.spawn():with_particle_emitter({{ ttl = {ttl} }})"),
                "with_particle_emitter: unknown ttl",
            );
        }
    }

    #[test]
    fn particle_emitter_swaps_reversed_ranges_and_fills_missing_ends() {
        let e = emitter("{ arc = {90, 10}, speed = {200, 100}, ttl = {min=3, max=1} }");
        assert_eq!((e.arc_min_deg, e.arc_max_deg), (10.0, 90.0));
        assert_eq!((e.speed_min, e.speed_max), (100.0, 200.0));
        assert!(matches!(e.ttl, ParticleTtlData::Range { min, max } if min == 1.0 && max == 3.0));

        let e = emitter("{ arc = {45}, speed = {75} }");
        assert_eq!((e.arc_min_deg, e.arc_max_deg), (45.0, 360.0));
        assert_eq!((e.speed_min, e.speed_max), (75.0, 100.0));
    }

    #[test]
    fn particle_emitter_ttl_variants_clamp_negative_to_zero() {
        assert!(matches!(
            emitter("{ ttl = 'none' }").ttl,
            ParticleTtlData::None
        ));
        assert!(matches!(emitter("{ ttl = 2 }").ttl, ParticleTtlData::Fixed(t) if t == 2.0));
        // Integral and fractional numbers reach different LuaValue arms; clamp both.
        assert!(matches!(emitter("{ ttl = -1 }").ttl, ParticleTtlData::Fixed(t) if t == 0.0));
        assert!(matches!(emitter("{ ttl = -0.5 }").ttl, ParticleTtlData::Fixed(t) if t == 0.0));
        assert!(matches!(
            emitter("{ ttl = {min=-2, max=1} }").ttl,
            ParticleTtlData::Range { min, max } if min == 0.0 && max == 1.0
        ));
    }

    #[test]
    fn particle_emitter_rejects_negative_or_fractional_counts() {
        for field in ["particles_per_emission", "emissions_remaining"] {
            for bad in ["-1", "2.5", "'three'"] {
                assert_runtime_error(
                    &format!("engine.spawn():with_particle_emitter({{ {field} = {bad} }})"),
                    &format!("with_particle_emitter: {field} must be a non-negative integer"),
                );
            }
        }
        // Omitted counts still take the defaults.
        let e = emitter("{}");
        assert_eq!((e.particles_per_emission, e.emissions_remaining), (1, 100));
    }
}
