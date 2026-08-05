use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_group",
        "Set entity group",
        [("name", "string")],
        |_, this: &mut LuaEntityBuilder, name: String| {
            this.cmd.group = Some(name);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_position",
        "Set world position",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_zindex",
        "Set render order",
        [("z", "number")],
        |_, this: &mut LuaEntityBuilder, z: f32| {
            this.cmd.zindex = Some(z);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_rotation",
        "Set rotation in degrees",
        [("degrees", "number")],
        |_, this: &mut LuaEntityBuilder, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_scale",
        "Set scale",
        [("sx", "number"), ("sy", "number")],
        |_, this: &mut LuaEntityBuilder, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_persistent",
        "Survive scene transitions",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            this.cmd.persistent = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_screen_position",
        "Set screen position (UI elements). Requires :with_zindex() to render -- screen-space rendering requires ZIndex (mirrors world-space); entities without it are silently excluded, not an error.",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            this.cmd.screen_position = Some((x, y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_parent",
        "Set parent entity for transform hierarchy",
        [("parent_id", "integer")],
        |_, this: &mut LuaEntityBuilder, parent_id: u64| {
            this.cmd.parent = Some(parent_id);
            Ok(())
        }
    );
}
