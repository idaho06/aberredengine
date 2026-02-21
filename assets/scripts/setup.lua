-- setup.lua
-- Asset loading configuration
-- This file is loaded via require("setup") from main.lua
--
-- All assets for ALL examples are loaded here during on_setup().
-- Organised into sections per example for clarity.

local M = {}

--- Load assets shared across all examples (fonts, cursor, shaders, etc.)
local function load_common()
    engine.log_info("Loading common assets...")

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
end

--- Load assets for the Asteroids example
local function load_asteroids()
    engine.log_info("Loading Asteroids assets...")

    -- Textures
    engine.load_texture("asteroids-ship_sheet", "./assets/textures/asteroids/ship.png")
    engine.load_texture("asteroids-space01", "./assets/textures/asteroids/space01.png")
    engine.load_texture("asteroids-space02", "./assets/textures/asteroids/space02.png")
    engine.load_texture("asteroids-space03", "./assets/textures/asteroids/space03.png")
    engine.load_texture("asteroids-space04", "./assets/textures/asteroids/space04.png")
    engine.load_texture("asteroids-big01", "./assets/textures/asteroids/big01.png")
    engine.load_texture("asteroids-big02", "./assets/textures/asteroids/big02.png")
    engine.load_texture("asteroids-big03", "./assets/textures/asteroids/big03.png")
    engine.load_texture("asteroids-medium01", "./assets/textures/asteroids/medium01.png")
    engine.load_texture("asteroids-medium02", "./assets/textures/asteroids/medium02.png")
    engine.load_texture("asteroids-medium03", "./assets/textures/asteroids/medium03.png")
    engine.load_texture("asteroids-small01", "./assets/textures/asteroids/small01.png")
    engine.load_texture("asteroids-small02", "./assets/textures/asteroids/small02.png")
    engine.load_texture("asteroids-small03", "./assets/textures/asteroids/small03.png")
    engine.load_texture("asteroids-laser", "./assets/textures/asteroids/laser.png")
    engine.load_texture("asteroids-explosion01_sheet", "./assets/textures/asteroids/explosion01.png")
    engine.load_texture("asteroids-explosion02_sheet", "./assets/textures/asteroids/explosion02.png")
    engine.load_texture("asteroids-explosion03_sheet", "./assets/textures/asteroids/explosion03.png")
    engine.load_texture("asteroids-stars01_sheet", "./assets/textures/asteroids/stars01.png")

    -- Sound effects
    engine.load_sound("asteroids-blaster", "./assets/audio/asteroids/blaster.ogg")
    engine.load_sound("asteroids-scanner", "./assets/audio/asteroids/scanner.ogg")
    engine.load_sound("asteroids-explosion01", "./assets/audio/asteroids/explosion01.ogg")

    -- Animations
    engine.register_animation("asteroids-ship_idle", "asteroids-ship_sheet", 0, 0, 64, 8, 15, true)
    engine.register_animation("asteroids-ship_propulsion", "asteroids-ship_sheet", 0, 64, 64, 8, 15, true)
    engine.register_animation("asteroids-explosion01", "asteroids-explosion01_sheet", 0, 0, 64, 16, 20, false)
    engine.register_animation("asteroids-explosion02", "asteroids-explosion02_sheet", 0, 0, 32, 7, 20, false)
    engine.register_animation("asteroids-explosion03", "asteroids-explosion03_sheet", 0, 0, 16, 7, 20, false)
end

--- Load assets for the Arkanoid example
--- TODO: Copy assets from ../arkanoid and uncomment
local function load_arkanoid()
    engine.log_info("Loading Arkanoid assets (stubs)...")

    -- Textures
    -- engine.load_texture("arkanoid-title", "./assets/textures/arkanoid/title.png")
    -- engine.load_texture("arkanoid-background", "./assets/textures/arkanoid/background01.png")
    -- engine.load_texture("arkanoid-vaus", "./assets/textures/arkanoid/vaus.png")
    -- engine.load_texture("arkanoid-ball", "./assets/textures/arkanoid/ball_12.png")
    -- engine.load_texture("arkanoid-brick_red", "./assets/textures/arkanoid/brick_red.png")
    -- engine.load_texture("arkanoid-brick_green", "./assets/textures/arkanoid/brick_green.png")
    -- engine.load_texture("arkanoid-brick_blue", "./assets/textures/arkanoid/brick_blue.png")
    -- engine.load_texture("arkanoid-brick_yellow", "./assets/textures/arkanoid/brick_yellow.png")
    -- engine.load_texture("arkanoid-brick_purple", "./assets/textures/arkanoid/brick_purple.png")
    -- engine.load_texture("arkanoid-brick_silver", "./assets/textures/arkanoid/brick_silver.png")
    -- engine.load_texture("arkanoid-vaus_sheet", "./assets/textures/arkanoid/vaus_sheet.png")

    -- Music
    -- engine.load_music("arkanoid-boss_fight", "./assets/audio/arkanoid/boss_fight.xm")
    -- engine.load_music("arkanoid-journey_begins", "./assets/audio/arkanoid/journey_begins.xm")
    -- engine.load_music("arkanoid-player_ready", "./assets/audio/arkanoid/player_ready.xm")
    -- engine.load_music("arkanoid-success", "./assets/audio/arkanoid/success.xm")
    -- engine.load_music("arkanoid-menu", "./assets/audio/arkanoid/woffy_-_arkanoid_cover.xm")

    -- Sound effects
    -- engine.load_sound("arkanoid-ding", "./assets/audio/arkanoid/ding.wav")
    -- engine.load_sound("arkanoid-ping", "./assets/audio/arkanoid/ping.wav")

    -- Tilemaps
    -- engine.load_tilemap("arkanoid-level01", "./assets/tilemaps/arkanoid/level01")

    -- Animations
    -- engine.register_animation("arkanoid-vaus_glowing", "arkanoid-vaus_sheet", 0, 0, 96, 16, 15, true)
    -- engine.register_animation("arkanoid-vaus_hit", "arkanoid-vaus_sheet", 0, 24, 96, 6, 15, false)
end

--- Load assets for the Birthday Card example
--- TODO: Copy assets from ../raquelhb15 and uncomment
local function load_birthday()
    engine.log_info("Loading Birthday Card assets (stubs)...")

    -- Fonts
    -- engine.load_font("birthday-love", "./assets/fonts/birthday/love_font.ttf", 128)

    -- Textures
    -- engine.load_texture("birthday-spin_hearts-sheet", "./assets/textures/birthday/spin_hearts.png")
    -- engine.load_texture("birthday-beat_hearts-sheet", "./assets/textures/birthday/beat_hearts.png")
    -- engine.load_texture("birthday-big_heart-sheet", "./assets/textures/birthday/big_heart.png")
    -- engine.load_texture("birthday-white", "./assets/textures/birthday/white.png")
    -- engine.load_texture("birthday-raquel_back", "./assets/textures/birthday/raquel_back.png")
    -- engine.load_texture("birthday-gems-sheet", "./assets/textures/birthday/gems.png")

    -- Music
    -- engine.load_music("birthday-birthday", "./assets/audio/birthday/birthday.ogg")
    -- engine.load_music("birthday-harry", "./assets/audio/birthday/harry.ogg")

    -- Shaders
    -- engine.load_shader("birthday-fade", nil, "./assets/shaders/birthday/fade.fs")

    -- Animations
    -- (heart_spin01-07, heart_beat01-06, heart_beat_big — to be registered when porting)
end

--- Called during the Setup game state to load all assets.
--- Assets are queued here and processed by the Rust engine.
function M.load_assets()
    engine.log_info("Loading assets from Lua...")

    load_common()
    load_asteroids()
    load_arkanoid()
    load_birthday()

    engine.log_info("All assets queued for loading!")
end

return M
