-- scenes/asteroids/level01.lua
-- Asteroids example — entity spawning and phase callbacks
-- Scene name: "asteroids_level01"

local M = {}

local function spawn_big_asteroids()
    -- Spawn a few asteroids at random positions
    local asteroid_textures = { "asteroids-big01", "asteroids-big02", "asteroids-big03" }
    for i = 1, 10 do
        local texture_index = math.random(1, #asteroid_textures)
        local texture_name = asteroid_textures[texture_index]
        local pos_x = math.random(0, 2048)
        local pos_y = math.random(0, 2048)
        engine.spawn()
            :with_group("asteroids")
            :with_position(pos_x, pos_y)
            :with_sprite(texture_name, 128, 128, 64, 64)
            :with_rotation(math.random(0, 360))
            :with_velocity(math.random(-10, 10), math.random(-10, 10))
            :with_zindex(5)
            :with_collider(80, 80, 40, 40)
        --:with_signals()
            :with_signal_integer("hp", 3)
            :with_signal_integer("asteroid_type", 3) -- Big asteroid
            :with_phase({
                initial = "drifting",
                phases = {
                    drifting = {
                        on_enter = "asteroid_phase_drifting_enter",  -- Setup vars in signals
                        on_update = "asteroid_phase_drifting_update" -- Handle movement
                    },
                    exploding = {
                        on_enter = "asteroid_phase_exploding_enter" -- Phase changed on collision
                    }
                }
            })
            :with_shader("blink", {
                colBlink   = { 1.0, 0.0, 0.0, 1.0 },
                uCycleTime = 0.6,
                uBlinkPct  = 0.0,
            })
            :build()
    end
    engine.log_info("Asteroids spawned!")
end

local function spawn_medium_asteroids(x, y)
    local asteroid_textures = { "asteroids-medium01", "asteroids-medium02", "asteroids-medium03" }
    for i = 1, 3 do
        local texture_index = math.random(1, #asteroid_textures)
        local texture_name = asteroid_textures[texture_index]
        engine.spawn()
            :with_group("asteroids")
            :with_position(x + math.random(-20, 20), y + math.random(-20, 20))
            :with_sprite(texture_name, 64, 64, 32, 32)
            :with_rotation(math.random(0, 360))
            :with_velocity(math.random(-30, 30), math.random(-30, 30))
            :with_zindex(5)
            :with_collider(40, 40, 20, 20)
        --:with_signals()
            :with_signal_integer("hp", 2)
            :with_signal_integer("asteroid_type", 2) -- Medium asteroid
            :with_phase({
                initial = "drifting",
                phases = {
                    drifting = {
                        on_enter = "asteroid_phase_drifting_enter",
                        on_update = "asteroid_phase_drifting_update"
                    },
                    exploding = {
                        on_enter = "asteroid_phase_exploding_enter"
                    }
                }
            })
            :with_shader("blink", {
                colBlink   = { 1.0, 0.0, 0.0, 1.0 },
                uCycleTime = 0.6,
                uBlinkPct  = 0.0,
            })
            :build()
    end
end

local function spawn_small_asteroids(x, y)
    local asteroid_textures = { "asteroids-small01", "asteroids-small02", "asteroids-small03" }
    for i = 1, 6 do
        local texture_index = math.random(1, #asteroid_textures)
        local texture_name = asteroid_textures[texture_index]
        engine.spawn()
            :with_group("asteroids")
            :with_position(x + math.random(-20, 20), y + math.random(-20, 20))
            :with_sprite(texture_name, 32, 32, 16, 16)
            :with_rotation(math.random(0, 360))
            :with_velocity(math.random(-60, 60), math.random(-60, 60))
            :with_zindex(5)
            :with_collider(40, 40, 20, 20)
        --:with_signals()
            :with_signal_integer("hp", 1)
            :with_signal_integer("asteroid_type", 1) -- Small asteroid
            :with_phase({
                initial = "drifting",
                phases = {
                    drifting = {
                        on_enter = "asteroid_phase_drifting_enter",
                        on_update = "asteroid_phase_drifting_update"
                    },
                    exploding = {
                        on_enter = "asteroid_phase_exploding_enter"
                    }
                }
            })
            :with_shader("blink", {
                colBlink   = { 1.0, 0.0, 0.0, 1.0 },
                uCycleTime = 0.6,
                uBlinkPct  = 0.0,
            })
            :build()
    end
end

-- ==================== COLLISION CALLBACK FUNCTIONS ====================
-- These are called when collisions occur between entities with matching groups.
-- ctx = {
--   a = { id, group, pos={x,y}, vel={x,y}, rect={x,y,w,h}, signals={flags={...}, integers={...}} },
--   b = { ...same... },
--   sides = { a = {"left","top",...}, b = {...} }
-- }

local function on_asteroid_ship_collision(ctx)
    engine.log_info("Collision: Ship (ID " .. tostring(ctx.a.id) .. ") with Asteroid (ID " .. tostring(ctx.b.id) .. ")")
    -- For now, just log the collision. In a real game, we might reduce ship health, destroy asteroid, etc.
end

local function on_asteroid_laser_collision(ctx)
    engine.log_info("Collision: Laser (ID " .. tostring(ctx.a.id) .. ") with Asteroid (ID " .. tostring(ctx.b.id) .. ")")
    -- entities are ordered by group name, so ctx.a is always asteroid, ctx.b is always laser
    -- Reduce asteroid HP
    local hp = ctx.a.signals.integers.hp or 0
    hp = hp - 1
    engine.entity_signal_set_integer(ctx.a.id, "hp", hp)
    engine.log_info("Asteroid HP reduced to " .. tostring(hp))
    if hp <= 0 then
        engine.log_info("Asteroid destroyed!")
        -- Change asteroid phase to exploding
        engine.phase_transition(ctx.a.id, "exploding")
    end
    -- change blink timing depending on remaining hp
    if hp == 2 then
        engine.entity_shader_set_float(ctx.a.id, "uCycleTime", 1.0)
        engine.entity_shader_set_float(ctx.a.id, "uBlinkPct", 0.1)
    elseif hp == 1 then
        engine.entity_shader_set_float(ctx.a.id, "uCycleTime", 0.5)
        engine.entity_shader_set_float(ctx.a.id, "uBlinkPct", 0.1)
    end
    -- Spawn a small explosion at the collision point, facing 180 degrees (from laser direction)
    local laser_pos = ctx.b.pos
    local laser_rotation = ctx.b.rotation or 0.0
    engine.clone("explosion03_animation")
        :with_position(laser_pos.x, laser_pos.y)
        :with_rotation(laser_rotation + 180.0)
        :with_ttl(0.4)
        :build()
    -- Destroy laser entity
    engine.entity_despawn(ctx.b.id)
end

-- ==================== PHASE CALLBACK FUNCTIONS ====================
-- These are named functions called directly by the engine based on
-- the phase definitions in :with_phase()
-- Phase callbacks now receive EntityContext (ctx) instead of entity_id

-- =================== ASTEROID PHASE CALLBACKS AND HELPERS ====================

local function asteroid_phase_drifting_enter(ctx, input)
    engine.log_info("Asteroid drifting enter: ID " .. tostring(ctx.id))
    -- Set random rotation speed
    local rotation_speed = math.random(-30.0, 30.0) -- degrees per second
    engine.entity_signal_set_scalar(ctx.id, "rotation_speed", rotation_speed)
end

local function asteroid_phase_drifting_update(ctx, input, dt)
    -- Rotate asteroid based on rotation speed
    local rotation_speed = ctx.signals.scalars.rotation_speed or 0.0
    local new_rotation = (ctx.rotation or 0.0) + rotation_speed * dt
    engine.entity_set_rotation(ctx.id, new_rotation)
end

local function asteroid_phase_exploding_enter(ctx, input)
    engine.log_info("Asteroid exploding enter: ID " .. tostring(ctx.id))
    -- Get type of asteroid by signal "asteroid_type" (big = 3, medium = 2, small = 1)
    local asteroid_type = ctx.signals.integers.asteroid_type or 3
    if asteroid_type == 3 then
        -- Big asteroid - spawn 2 medium asteroids
        spawn_medium_asteroids(ctx.pos.x, ctx.pos.y)
        engine.clone("explosion_big_emitter")
            :with_position(ctx.pos.x, ctx.pos.y)
            :with_ttl(3.0)
            :build()
    end
    if asteroid_type == 2 then
        -- Medium asteroid - spawn 2 small asteroids
        spawn_small_asteroids(ctx.pos.x, ctx.pos.y)
        engine.clone("explosion_medium_emitter")
            :with_position(ctx.pos.x, ctx.pos.y)
            :with_ttl(3.0)
            :build()
    end

    if asteroid_type == 1 then
        -- Small asteroid - just explode
        engine.clone("explosion_small_emitter")
            :with_position(ctx.pos.x, ctx.pos.y)
            :with_ttl(3.0)
            :build()
    end

    engine.play_sound("asteroids-explosion01")

    -- Despawn asteroid entity
    engine.entity_despawn(ctx.id)
end

-- ==================== SHIP PHASE CALLBACKS AND HELPERS ====================

-- Current ship rotation (degrees)
local SHIP_ROTATION = 0.0

--- Helper function to rotate the ship based on input
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function rotate_ship(ctx, input, dt)
    local rotation_speed = 360.0   -- degrees per second
    local propulsion_accel = 300.0 -- units per second squared
    SHIP_ROTATION = ctx.rotation or 0.0

    -- calculate acceleration vector based on current rotation
    local radians = math.rad(SHIP_ROTATION)
    local accel_x = math.sin(radians) * propulsion_accel
    local accel_y = -math.cos(radians) * propulsion_accel
    engine.entity_set_force_value(ctx.id, "propulsion", accel_x, accel_y)

    if input.digital.left.pressed then
        SHIP_ROTATION = SHIP_ROTATION - rotation_speed * dt
    end
    if input.digital.right.pressed then
        SHIP_ROTATION = SHIP_ROTATION + rotation_speed * dt
    end

    engine.entity_set_rotation(ctx.id, SHIP_ROTATION)
end

local function fire_laser(ctx)
    local radians = math.rad(SHIP_ROTATION)
    local nose_offset = 32.0 -- distance from center to nose
    local spawn_x = ctx.pos.x + math.sin(radians) * nose_offset
    local spawn_y = ctx.pos.y - math.cos(radians) * nose_offset
    engine.clone("laser_template")
        :with_group("lasers")
        :with_position(spawn_x, spawn_y)
        :with_rotation(SHIP_ROTATION)
        :with_velocity(
            math.sin(radians) * 500.0,
            -math.cos(radians) * 500.0
        )
        :with_ttl(1.5)
        :build()
    engine.play_sound_pitched("asteroids-blaster", 1.0 + (math.random() - 0.5) * 0.2)
end

--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function ship_phase_idle_enter(ctx, input)
    engine.log_info("Ship entered idle phase. Previous phase: " .. tostring(ctx.previous_phase))
    engine.entity_set_animation(ctx.id, "asteroids-ship_idle")
    engine.entity_set_force_enabled(ctx.id, "propulsion", false)
end

--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function ship_phase_idle_update(ctx, input, dt)
    engine.set_camera(ctx.pos.x, ctx.pos.y, 640 / 2, 360 / 2, 0.0, 1.0)
    rotate_ship(ctx, input, dt)

    if input.digital.up.just_pressed then
        return "propulsion"
    end

    if input.digital.action_1.just_pressed then
        fire_laser(ctx)
    end
end

--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function ship_phase_propulsion_enter(ctx, input)
    engine.log_info("Ship entered propulsion phase. Previous phase: " .. tostring(ctx.previous_phase))
    engine.entity_set_animation(ctx.id, "asteroids-ship_propulsion")
    engine.entity_set_force_enabled(ctx.id, "propulsion", true)
end

--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function ship_phase_propulsion_update(ctx, input, dt)
    engine.set_camera(ctx.pos.x, ctx.pos.y, 640 / 2, 360 / 2, 0.0, 1.0)
    rotate_ship(ctx, input, dt)

    if input.digital.action_1.just_pressed then
        fire_laser(ctx)
    end

    if input.digital.up.just_released then
        return "idle"
    end
end

-- ==================== SCENE GAME STATE CALLBACKS ====================

local function scene_init_enter(ctx, input)
    engine.log_info("scene_init_enter: Entering init phase")
    return "get_started"
end

--- Called when entering "get_started" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function scene_get_started_enter(ctx, input)
    engine.log_info("Entering get_started phase")
end

--- Called each frame in "get_started" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function scene_get_started_update(ctx, input, dt)
    -- Transition to playing after setup
    -- engine.phase_transition(ctx.id, "playing")
end

--- Called each frame in "playing" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function scene_playing_update(ctx, input, dt)
end

--- Called each frame in "lose_life" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function scene_lose_life_update(ctx, input, dt)
end

--- Called when entering "game_over" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function scene_game_over_enter(ctx, input)
end

--- Called each frame in "game_over" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function scene_game_over_update(ctx, input, dt)
end

--- Called when entering "level_cleared" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
local function scene_level_cleared_enter(ctx, input)
end

--- Called each frame in "level_cleared" phase
--- @param ctx EntityContext Entity context table
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function scene_level_cleared_update(ctx, input, dt)
end

-- ==================== SCENE UPDATE ====================

--- Called each frame when asteroids_level01 scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_asteroids_level01(input, dt)
    -- Check for back button to return to menu
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ───────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- The KEY must exactly match the string passed to the engine
-- (e.g. in :with_phase(), :with_lua_collision_rule(), :with_lua_timer()).

M._callbacks = {
    -- Scene update (engine calls on_update_<scene_name> automatically)
    on_update_asteroids_level01     = on_update_asteroids_level01,
    -- Collision callbacks
    on_asteroid_ship_collision      = on_asteroid_ship_collision,
    on_asteroid_laser_collision     = on_asteroid_laser_collision,
    -- Asteroid phase callbacks
    asteroid_phase_drifting_enter   = asteroid_phase_drifting_enter,
    asteroid_phase_drifting_update  = asteroid_phase_drifting_update,
    asteroid_phase_exploding_enter  = asteroid_phase_exploding_enter,
    -- Ship phase callbacks
    ship_phase_idle_enter           = ship_phase_idle_enter,
    ship_phase_idle_update          = ship_phase_idle_update,
    ship_phase_propulsion_enter     = ship_phase_propulsion_enter,
    ship_phase_propulsion_update    = ship_phase_propulsion_update,
    -- Scene state phase callbacks
    scene_init_enter                = scene_init_enter,
    scene_get_started_enter         = scene_get_started_enter,
    scene_get_started_update        = scene_get_started_update,
    scene_playing_update            = scene_playing_update,
    scene_lose_life_update          = scene_lose_life_update,
    scene_game_over_enter           = scene_game_over_enter,
    scene_game_over_update          = scene_game_over_update,
    scene_level_cleared_enter       = scene_level_cleared_enter,
    scene_level_cleared_update      = scene_level_cleared_update,
}

-- ==================== ENTITY SPAWNING ====================

--- Spawn the ship
local function spawn_ship()
    engine.spawn()
        :with_group("ship")
        :with_position(0, 0)
        :with_zindex(2)
        :with_sprite("asteroids-ship_sheet", 64, 64, 32, 32)
        :with_animation("asteroids-ship_idle")
        :with_collider(64, 64, 32, 32)
        :with_rotation(0.0)
        :with_velocity(0, 0)
        :with_friction(0.0)
        :with_accel("propulsion", 0.0, -300.0, false)
        :with_signals()
        :with_phase({
            initial = "idle",
            phases = {
                idle = {
                    on_enter = "ship_phase_idle_enter",
                    on_update = "ship_phase_idle_update"
                },
                propulsion = {
                    on_enter = "ship_phase_propulsion_enter",
                    on_update = "ship_phase_propulsion_update"
                }
            }
        })
        :register_as("ship")
        :build()

    engine.log_info("Ship spawned!")
end

--- Spawn the tiled space background
local function spawn_background()
    local tile_size = 512
    local grid_size = 8
    for row = 0, grid_size - 1 do
        for col = 0, grid_size - 1 do
            local texture_index = math.random(1, 4)
            local texture_name = "asteroids-space0" .. tostring(texture_index)
            local rotation_options = { 0, 90, 180, 270 }
            local rotation_index = math.random(1, #rotation_options)
            local rotation = rotation_options[rotation_index]
            engine.spawn()
                :with_group("background")
                :with_position(col * tile_size, row * tile_size)
                :with_sprite(texture_name, tile_size, tile_size, tile_size / 2, tile_size / 2)
                :with_rotation(rotation)
                :with_zindex(-10)
                :build()
        end
    end
end

local function spawn_template_laser()
    engine.spawn()
        :with_sprite("asteroids-laser", 48, 83, 24, 21)
        :with_collider(6, 6, 3, 3)
        :with_zindex(1)
        :with_signals()
        :register_as("laser_template")
        :build()

    engine.log_info("Laser template spawned!")
end

local function spawn_template_explosions()
    engine.spawn()
        :with_sprite("asteroids-explosion01_sheet", 64, 64, 32, 32)
        :with_animation("asteroids-explosion01")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion01_animation")
        :build()

    engine.spawn()
        :with_sprite("asteroids-explosion02_sheet", 32, 32, 16, 16)
        :with_animation("asteroids-explosion02")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion02_animation")
        :build()

    engine.spawn()
        :with_sprite("asteroids-explosion03_sheet", 16, 16, 8, 8)
        :with_animation("asteroids-explosion03")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion03_animation")
        :build()

    engine.spawn()
        :with_particle_emitter({
            templates = { "explosion01_animation" },
            shape = { type = "rect", width = 60, height = 60 },
            particles_per_emission = 10,
            emissions_per_second = 100,
            emissions_remaining = 25,
            arc = { 0, 360 },
            speed = { 0, 200 },
            ttl = 0.8,
        })
        :register_as("explosion_big_emitter")
        :build()

    engine.spawn()
        :with_particle_emitter({
            templates = { "explosion02_animation" },
            shape = { type = "rect", width = 30, height = 30 },
            particles_per_emission = 5,
            emissions_per_second = 50,
            emissions_remaining = 25,
            arc = { 0, 360 },
            speed = { 0, 100 },
            ttl = 0.4,
        })
        :register_as("explosion_medium_emitter")
        :build()

    engine.spawn()
        :with_particle_emitter({
            templates = { "explosion03_animation" },
            shape = { type = "rect", width = 15, height = 15 },
            particles_per_emission = 5,
            emissions_per_second = 50,
            emissions_remaining = 15,
            arc = { 0, 360 },
            speed = { 0, 50 },
            ttl = 0.4,
        })
        :register_as("explosion_small_emitter")
        :build()

    engine.log_info("Explosion templates spawned!")
end

local function spawn_collision_rules()
    engine.spawn()
        :with_lua_collision_rule("asteroids", "ship", "on_asteroid_ship_collision")
        :with_group("collision_rules")
        :build()
    engine.spawn()
        :with_lua_collision_rule("asteroids", "lasers", "on_asteroid_laser_collision")
        :with_group("collision_rules")
        :build()
    engine.log_info("Collision rules spawned!")
end

--- Spawn all entities for the Asteroids level.
function M.spawn()
    engine.log_info("Spawning Asteroids level01 scene entities...")

    -- Set render resolution for Asteroids
    engine.set_render_size(640, 360)

    engine.set_camera(0, 0, 640 / 2, 360 / 2, 0.0, 1.0)

    spawn_ship()
    spawn_background()
    spawn_big_asteroids()
    spawn_template_laser()
    spawn_template_explosions()
    spawn_collision_rules()

    -- Scene phase entity
    engine.spawn()
        :with_group("scene_phases")
        :with_phase({
            initial = "init",
            phases = {
                init = {
                    on_enter = "scene_init_enter"
                },
                get_started = {
                    on_enter = "scene_get_started_enter",
                    on_update = "scene_get_started_update"
                },
                playing = {
                    on_update = "scene_playing_update"
                },
                lose_life = {
                    on_update = "scene_lose_life_update"
                },
                game_over = {
                    on_enter = "scene_game_over_enter",
                    on_update = "scene_game_over_update"
                },
                level_cleared = {
                    on_enter = "scene_level_cleared_enter",
                    on_update = "scene_level_cleared_update"
                }
            }
        })
        :build()

    engine.post_process_shader({ "bloom" })
    engine.post_process_set_float("threshold", 0.7)
    engine.post_process_set_float("intensity", 1.8)
    engine.post_process_set_float("radius", 2.0)
    engine.post_process_set_float("uResDivisor", 1.5)
    engine.post_process_set_int("uMaskStyle", 1)

    engine.log_info("Asteroids level01 scene entities queued!")
end

return M
