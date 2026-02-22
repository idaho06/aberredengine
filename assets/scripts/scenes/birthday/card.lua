-- scenes/birthday/card.lua
-- Birthday Card example — card scene
-- Scene name: "birthday_card"
-- Internal render resolution: 618x864

local M = {}

local fade_strength = 1.0

local function spawn_gems_particles_templates()
    local gem_size = 16
    local sheet_width = 128
    local sheet_height = 128
    local cols = sheet_width / gem_size
    local rows = sheet_height / gem_size
    local template_names_list = {}
    for row = 0, rows - 1 do
        for col = 0, cols - 1 do
            local template_name = "gem_particle_" .. (row * cols + col + 1)
            engine.spawn()
                :with_sprite("birthday-gems-sheet", gem_size, gem_size, gem_size / 2, gem_size / 2)
                :with_sprite_offset(col * gem_size, row * gem_size)
                :with_zindex(0.0)
                :register_as(template_name)
                :build()
            table.insert(template_names_list, template_name)
        end
    end
    return template_names_list
end

-- ─── Phase callbacks (local) ─────────────────────────────────────────────────

local function card_fade_in_on_enter(ctx, input)
    fade_strength = 1.0
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)
end

local function card_fade_in_on_update(ctx, input, dt)
    fade_strength = math.max(fade_strength - dt / 1.5, 0.0)
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    if fade_strength <= 0.0 then
        return "hold"
    end
end

local function card_hold_on_enter(ctx, input)
    engine.log_info("Card scene hold phase entered.")
end

local function card_hold_on_update(ctx, input, dt)
    if ctx.time_in_phase >= 25.0 and not engine.has_flag("card_hold_text_shown") then
        engine.spawn()
            :with_position(-50, 400)
            :with_text("Press any key to exit...", "birthday-love", 12 * 2, 255, 32, 32, 255)
            :with_zindex(2)
            :register_as("hold_text")
            :build()
        engine.spawn()
            :with_position(-50 + 2, 400 + 2)
            :with_text("Press any key to exit...", "birthday-love", 12 * 2, 32, 32, 32, 255)
            :with_zindex(1.9)
            :register_as("hold_text_shadow")
            :build()

        engine.set_flag("card_hold_text_shown")
    end

    if (input.digital.action_1.just_pressed or input.digital.action_2.just_pressed
            or input.digital.up.just_pressed or input.digital.down.just_pressed or input.digital.left.just_pressed
            or input.digital.right.just_pressed) then
        return "fade_out"
    end
end

local function card_hold_on_exit(ctx)
    if engine.has_flag("card_hold_text_shown") then
        local hold_text_id = engine.get_entity("hold_text")
        local hold_text_shadow_id = engine.get_entity("hold_text_shadow")
        if hold_text_id and hold_text_shadow_id then
            engine.entity_despawn(hold_text_id)
            engine.entity_despawn(hold_text_shadow_id)
            engine.clear_flag("card_hold_text_shown")
        end
    end
end

local function card_fade_out_on_enter(ctx, input)
    fade_strength = 0.0
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)
end

local function card_fade_out_on_update(ctx, input, dt)
    fade_strength = math.min(fade_strength + dt / 1.5, 1.0)
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    if fade_strength >= 1.0 then
        return "fade_out_done"
    end
end

local function card_fade_out_on_exit(ctx)
    engine.change_scene("menu")
    engine.log_info("Card scene fade-out complete. Returning to menu.")
end

--- Called each frame when birthday_card scene is active.
local function on_update_birthday_card(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────

M._callbacks = {
    on_update_birthday_card  = on_update_birthday_card,
    card_fade_in_on_enter    = card_fade_in_on_enter,
    card_fade_in_on_update   = card_fade_in_on_update,
    card_hold_on_enter       = card_hold_on_enter,
    card_hold_on_update      = card_hold_on_update,
    card_hold_on_exit        = card_hold_on_exit,
    card_fade_out_on_enter   = card_fade_out_on_enter,
    card_fade_out_on_update  = card_fade_out_on_update,
    card_fade_out_on_exit    = card_fade_out_on_exit,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

function M.spawn()
    engine.log_info("Spawning birthday card scene entities...")

    -- Set render resolution for Birthday Card
    engine.set_render_size(618, 864)

    local camera_offset_x = 618.0 / 2.0
    local camera_offset_y = 864.0 / 2.0
    engine.set_camera(0.0, 0.0, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- White background
    engine.spawn()
        :with_position(0, 0)
        :with_sprite("birthday-white", 256, 256, 128, 128)
        :with_zindex(-10.0)
        :with_scale(4, 4)
        :with_tint(255, 255, 255, 220)
        :register_as("background")
        :build()

    -- Raquel photo
    engine.spawn()
        :with_position(0, 0)
        :with_sprite("birthday-raquel_back", 618, 874, 618 / 2, 874 / 2)
        :with_zindex(1.0)
        :build()

    -- Text lines
    local line1_pos_x = -230
    local line1_pos_y = -390
    local line1_size = 15 * 3

    engine.spawn()
        :with_position(line1_pos_x, line1_pos_y)
        :with_zindex(2.0)
        :with_text("Raquel pinta su mundo", "birthday-love", line1_size, 255, 255, 255, 255)
        :register_as("line1")
        :build()

    engine.spawn()
        :with_position(line1_pos_x + 2, line1_pos_y + 2)
        :with_text("Raquel pinta su mundo", "birthday-love", line1_size, 0, 0, 0, 128)
        :with_zindex(1.0)
        :with_signals()
        :register_as("shadow1")
        :build()

    local line2_pos_x = -230
    local line2_pos_y = line1_pos_y + 50
    local line2_size = 15 * 3

    engine.spawn()
        :with_position(line2_pos_x, line2_pos_y)
        :with_zindex(2.0)
        :with_text("Callada y libre.", "birthday-love", line2_size, 255, 255, 255, 255)
        :register_as("line2")
        :build()

    engine.spawn()
        :with_position(line2_pos_x + 2, line2_pos_y + 2)
        :with_text("Callada y libre.", "birthday-love", line2_size, 0, 0, 0, 128)
        :with_zindex(1.0)
        :with_signals()
        :register_as("shadow2")
        :build()

    -- Play Harry Styles music
    engine.play_music("birthday-harry", true)

    local particle_list = spawn_gems_particles_templates()

    -- Gems particle emitter
    engine.spawn()
        :with_position(40, -190)
        :with_particle_emitter({
            templates = particle_list,
            shape = "point",
            particles_per_emission = 5,
            emissions_per_second = 5,
            emissions_remaining = 4294967295,
            arc = { -65, 65 },
            speed = { 10, 60 },
            ttl = { min = 3.0, max = 15.0 },
        })
        :build()

    engine.post_process_shader({ "fade" })
    engine.post_process_set_vec4("fadeColor", 1.0, 1.0, 1.0, fade_strength)

    -- Scene phase controller
    engine.spawn()
        :with_signals()
        :with_group("card_controller")
        :with_phase({
            initial = "fade_in",
            phases = {
                fade_in = {
                    on_enter = "card_fade_in_on_enter",
                    on_update = "card_fade_in_on_update"
                },
                hold = {
                    on_enter = "card_hold_on_enter",
                    on_update = "card_hold_on_update",
                    on_exit = "card_hold_on_exit"
                },
                fade_out = {
                    on_enter = "card_fade_out_on_enter",
                    on_update = "card_fade_out_on_update",
                    on_exit = "card_fade_out_on_exit"
                }
            }
        })
        :register_as("card_controller")
        :build()

    engine.log_info("Birthday card scene entities queued!")
end

return M
