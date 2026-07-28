use super::*;

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
        "Add a Lua timer callback",
        [("duration", "number"), ("callback", "string")],
        |_, this: &mut LuaEntityBuilder, (duration, callback): (f32, String)| {
            this.cmd.lua_timer = Some((duration, callback));
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

            if let Ok(shape_value) = table.get::<LuaValue>("shape") {
                match shape_value {
                    LuaValue::String(s) if s.to_string_lossy() == "point" => {
                        data.shape = ParticleEmitterShapeData::Point;
                    }
                    LuaValue::Table(shape_table) => {
                        let kind: String = shape_table
                            .get("kind")
                            .or_else(|_| shape_table.get("type"))
                            .unwrap_or_default();
                        if kind == "rect" {
                            let width: f32 = shape_table.get("width").unwrap_or(0.0);
                            let height: f32 = shape_table.get("height").unwrap_or(0.0);
                            data.shape = ParticleEmitterShapeData::Rect { width, height };
                        }
                    }
                    _ => {}
                }
            }

            if let Ok(offset_table) = table.get::<LuaTable>("offset") {
                data.offset_x = offset_table.get("x").unwrap_or(0.0);
                data.offset_y = offset_table.get("y").unwrap_or(0.0);
            }

            if let Ok(v) = table.get::<u32>("particles_per_emission") {
                data.particles_per_emission = v;
            }
            if let Ok(v) = table.get::<f32>("emissions_per_second") {
                data.emissions_per_second = v;
            }
            if let Ok(v) = table.get::<u32>("emissions_remaining") {
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

            if let Ok(ttl_value) = table.get::<LuaValue>("ttl") {
                match ttl_value {
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
                    _ => {}
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
