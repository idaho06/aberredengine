-- setup.lua
-- Asset loading configuration
-- This file is loaded via require("setup") from main.lua
--
-- All assets for ALL examples are loaded here during on_setup().
-- Organised into sections per example for clarity.

local M = {}

--- Load assets shared across all examples (fonts, cursor, shaders, etc.)
local function load_common()
    engine.log_debug("Loading common assets...")

    -- Fonts
    engine.load_font("arcade", "./assets/fonts/Arcade_Cabinet.ttf", 128)
    engine.load_font("future", "./assets/fonts/Formal_Future.ttf", 128)

    -- Shared textures
    engine.load_texture("cursor", "./assets/textures/cursor.png")
    engine.load_texture("black", "./assets/textures/black.png")

    -- Shared sound effects
    engine.load_sound("option", "./assets/audio/option.wav")

    -- Shaders
    engine.load_shader("invert", nil, "./assets/shaders/invert.fs")
    engine.load_shader("wave", nil, "./assets/shaders/wave.fs")
    engine.load_shader("bloom", nil, "./assets/shaders/bloom.fs")
    engine.load_shader("outline", nil, "./assets/shaders/outline.fs")
    engine.load_shader("crt", nil, "./assets/shaders/crt2.fs")
    engine.load_shader("blink", nil, "./assets/shaders/blink.fs")
    engine.load_shader("fade", nil, "./assets/shaders/fade.fs")
    engine.load_shader("parallax_scroll", nil, "./assets/shaders/parallax_scroll.fs")
end

--- Load assets for the Asteroids example
local function load_asteroids()
    engine.log_debug("Loading Asteroids assets...")

    -- Textures
    -- Asteroids sprites rotate and scale smoothly, so use anisotropic filtering
    -- (paired with engine.set_pixel_snap_camera(false) in level01.lua's M.spawn()).
    local filter = "trilinear"
    engine.load_texture("asteroids-ship_sheet", "./assets/textures/asteroids/ship.png", filter)
    engine.load_texture("asteroids-space01", "./assets/textures/asteroids/space01.png", filter)
    engine.load_texture("asteroids-space02", "./assets/textures/asteroids/space02.png", filter)
    engine.load_texture("asteroids-space03", "./assets/textures/asteroids/space03.png", filter)
    engine.load_texture("asteroids-space04", "./assets/textures/asteroids/space04.png", filter)
    engine.load_texture("asteroids-big01", "./assets/textures/asteroids/big01.png", filter)
    engine.load_texture("asteroids-big02", "./assets/textures/asteroids/big02.png", filter)
    engine.load_texture("asteroids-big03", "./assets/textures/asteroids/big03.png", filter)
    engine.load_texture("asteroids-medium01", "./assets/textures/asteroids/medium01.png", filter)
    engine.load_texture("asteroids-medium02", "./assets/textures/asteroids/medium02.png", filter)
    engine.load_texture("asteroids-medium03", "./assets/textures/asteroids/medium03.png", filter)
    engine.load_texture("asteroids-small01", "./assets/textures/asteroids/small01.png", filter)
    engine.load_texture("asteroids-small02", "./assets/textures/asteroids/small02.png", filter)
    engine.load_texture("asteroids-small03", "./assets/textures/asteroids/small03.png", filter)
    engine.load_texture("asteroids-laser", "./assets/textures/asteroids/laser.png", filter)
    engine.load_texture("asteroids-explosion01_sheet", "./assets/textures/asteroids/explosion01.png", filter)
    engine.load_texture("asteroids-explosion02_sheet", "./assets/textures/asteroids/explosion02.png", filter)
    engine.load_texture("asteroids-explosion03_sheet", "./assets/textures/asteroids/explosion03.png", filter)
    engine.load_texture("asteroids-stars01_sheet", "./assets/textures/asteroids/stars01.png", filter)

    -- Sound effects
    engine.load_sound("asteroids-blaster", "./assets/audio/asteroids/blaster.ogg")
    engine.load_sound("asteroids-scanner", "./assets/audio/asteroids/scanner.ogg")
    engine.load_sound("asteroids-explosion01", "./assets/audio/asteroids/explosion01.ogg")

    -- Animations
    engine.register_animation("asteroids-ship_idle", "asteroids-ship_sheet", 0, 0, 64, 0, 8, 15, true)
    engine.register_animation("asteroids-ship_propulsion", "asteroids-ship_sheet", 0, 64, 64, 0, 8, 15, true)
    engine.register_animation("asteroids-explosion01", "asteroids-explosion01_sheet", 0, 0, 64, 0, 16, 20, false)
    engine.register_animation("asteroids-explosion02", "asteroids-explosion02_sheet", 0, 0, 32, 0, 7, 20, false)
    engine.register_animation("asteroids-explosion03", "asteroids-explosion03_sheet", 0, 0, 16, 0, 7, 20, false)
end

--- Load assets for the Arkanoid example
local function load_arkanoid()
    engine.log_debug("Loading Arkanoid assets...")

    -- Textures
    engine.load_texture("arkanoid-title", "./assets/textures/arkanoid/title.png")
    engine.load_texture("arkanoid-background", "./assets/textures/arkanoid/background01.png")
    engine.load_texture("arkanoid-vaus", "./assets/textures/arkanoid/vaus.png")
    engine.load_texture("arkanoid-ball", "./assets/textures/arkanoid/ball_12.png")
    engine.load_texture("arkanoid-brick_red", "./assets/textures/arkanoid/brick_red.png")
    engine.load_texture("arkanoid-brick_green", "./assets/textures/arkanoid/brick_green.png")
    engine.load_texture("arkanoid-brick_blue", "./assets/textures/arkanoid/brick_blue.png")
    engine.load_texture("arkanoid-brick_yellow", "./assets/textures/arkanoid/brick_yellow.png")
    engine.load_texture("arkanoid-brick_purple", "./assets/textures/arkanoid/brick_purple.png")
    engine.load_texture("arkanoid-brick_silver", "./assets/textures/arkanoid/brick_silver.png")
    engine.load_texture("arkanoid-vaus_sheet", "./assets/textures/arkanoid/vaus_sheet.png")

    -- Music
    engine.load_music("arkanoid-boss_fight", "./assets/audio/arkanoid/boss_fight.xm")
    engine.load_music("arkanoid-journey_begins", "./assets/audio/arkanoid/journey_begins.xm")
    engine.load_music("arkanoid-player_ready", "./assets/audio/arkanoid/player_ready.xm")
    engine.load_music("arkanoid-success", "./assets/audio/arkanoid/success.xm")
    engine.load_music("arkanoid-menu_music", "./assets/audio/arkanoid/woffy_-_arkanoid_cover.xm")

    -- Sound effects
    engine.load_sound("arkanoid-ding", "./assets/audio/arkanoid/ding.wav")
    engine.load_sound("arkanoid-ping", "./assets/audio/arkanoid/ping.wav")

    -- Animations
    engine.register_animation("arkanoid-vaus_glowing", "arkanoid-vaus_sheet", 0, 0, 96, 0, 16, 15, true)
    engine.register_animation("arkanoid-vaus_hit", "arkanoid-vaus_sheet", 0, 24, 96, 0, 6, 15, false)
end

--- Load assets for the Birthday Card example
local function load_birthday()
    engine.log_debug("Loading Birthday Card assets...")

    -- Fonts
    engine.load_font("birthday-love", "./assets/fonts/birthday/Endless_Love.ttf", 120)

    -- Textures
    engine.load_texture("birthday-spin_hearts-sheet", "./assets/textures/birthday/Hearts.png")
    engine.load_texture("birthday-beat_hearts-sheet", "./assets/textures/birthday/HeartsBeat.png")
    engine.load_texture("birthday-big_heart-sheet", "./assets/textures/birthday/bigheart-sheet.png")
    engine.load_texture("birthday-white", "./assets/textures/birthday/white.png")
    engine.load_texture("birthday-raquel_back", "./assets/textures/birthday/espaldas_small.png")
    engine.load_texture("birthday-gems-sheet", "./assets/textures/birthday/gemstones-sheet.png")

    -- Music
    engine.load_music("birthday-birthday_music", "./assets/audio/birthday/birthday.ogg")
    engine.load_music("birthday-harry", "./assets/audio/birthday/adore_you_karaoke_harry_styles.ogg")

    -- Animations
    engine.register_animation("birthday-heart_beat_big", "birthday-big_heart-sheet", 0, 0, 400, 0, 10, 15, true)
    engine.register_animation("birthday-heart_spin01", "birthday-spin_hearts-sheet", 0, 0, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin02", "birthday-spin_hearts-sheet", 0, 16, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin03", "birthday-spin_hearts-sheet", 0, 32, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin04", "birthday-spin_hearts-sheet", 0, 48, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin05", "birthday-spin_hearts-sheet", 0, 64, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin06", "birthday-spin_hearts-sheet", 0, 80, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_spin07", "birthday-spin_hearts-sheet", 0, 96, 16, 0, 6, 15, true)
    engine.register_animation("birthday-heart_beat01", "birthday-beat_hearts-sheet", 0, 0, 16, 0, 4, 12, true)
    engine.register_animation("birthday-heart_beat02", "birthday-beat_hearts-sheet", 0, 16, 16, 0, 4, 12, true)
    engine.register_animation("birthday-heart_beat03", "birthday-beat_hearts-sheet", 0, 32, 16, 0, 4, 12, true)
    engine.register_animation("birthday-heart_beat04", "birthday-beat_hearts-sheet", 0, 48, 16, 0, 4, 12, true)
    engine.register_animation("birthday-heart_beat05", "birthday-beat_hearts-sheet", 0, 64, 16, 0, 4, 12, true)
    engine.register_animation("birthday-heart_beat06", "birthday-beat_hearts-sheet", 0, 80, 16, 0, 4, 12, true)
end

--- Load assets for the Kraken example
local function load_kraken()
    engine.log_debug("Loading Kraken assets...")

    -- Textures
    -- Kraken tentacles rotate/stretch smoothly, so use anisotropic filtering.
    local filter = "trilinear"
    engine.load_texture("kraken-mouth", "./assets/textures/kraken/mouth.png", filter)
    engine.load_texture("kraken-tentacle", "./assets/textures/kraken/tentacle.png", filter)
end

--- Load assets for the Sidescroller example
local function load_sidescroller()
    engine.log_debug("Loading Sidescroller assets...")
    -- Textures
    engine.load_texture("sidescroller-char_red_1_sheet", "./assets/textures/sidescroller/char_red_1.png")
    engine.load_texture("sidescroller-char_red_2_sheet", "./assets/textures/sidescroller/char_red_2.png")
    engine.load_texture("sidescroller01", "./assets/textures/sidescroller/sidescroller01.png")
    engine.load_texture("shop_animation", "./assets/textures/sidescroller/shop_anim.png")
    engine.load_texture("back_layer01", "./assets/textures/sidescroller/background_layer_1.png")
    engine.load_texture("back_layer02", "./assets/textures/sidescroller/background_layer_2.png")
    engine.load_texture("back_layer03", "./assets/textures/sidescroller/background_layer_3.png")

    -- Animations
    local sprite_size = 56
    engine.register_animation("sidescroller-char_red_idle", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 0, sprite_size, 0, 6, 10, true)
    engine.register_animation("sidescroller-char_red_attack", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 1, sprite_size, 0, 6, 10, false)
    engine.register_animation("sidescroller-char_red_attack_combo", "sidescroller-char_red_1_sheet",
        sprite_size * 6, sprite_size * 1, sprite_size, 0, 2, 10, false)
    engine.register_animation("sidescroller-char_red_run", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 2, sprite_size, 0, 8, 10, true)
    engine.register_animation("sidescroller-char_red_jump_prep", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 3, sprite_size, 0, 2, 10, false)
    engine.register_animation("sidescroller-char_red_jump_up", "sidescroller-char_red_1_sheet",
        sprite_size * 2, sprite_size * 3, sprite_size, 0, 4, 10, false)
    engine.register_animation("sidescroller-char_red_jump_reload", "sidescroller-char_red_1_sheet",
        sprite_size * 6, sprite_size * 3, sprite_size, sprite_size, 3, 10, false) -- 2 frames in the first row, then 1 more frame in the second row
    engine.register_animation("sidescroller-char_red_jump_falling", "sidescroller-char_red_1_sheet",
        sprite_size * 1, sprite_size * 4, sprite_size, 0, 4, 10, false)
    engine.register_animation("sidescroller-char_red_jump_landing", "sidescroller-char_red_1_sheet",
        sprite_size * 5, sprite_size * 4, sprite_size, 0, 3, 10, false)
    engine.register_animation("sidescroller-char_red_jump", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 3, sprite_size, sprite_size, 8 * 2, 10, false) -- two rows of 8 frames each for the full jump animation
    engine.register_animation("sidescroller-char_red_damage", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 5, sprite_size, 0, 4, 10, false)
    engine.register_animation("sidescroller-char_red_death", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 6, sprite_size, sprite_size, 8 + 4, 10, false) -- two rows of 8 frames, then 4 more frames in a third row
    engine.register_animation("sidescroller-char_red_cast", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 8, sprite_size, 0, 8, 10, false)
    engine.register_animation("sidescroller-char_red_crouch", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 9, sprite_size, 0, 3, 10, false)
    engine.register_animation("sidescroller-char_red_shield", "sidescroller-char_red_1_sheet",
        sprite_size * 0, sprite_size * 10, sprite_size, 0, 3, 10, false)
    engine.register_animation("sidescroller-char_red_walk", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 0, sprite_size, sprite_size, 8 + 2, 10, true) -- 8 frames in the first row, then 2 more frames in the second row for the full walk cycle
    engine.register_animation("sidescroller-char_red_slide_start", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 2, sprite_size, 0, 3, 10, false)
    engine.register_animation("sidescroller-char_red_slide_loop", "sidescroller-char_red_2_sheet",
        sprite_size * 3, sprite_size * 2, sprite_size, 0, 3, 10, true)
    engine.register_animation("sidescroller-char_red_slide_end", "sidescroller-char_red_2_sheet",
        sprite_size * 6, sprite_size * 2, sprite_size, 0, 2, 10, false)
    engine.register_animation("sidescroller-char_red_slide", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 2, sprite_size, 0, 8, 10, false)
    engine.register_animation("sidescroller-char_red_wall_slide", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 3, sprite_size, 0, 3, 10, true)
    engine.register_animation("sidescroller-char_red_wall", "sidescroller-char_red_2_sheet",
        sprite_size * 3, sprite_size * 3, sprite_size, 0, 1, 10, false)
    engine.register_animation("sidescroller-char_red_attack_critical", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 4, sprite_size, 0, 8, 10, false)
    engine.register_animation("sidescroller-char_red_ladder", "sidescroller-char_red_2_sheet",
        sprite_size * 0, sprite_size * 5, sprite_size, sprite_size, 8 + 2, 10, false)

end

--- Load assets for the Bunnymark example
local function load_bunnymark()
    engine.log_debug("Loading Bunnymark assets...")
    engine.load_texture("bunnymark-raybunny", "./assets/textures/bunnymark/raybunny.png")
end

--- Load assets and register GUI themes for the GUI Demo example.
--
-- Theme registration (engine.set_gui_theme_*) lives here, not in
-- gui_demo.lua's M.spawn(): those calls queue into gui_theme_commands, a
-- "preserve"-policy queue (see queue_registry.rs) that survives the first
-- switch_scene's clear_all_commands() — unlike most RenderCmd-backed calls
-- (e.g. post-process), theme setup is initial game configuration, not
-- per-scene state, so it belongs alongside the rest of on_setup()'s asset
-- loading.
local function load_gui_demo()
    engine.log_debug("Loading GUI Demo assets...")
    engine.load_texture("gui-bluewindow", "./assets/textures/gui/bluewindow_6_6_6_6.png")
    engine.load_texture("gui-button-atlas", "./assets/textures/gui/button_atlas_8_8_8_8.png")
    engine.load_texture("gui-label", "./assets/textures/gui/label_6_6_6_6.png")

    -- "default" theme: bluewindow_6_6_6_6.png is 64x64 with 6px nine-patch
    -- borders on all sides (encoded in the filename). Every set_gui_theme_*
    -- call takes the theme name as its first argument (see
    -- docs/gui-system-architecture.md, Roadmap item #2) — a named theme
    -- registered here persists in GuiThemeStore across every scene switch
    -- for the rest of the game's lifetime, not just this one scene.
    engine.set_gui_theme_panel("default", "gui-bluewindow", 0, 0, 64, 64, 6, 6, 6, 6)

    -- Button skin: button_atlas_8_8_8_8.png is a 128x128 2x2 grid of 64x64
    -- cells (top-left=normal, top-right=hover, bottom-left=pressed,
    -- bottom-right=disabled), 8px nine-patch borders on all sides (encoded
    -- in the filename, same convention as bluewindow_6_6_6_6.png above).
    engine.set_gui_theme_button("default", "normal", "gui-button-atlas", 0, 0, 64, 64, 8, 8, 8, 8)
    -- engine.set_gui_theme_button("default", "hover", "gui-button-atlas", 64, 0, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "pressed", "gui-button-atlas", 0, 64, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "disabled", "gui-button-atlas", 64, 64, 64, 64, 8, 8, 8, 8)

    -- Label skin: label_6_6_6_6.png is 64x64 with 6px nine-patch borders on
    -- all sides (encoded in the filename, same convention as
    -- bluewindow_6_6_6_6.png above).
    engine.set_gui_theme_label("default", "gui-label", 0, 0, 64, 64, 6, 6, 6, 6)

    -- Caption font/size/color for every "default"-themed GuiButton/GuiLabel
    -- caption — set once here rather than per :with_gui_button()/
    -- :with_gui_label() call.
    engine.set_gui_theme_font("default", "arcade", 16, 255, 255, 255, 255)

    -- "compact" theme: a second named theme, mixed into the same scene
    -- alongside "default" — gui_demo.lua's window 2 and its Hide button
    -- reference it via :with_gui_theme_key("compact") instead of inheriting
    -- "default" from anywhere (theme_key is flat/explicit per widget, never
    -- inherited from a parent GuiWindow). Reuses the same atlas textures as
    -- "default" but with a smaller, tinted caption font, to demonstrate
    -- that two themes can coexist and render independently in one scene —
    -- not that they must use different art.
    engine.set_gui_theme_panel("compact", "gui-bluewindow", 0, 0, 64, 64, 6, 6, 6, 6)
    engine.set_gui_theme_button("compact", "normal", "gui-button-atlas", 0, 0, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("compact", "pressed", "gui-button-atlas", 0, 64, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_font("compact", "arcade", 12, 255, 220, 120, 255)
end

--- Called during the Setup game state to load all assets.
--- Assets are queued here and processed by the Rust engine.
function M.load_assets()
    engine.log_debug("Loading assets from Lua...")

    load_common()
    load_asteroids()
    load_arkanoid()
    load_birthday()
    load_kraken()
    load_sidescroller()
    load_bunnymark()
    load_gui_demo()

    engine.log_debug("All assets queued for loading!")
end

return M
