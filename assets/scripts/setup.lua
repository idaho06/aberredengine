-- setup.lua
-- Asset loading configuration
-- This file is loaded via require("setup") from main.lua

local M = {}

--- Called during the Setup game state to load all assets.
--- Assets are queued here and processed by the Rust engine.
function M.load_assets()
    engine.log_info("Loading assets from Lua...")

    -- Fonts
    engine.load_font("arcade", "./assets/fonts/Arcade_Cabinet.ttf", 128)
    engine.load_font("future", "./assets/fonts/Formal_Future.ttf", 128)

    -- Textures
    engine.load_texture("title", "./assets/textures/title.png")
    engine.load_texture("background", "./assets/textures/background01.png")
    engine.load_texture("cursor", "./assets/textures/cursor.png")
    engine.load_texture("vaus", "./assets/textures/vaus.png")
    engine.load_texture("ball", "./assets/textures/ball_12.png")
    engine.load_texture("brick_red", "./assets/textures/brick_red.png")
    engine.load_texture("brick_green", "./assets/textures/brick_green.png")
    engine.load_texture("brick_blue", "./assets/textures/brick_blue.png")
    engine.load_texture("brick_yellow", "./assets/textures/brick_yellow.png")
    engine.load_texture("brick_purple", "./assets/textures/brick_purple.png")
    engine.load_texture("brick_silver", "./assets/textures/brick_silver.png")

    -- Music
    engine.load_music("boss_fight", "./assets/audio/boss_fight.xm")
    engine.load_music("journey_begins", "./assets/audio/journey_begins.xm")
    engine.load_music("player_ready", "./assets/audio/player_ready.xm")
    engine.load_music("success", "./assets/audio/success.xm")
    engine.load_music("menu", "./assets/audio/woffy_-_arkanoid_cover.xm")

    -- Sound effects
    engine.load_sound("ding", "./assets/audio/ding.wav")
    engine.load_sound("ping", "./assets/audio/ping.wav")
    engine.load_sound("option", "./assets/audio/option.wav")

    engine.log_info("Assets queued for loading!")
end

return M
