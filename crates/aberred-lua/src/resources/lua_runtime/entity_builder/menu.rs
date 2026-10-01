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

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use super::*;

    const MENU: &str = "engine.spawn():with_menu(\
        { {id='play', label='Play'}, {id='quit', label='Quit'} }, \
        10, 20, 'arcade', 16, 24, true)";

    fn built_menu(modifiers: &str) -> MenuData {
        built_spawn_cmd(&format!("{MENU}{modifiers}:build()"))
            .menu
            .expect("with_menu sets cmd.menu")
    }

    #[test]
    fn with_menu_parses_items_in_order_and_layout_args() {
        let menu = built_menu("");
        assert_eq!(
            menu.items,
            vec![
                ("play".to_string(), "Play".to_string()),
                ("quit".to_string(), "Quit".to_string()),
            ]
        );
        assert_eq!((menu.origin_x, menu.origin_y), (10.0, 20.0));
        assert_eq!(menu.font, "arcade");
        assert_eq!((menu.font_size, menu.item_spacing), (16.0, 24.0));
        assert!(menu.use_screen_space);
        assert!(menu.normal_color.is_none() && menu.selected_color.is_none());
        assert!(menu.actions.is_empty() && menu.visible_count.is_none());
    }

    #[test]
    fn with_menu_rejects_item_without_label() {
        // mlua's conversion error does not name the missing field; only the traceback
        // points at with_menu.
        assert_runtime_error(
            "engine.spawn():with_menu({ {id='play'} }, 0, 0, 'f', 16, 24, false)",
            "in method 'with_menu'",
        );
    }

    #[test]
    fn menu_modifiers_require_with_menu_first() {
        for (call, name) in [
            ("with_menu_colors(1,2,3,4,5,6,7,8)", "with_menu_colors"),
            ("with_menu_dynamic_text(true)", "with_menu_dynamic_text"),
            ("with_menu_cursor('c')", "with_menu_cursor"),
            (
                "with_menu_selection_sound('s')",
                "with_menu_selection_sound",
            ),
            (
                "with_menu_action_set_scene('i', 's')",
                "with_menu_action_set_scene",
            ),
            (
                "with_menu_action_show_submenu('i', 'm')",
                "with_menu_action_show_submenu",
            ),
            ("with_menu_action_quit('i')", "with_menu_action_quit"),
            ("with_menu_callback('cb')", "with_menu_callback"),
            ("with_menu_visible_count(3)", "with_menu_visible_count"),
        ] {
            assert_runtime_error(
                &format!("engine.spawn():{call}"),
                &format!("{name}() requires with_menu() first"),
            );
        }
    }

    #[test]
    fn with_menu_colors_splits_normal_and_selected() {
        let menu = built_menu(":with_menu_colors(1, 2, 3, 4, 5, 6, 7, 8)");
        let n = menu.normal_color.unwrap();
        let s = menu.selected_color.unwrap();
        assert_eq!((n.r, n.g, n.b, n.a), (1, 2, 3, 4));
        assert_eq!((s.r, s.g, s.b, s.a), (5, 6, 7, 8));
    }

    #[test]
    fn menu_actions_accumulate_in_call_order() {
        let menu = built_menu(
            ":with_menu_action_set_scene('play', 'level01')\
             :with_menu_action_show_submenu('options', 'options_menu')\
             :with_menu_action_quit('quit')",
        );
        let actions: Vec<String> = menu
            .actions
            .iter()
            .map(|(id, action)| match action {
                MenuActionData::SetScene { scene } => format!("{id}:scene:{scene}"),
                MenuActionData::ShowSubMenu { menu } => format!("{id}:submenu:{menu}"),
                MenuActionData::QuitGame => format!("{id}:quit"),
            })
            .collect();
        assert_eq!(
            actions,
            [
                "play:scene:level01",
                "options:submenu:options_menu",
                "quit:quit"
            ]
        );
    }

    #[test]
    fn menu_modifiers_set_optional_fields() {
        let menu = built_menu(
            ":with_menu_dynamic_text(true)\
             :with_menu_cursor('cursor')\
             :with_menu_selection_sound('blip')\
             :with_menu_callback('on_menu')\
             :with_menu_visible_count(3)",
        );
        assert_eq!(menu.dynamic_text, Some(true));
        assert_eq!(menu.cursor_entity_key.as_deref(), Some("cursor"));
        assert_eq!(menu.selection_change_sound.as_deref(), Some("blip"));
        assert_eq!(menu.on_select_callback.as_deref(), Some("on_menu"));
        assert_eq!(menu.visible_count, Some(3));
    }
}
