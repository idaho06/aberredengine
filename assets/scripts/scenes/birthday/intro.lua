-- scenes/birthday/intro.lua
-- Birthday Card example — intro scene
-- Scene name: "birthday_intro"
-- Internal render resolution: 618x864

local M = {}

local fade_strength = 1.0

local function spawn_heart_particles_templates()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 0)
        :with_animation("birthday-heart_spin01")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin01")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 16)
        :with_animation("birthday-heart_spin02")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin02")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 32)
        :with_animation("birthday-heart_spin03")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin03")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 48)
        :with_animation("birthday-heart_spin04")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin04")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 64)
        :with_animation("birthday-heart_spin05")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin05")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 80)
        :with_animation("birthday-heart_spin06")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin06")
        :build()
    engine.spawn()
        :with_sprite("birthday-spin_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 96)
        :with_animation("birthday-heart_spin07")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_spin07")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 0)
        :with_animation("birthday-heart_beat01")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat01")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 16)
        :with_animation("birthday-heart_beat02")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat02")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 32)
        :with_animation("birthday-heart_beat03")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat03")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 48)
        :with_animation("birthday-heart_beat04")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat04")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 64)
        :with_animation("birthday-heart_beat05")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat05")
        :build()
    engine.spawn()
        :with_sprite("birthday-beat_hearts-sheet", 16, 16, 8, 8)
        :with_sprite_offset(0, 80)
        :with_animation("birthday-heart_beat06")
        :with_zindex(2)
        :with_signals()
        :register_as("heart_beat06")
        :build()
end

-- ─── Phase callbacks (local) ─────────────────────────────────────────────────

local function scene_fade_in_on_enter(ctx, input)
    fade_strength = 1.0
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)
end

local function scene_fade_in_on_update(ctx, input, dt)
    fade_strength = math.max(fade_strength - dt / 3.0, 0.0)
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    if fade_strength <= 0.0 then
        return "hold"
    end

    if (input.digital.action_1.just_pressed or input.digital.action_2.just_pressed
            or input.digital.up.just_pressed or input.digital.down.just_pressed or input.digital.left.just_pressed
            or input.digital.right.just_pressed) then
        return "fade_out"
    end
end

local function scene_hold_on_enter(ctx, input)
    engine.log_info("Intro scene hold phase entered.")
end

local function scene_hold_on_update(ctx, input, dt)
    if ctx.time_in_phase >= 5.0 and not engine.has_flag("intro_hold_text_shown") then
        engine.spawn()
            :with_position(-50, 400)
            :with_text("Press any key to continue...", "birthday-love", 12 * 2, 255, 32, 32, 255)
            :with_zindex(2)
            :register_as("hold_text")
            :build()
        engine.spawn()
            :with_position(-50 + 2, 400 + 2)
            :with_text("Press any key to continue...", "birthday-love", 12 * 2, 32, 32, 32, 255)
            :with_zindex(1.9)
            :register_as("hold_text_shadow")
            :build()

        engine.set_flag("intro_hold_text_shown")
    end

    if (input.digital.action_1.just_pressed or input.digital.action_2.just_pressed
            or input.digital.up.just_pressed or input.digital.down.just_pressed or input.digital.left.just_pressed
            or input.digital.right.just_pressed) then
        return "fade_out"
    end
end

local function scene_hold_on_exit(ctx)
    if engine.has_flag("intro_hold_text_shown") then
        local hold_text_id = engine.get_entity("hold_text")
        local hold_text_shadow_id = engine.get_entity("hold_text_shadow")
        if hold_text_id and hold_text_shadow_id then
            engine.entity_despawn(hold_text_id)
            engine.entity_despawn(hold_text_shadow_id)
            engine.clear_flag("intro_hold_text_shown")
        end
    end
end

local function scene_fade_out_on_enter(ctx, input)
    fade_strength = 0.0
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)
end

local function scene_fade_out_on_update(ctx, input, dt)
    fade_strength = math.min(fade_strength + dt / 1.5, 1.0)
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    if fade_strength >= 1.0 then
        return "fade_out_done"
    end
end

local function scene_fade_out_on_exit(ctx)
    engine.change_scene("birthday_card")
    engine.log_info("Intro scene fade-out complete. Queued card scene.")
end

--- Called each frame when birthday_intro scene is active.
local function on_update_birthday_intro(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
    on_update_birthday_intro = on_update_birthday_intro,
    scene_fade_in_on_enter   = scene_fade_in_on_enter,
    scene_fade_in_on_update  = scene_fade_in_on_update,
    scene_hold_on_enter      = scene_hold_on_enter,
    scene_hold_on_update     = scene_hold_on_update,
    scene_hold_on_exit       = scene_hold_on_exit,
    scene_fade_out_on_enter  = scene_fade_out_on_enter,
    scene_fade_out_on_update = scene_fade_out_on_update,
    scene_fade_out_on_exit   = scene_fade_out_on_exit,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_info("Spawning birthday intro scene entities...")

    -- Set render resolution for Birthday Card
    engine.set_render_size(618, 864)

    local camera_offset_x = 618.0 / 2.0
    local camera_offset_y = 864.0 / 2.0
    engine.set_camera(0.0, 0.0, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- Text: "Quince destellos..."
    engine.spawn()
        :with_position(-250, -30)
        :with_zindex(2.0)
        :with_text("Quince destellos...", "birthday-love", 15 * 4, 255, 255, 255, 255)
        :register_as("line1")
        :build()

    engine.spawn()
        :with_position(-250 + 2, -30 + 2)
        :with_text("Quince destellos...", "birthday-love", 15 * 4, 0, 0, 0, 128)
        :with_zindex(1.0)
        :with_signals()
        :register_as("shadow1")
        :build()

    -- White background
    engine.spawn()
        :with_position(0, 0)
        :with_sprite("birthday-white", 256, 256, 128, 128)
        :with_zindex(-10.0)
        :with_scale(4, 4)
        :with_tint(255, 255, 255, 220)
        :register_as("white_background")
        :build()

    -- Big heart beating in center
    engine.spawn()
        :with_group("heart")
        :with_position(0, 0)
        :with_zindex(2)
        :with_sprite("birthday-big_heart-sheet", 400, 400, 200, 200)
        :with_animation("birthday-heart_beat_big")
        :with_collider(400, 400, 200, 200)
        :with_scale(1.0, 1.0)
        :register_as("big_heart")
        :build()

    -- Play birthday music
    engine.play_music("birthday-birthday_music", true)

    spawn_heart_particles_templates()

    -- Hearts particle emitter
    engine.spawn()
        :with_position(0, 0)
        :with_particle_emitter({
            templates = { "heart_spin01", "heart_spin02", "heart_spin03", "heart_spin04", "heart_spin05", "heart_spin06",
                "heart_spin07",
                "heart_beat01", "heart_beat02", "heart_beat03", "heart_beat04", "heart_beat05", "heart_beat06" },
            shape = { type = "rect", width = 618.0, height = 864.0 },
            particles_per_emission = 1,
            emissions_per_second = 3,
            emissions_remaining = 4294967295,
            arc = { 0, 360 },
            speed = { 0, 60 },
            ttl = { min = 3.0, max = 15.0 },
        })
        :build()

    engine.post_process_shader({ "fade" })
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    -- Scene phase controller
    engine.spawn()
        :with_signals()
        :with_group("intro_controller")
        :with_phase({
            initial = "fade_in",
            phases = {
                fade_in = {
                    on_enter = "scene_fade_in_on_enter",
                    on_update = "scene_fade_in_on_update"
                },
                hold = {
                    on_enter = "scene_hold_on_enter",
                    on_update = "scene_hold_on_update",
                    on_exit = "scene_hold_on_exit"
                },
                fade_out = {
                    on_enter = "scene_fade_out_on_enter",
                    on_update = "scene_fade_out_on_update",
                    on_exit = "scene_fade_out_on_exit"
                }
            }
        })
        :register_as("intro_controller")
        :build()

    engine.log_info("Birthday intro scene entities queued!")
end

return M
