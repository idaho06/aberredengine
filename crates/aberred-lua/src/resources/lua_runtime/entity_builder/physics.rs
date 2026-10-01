use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_velocity",
        "Set velocity (creates RigidBody if needed)",
        [("vx", "number"), ("vy", "number")],
        |_, this: &mut LuaEntityBuilder, (vx, vy): (f32, f32)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.velocity_x = vx;
                rb.velocity_y = vy;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    velocity_x: vx,
                    velocity_y: vy,
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_friction",
        "Set friction (creates RigidBody if needed)",
        [("friction", "number")],
        |_, this: &mut LuaEntityBuilder, friction: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.friction = friction;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    friction,
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_max_speed",
        "Set max speed clamp (creates RigidBody if needed)",
        [("speed", "number")],
        |_, this: &mut LuaEntityBuilder, speed: f32| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.max_speed = Some(speed);
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    max_speed: Some(speed),
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_accel",
        "Add a named acceleration force",
        [
            ("name", "string"),
            ("x", "number"),
            ("y", "number"),
            ("enabled", "boolean"),
        ],
        |_, this: &mut LuaEntityBuilder, (name, x, y, enabled): (String, f32, f32, bool)| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.forces.push(ForceData {
                    name,
                    x,
                    y,
                    enabled,
                });
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    forces: vec![ForceData {
                        name,
                        x,
                        y,
                        enabled,
                    }],
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_frozen",
        "Mark entity as frozen (physics skipped)",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            if let Some(ref mut rb) = this.cmd.rigidbody {
                rb.frozen = true;
            } else {
                this.cmd.rigidbody = Some(RigidBodyData {
                    frozen: true,
                    ..RigidBodyData::default()
                });
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_collider",
        "Set box collider",
        [
            ("width", "number"),
            ("height", "number"),
            ("origin_x", "number"),
            ("origin_y", "number"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (width, height, origin_x, origin_y): (f32, f32, f32, f32)| {
            this.cmd.collider = Some(ColliderData {
                width,
                height,
                offset_x: 0.0,
                offset_y: 0.0,
                origin_x,
                origin_y,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_collider_offset",
        "Set collider offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut collider) = this.cmd.collider else {
                return Err(LuaError::runtime(
                    "with_collider_offset() requires with_collider() first",
                ));
            };
            collider.offset_x = offset_x;
            collider.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_mouse_controlled",
        "Enable mouse position tracking",
        [("follow_x", "boolean"), ("follow_y", "boolean")],
        |_, this: &mut LuaEntityBuilder, (follow_x, follow_y): (bool, bool)| {
            this.cmd.mouse_controlled = Some((follow_x, follow_y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_stuckto",
        "Attach entity to a target entity",
        [
            ("target_entity_id", "integer"),
            ("follow_x", "boolean"),
            ("follow_y", "boolean"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (target_entity_id, follow_x, follow_y): (u64, bool, bool)| {
            this.cmd.stuckto = Some(StuckToData {
                target_entity_id,
                offset_x: 0.0,
                offset_y: 0.0,
                follow_x,
                follow_y,
                stored_velocity: None,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_stuckto_offset",
        "Set offset for StuckTo",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut stuckto) = this.cmd.stuckto else {
                return Err(LuaError::runtime(
                    "with_stuckto_offset() requires with_stuckto() first",
                ));
            };
            stuckto.offset_x = offset_x;
            stuckto.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_stuckto_stored_velocity",
        "Set velocity to restore when unstuck",
        [("vx", "number"), ("vy", "number")],
        |_, this: &mut LuaEntityBuilder, (vx, vy): (f32, f32)| {
            let Some(ref mut stuckto) = this.cmd.stuckto else {
                return Err(LuaError::runtime(
                    "with_stuckto_stored_velocity() requires with_stuckto() first",
                ));
            };
            stuckto.stored_velocity = Some((vx, vy));
            Ok(())
        }
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use super::*;

    fn built(chain: &str) -> super::super::SpawnCmd {
        built_spawn_cmd(&format!("engine.spawn(){chain}:build()"))
    }

    fn rb_summary(rb: &RigidBodyData) -> (f32, f32, f32, Option<f32>, bool, Vec<String>) {
        let forces = rb
            .forces
            .iter()
            .map(|f| format!("{}:{},{}:{}", f.name, f.x, f.y, f.enabled))
            .collect();
        (
            rb.velocity_x,
            rb.velocity_y,
            rb.friction,
            rb.max_speed,
            rb.frozen,
            forces,
        )
    }

    #[test]
    fn each_rigidbody_method_creates_a_default_body_when_absent() {
        let rb = built(":with_velocity(3, -4)").rigidbody.unwrap();
        assert_eq!(rb_summary(&rb), (3.0, -4.0, 0.0, None, false, vec![]));
        let rb = built(":with_friction(0.5)").rigidbody.unwrap();
        assert_eq!(rb_summary(&rb), (0.0, 0.0, 0.5, None, false, vec![]));
        let rb = built(":with_max_speed(200)").rigidbody.unwrap();
        assert_eq!(rb_summary(&rb), (0.0, 0.0, 0.0, Some(200.0), false, vec![]));
        let rb = built(":with_accel('gravity', 0, 98, true)")
            .rigidbody
            .unwrap();
        assert_eq!(
            rb_summary(&rb),
            (
                0.0,
                0.0,
                0.0,
                None,
                false,
                vec!["gravity:0,98:true".to_string()]
            )
        );
        let rb = built(":with_frozen()").rigidbody.unwrap();
        assert_eq!(rb_summary(&rb), (0.0, 0.0, 0.0, None, true, vec![]));
        assert!(built("").rigidbody.is_none());
    }

    #[test]
    fn rigidbody_methods_merge_in_any_order() {
        let forward = built(
            ":with_velocity(1, 2):with_friction(0.3):with_max_speed(50)\
             :with_accel('gravity', 0, 98, true):with_accel('wind', 5, 0, false):with_frozen()",
        )
        .rigidbody
        .unwrap();
        let reversed = built(
            ":with_frozen():with_accel('gravity', 0, 98, true):with_accel('wind', 5, 0, false)\
             :with_max_speed(50):with_friction(0.3):with_velocity(1, 2)",
        )
        .rigidbody
        .unwrap();
        let expected = (
            1.0,
            2.0,
            0.3,
            Some(50.0),
            true,
            vec![
                "gravity:0,98:true".to_string(),
                "wind:5,0:false".to_string(),
            ],
        );
        assert_eq!(rb_summary(&forward), expected);
        assert_eq!(rb_summary(&reversed), expected);
    }

    #[test]
    fn repeated_rigidbody_setters_last_call_wins() {
        let rb =
            built(":with_velocity(1, 1):with_velocity(7, 8):with_friction(1):with_friction(2)")
                .rigidbody
                .unwrap();
        assert_eq!((rb.velocity_x, rb.velocity_y, rb.friction), (7.0, 8.0, 2.0));
    }

    #[test]
    fn collider_offset_defaults_zero_and_is_set_after_collider() {
        let c = built(":with_collider(20, 10, 5, 2)").collider.unwrap();
        assert_eq!(
            (
                c.width, c.height, c.origin_x, c.origin_y, c.offset_x, c.offset_y
            ),
            (20.0, 10.0, 5.0, 2.0, 0.0, 0.0)
        );
        let c = built(":with_collider(20, 10, 5, 2):with_collider_offset(3, 4)")
            .collider
            .unwrap();
        assert_eq!((c.offset_x, c.offset_y), (3.0, 4.0));
    }

    #[test]
    fn stuckto_modifiers_require_stuckto_and_set_offset_and_velocity() {
        assert_runtime_error(
            "engine.spawn():with_stuckto_stored_velocity(1, 2)",
            "with_stuckto_stored_velocity() requires with_stuckto() first",
        );
        let s = built(":with_stuckto(42, true, false)").stuckto.unwrap();
        assert_eq!(s.target_entity_id, 42);
        assert_eq!((s.follow_x, s.follow_y), (true, false));
        assert_eq!(
            (s.offset_x, s.offset_y, s.stored_velocity),
            (0.0, 0.0, None)
        );

        let s = built(":with_stuckto(42, true, true):with_stuckto_offset(0, -8):with_stuckto_stored_velocity(100, 0)")
            .stuckto
            .unwrap();
        assert_eq!((s.offset_x, s.offset_y), (0.0, -8.0));
        assert_eq!(s.stored_velocity, Some((100.0, 0.0)));
    }

    #[test]
    fn with_mouse_controlled_stores_axes() {
        assert_eq!(
            built(":with_mouse_controlled(true, false)").mouse_controlled,
            Some((true, false))
        );
    }
}
