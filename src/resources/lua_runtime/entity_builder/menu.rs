use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_menu",
        "Add interactive menu",
        [
            ("items", "table"),
            ("origin_x", "number"),
            ("origin_y", "number"),
            ("font", "string"),
            ("font_size", "number"),
            ("item_spacing", "number"),
            ("use_screen_space", "boolean"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (items_table, origin_x, origin_y, font, font_size, item_spacing, use_screen_space): (
            LuaTable,
            f32,
            f32,
            String,
            f32,
            f32,
            bool
        )| {
            let mut items: Vec<(String, String)> = Vec::new();
            for value in items_table.sequence_values::<LuaTable>() {
                let item_table = value?;
                let id: String = item_table.get("id")?;
                let label: String = item_table.get("label")?;
                items.push((id, label));
            }
            this.cmd.menu = Some(MenuData {
                items,
                origin_x,
                origin_y,
                font,
                font_size,
                item_spacing,
                use_screen_space,
                ..MenuData::default()
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_colors",
        "Set menu normal/selected colors (RGBA)",
        [
            ("nr", "integer"),
            ("ng", "integer"),
            ("nb", "integer"),
            ("na", "integer"),
            ("sr", "integer"),
            ("sg", "integer"),
            ("sb", "integer"),
            ("sa", "integer"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (nr, ng, nb, na, sr, sg, sb, sa): (u8, u8, u8, u8, u8, u8, u8, u8)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_colors() requires with_menu() first",
                ));
            };
            menu.normal_color = Some(ColorData {
                r: nr,
                g: ng,
                b: nb,
                a: na,
            });
            menu.selected_color = Some(ColorData {
                r: sr,
                g: sg,
                b: sb,
                a: sa,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_dynamic_text",
        "Enable dynamic text updates for menu items",
        [("dynamic", "boolean")],
        |_, this: &mut LuaEntityBuilder, dynamic: bool| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_dynamic_text() requires with_menu() first",
                ));
            };
            menu.dynamic_text = Some(dynamic);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_cursor",
        "Set cursor entity for menu",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_cursor() requires with_menu() first",
                ));
            };
            menu.cursor_entity_key = Some(key);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_selection_sound",
        "Set sound for menu selection changes",
        [("sound_key", "string")],
        |_, this: &mut LuaEntityBuilder, sound_key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_selection_sound() requires with_menu() first",
                ));
            };
            menu.selection_change_sound = Some(sound_key);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_action_set_scene",
        "Set scene-switch action for menu item",
        [("item_id", "string"), ("scene", "string")],
        |_, this: &mut LuaEntityBuilder, (item_id, scene): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_set_scene() requires with_menu() first",
                ));
            };
            menu.actions
                .push((item_id, MenuActionData::SetScene { scene }));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_action_show_submenu",
        "Set submenu action for menu item",
        [("item_id", "string"), ("submenu", "string")],
        |_, this: &mut LuaEntityBuilder, (item_id, submenu): (String, String)| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_show_submenu() requires with_menu() first",
                ));
            };
            menu.actions
                .push((item_id, MenuActionData::ShowSubMenu { menu: submenu }));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_action_quit",
        "Set quit action for menu item",
        [("item_id", "string")],
        |_, this: &mut LuaEntityBuilder, item_id: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_quit() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::QuitGame));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_callback",
        "Set Lua callback for menu selection",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_callback() requires with_menu() first",
                ));
            };
            menu.on_select_callback = Some(callback);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_menu_visible_count",
        "Set max visible menu items (enables scrolling)",
        [("count", "integer")],
        |_, this: &mut LuaEntityBuilder, count: usize| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_visible_count() requires with_menu() first",
                ));
            };
            menu.visible_count = Some(count);
            Ok(())
        }
    );
}
