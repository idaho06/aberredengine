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
