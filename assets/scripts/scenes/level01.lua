-- scenes/level01.lua
-- Level 01 scene - entity spawning and phase callbacks
-- Called when switching to the "level01" scene

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
            :build()
    end
    engine.log_info("Asteroids spawned!")
end

local function spawn_medium_asteroids(x, y)
    -- Spawn medium asteroids (not implemented yet)
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
                        on_enter = "asteroid_phase_drifting_enter",  -- Setup vars in signals
                        on_update = "asteroid_phase_drifting_update" -- Handle movement
                    },
                    exploding = {
                        on_enter = "asteroid_phase_exploding_enter" -- Phase changed on collision
                    }
                }
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
                        on_enter = "asteroid_phase_drifting_enter",  -- Setup vars in signals
                        on_update = "asteroid_phase_drifting_update" -- Handle movement
                    },
                    exploding = {
                        on_enter = "asteroid_phase_exploding_enter" -- Phase changed on collision
                    }
                }
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

function on_asteroid_ship_collision(ctx)
    engine.log_info("Collision: Ship (ID " .. tostring(ctx.a.id) .. ") with Asteroid (ID " .. tostring(ctx.b.id) .. ")")
    -- For now, just log the collision. In a real game, we might reduce ship health, destroy asteroid, etc.
end

function on_asteroid_laser_collision(ctx)
    engine.log_info("Collision: Laser (ID " .. tostring(ctx.a.id) .. ") with Asteroid (ID " .. tostring(ctx.b.id) .. ")")
    -- entities are ordered by group name, so ctx.a is always asteroid, ctx.b is always laser
    -- print context of entity a in debug
    -- engine.log_info("Asteroid context:\n" .. dump_value(ctx.a, 6))
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
    -- Destroy laser entity
    engine.entity_despawn(ctx.b.id)
end

-- ==================== PHASE CALLBACK FUNCTIONS ====================
-- These are named functions called directly by the engine based on
-- the phase definitions in :with_phase()
-- Phase callbacks now receive EntityContext (ctx) instead of entity_id

-- =================== ASTEROID PHASE CALLBACKS AND HELPERS ====================

function asteroid_phase_drifting_enter(ctx, input)
    engine.log_info("Asteroid drifting enter: ID " .. tostring(ctx.id))
    -- Set random rotation speed
    local rotation_speed = math.random(-30.0, 30.0) -- degrees per second
    engine.entity_signal_set_scalar(ctx.id, "rotation_speed", rotation_speed)
end

function asteroid_phase_drifting_update(ctx, input, dt)
    -- Rotate asteroid based on rotation speed
    local rotation_speed = ctx.signals.scalars.rotation_speed or 0.0
    local new_rotation = (ctx.rotation or 0.0) + rotation_speed * dt
    engine.entity_set_rotation(ctx.id, new_rotation)
end

function asteroid_phase_exploding_enter(ctx, input)
    engine.log_info("Asteroid exploding enter: ID " .. tostring(ctx.id))
    -- log debug ctx
    -- engine.log_info("Asteroid context:\n" .. Dump_value(ctx))
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

    -- Despawn asteroid entity
    engine.entity_despawn(ctx.id)
end

-- ==================== SHIP PHASE CALLBACKS AND HELPERS ====================

-- Current ship rotation (degrees)
local SHIP_ROTATION = 0.0

--- Helper function to rotate the ship based on input
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
local function rotate_ship(ctx, input, dt)
    local rotation_speed = 360.0   -- degrees per second
    local propulsion_accel = 300.0 -- units per second squared
    -- engine.log_info("Current rotation: " .. tostring(CURRENT_ROTATION))
    -- Note: ctx.rotation contains current rotation if available
    SHIP_ROTATION = ctx.rotation or 0.0

    -- calculate acceleration vector based on current rotation
    local radians = math.rad(SHIP_ROTATION)
    local accel_x = math.sin(radians) * propulsion_accel
    local accel_y = -math.cos(radians) * propulsion_accel
    engine.entity_set_force_value(ctx.id, "propulsion", accel_x, accel_y)

    if input.digital.left.pressed then
        -- engine.log_info("Rotating left")
        SHIP_ROTATION = SHIP_ROTATION - rotation_speed * dt
    end
    if input.digital.right.pressed then
        -- engine.log_info("Rotating right")
        SHIP_ROTATION = SHIP_ROTATION + rotation_speed * dt
    end

    engine.entity_set_rotation(ctx.id, SHIP_ROTATION)
end

local function fire_laser(ctx)
    -- Fire laser
    -- First, we calculate the spawn position at the ship's nose
    local radians = math.rad(SHIP_ROTATION)
    local nose_offset = 32.0 -- distance from center to nose
    local spawn_x = ctx.pos.x + math.sin(radians) * nose_offset
    local spawn_y = ctx.pos.y - math.cos(radians) * nose_offset
    -- Clone the laser template
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
    -- Play blaster sound
    engine.play_sound("blaster")
end

--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function ship_phase_idle_enter(ctx, input)
    engine.log_info("Ship entered idle phase. Previous phase: " .. tostring(ctx.previous_phase))
    -- set animation to idle
    engine.entity_set_animation(ctx.id, "ship_idle")
    engine.entity_set_force_enabled(ctx.id, "propulsion", false)
end

--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function ship_phase_idle_update(ctx, input, dt)
    -- engine.log_info("Ship idle phase update")
    -- set camera ship's position
    engine.set_camera(ctx.pos.x, ctx.pos.y, 640 / 2, 360 / 2, 0.0, 1.0)

    -- Handle rotation input
    rotate_ship(ctx, input, dt)

    -- If "up" is just pressed, switch to propulsion phase
    if input.digital.up.just_pressed then
        -- engine.phase_transition(ctx.id, "propulsion")
        return "propulsion"
    end

    if input.digital.action_1.just_pressed then
        -- Fire laser
        fire_laser(ctx)
    end
end

--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function ship_phase_propulsion_enter(ctx, input)
    engine.log_info("Ship entered propulsion phase. Previous phase: " .. tostring(ctx.previous_phase))
    -- set animation to propulsion
    engine.entity_set_animation(ctx.id, "ship_propulsion")
    engine.entity_set_force_enabled(ctx.id, "propulsion", true)
end

--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function ship_phase_propulsion_update(ctx, input, dt)
    -- set camera ship's position
    engine.set_camera(ctx.pos.x, ctx.pos.y, 640 / 2, 360 / 2, 0.0, 1.0)

    -- Handle rotation input
    rotate_ship(ctx, input, dt)

    if input.digital.action_1.just_pressed then
        -- Fire laser
        fire_laser(ctx)
    end

    -- When "up" is released, go back to idle
    if input.digital.up.just_released then
        -- engine.phase_transition(ctx.id, "idle")
        return "idle"
    end
end

-- ==================== SCENE GAME STATE CALLBACKS ====================

function scene_init_enter(ctx, input)
    engine.log_info("scene_init_enter: Entering init phase")
    return "get_started"
end

--[[
--- Called when entering "init" phase (not used, we just transition immediately)
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_init_update(ctx, input, dt)
    -- Immediately transition to get_started
    -- engine.phase_transition(ctx.id, "get_started")
    engine.log_info("scene_init_update: Init phase complete - transitioning to get_started")
    return "get_started"
end

function scene_init_exit(ctx, input)
    engine.log_info("scene_init_exit: Exiting init phase")
end
 ]]

--- Called when entering "get_started" phase
--- - Play "player_ready" music
--- - Spawn the ball (stuck to player paddle)
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function scene_get_started_enter(ctx, input)
    engine.log_info("Entering get_started phase")

    --[[     -- Play "player_ready" music (no loop)
    engine.play_music("player_ready", false)

    -- Get player entity ID for StuckTo
    local player_id = engine.get_entity("player")
    if not player_id then
        engine.log_error("Player entity not found in world signals! Cannot spawn ball.")
        return
    end
    engine.log_info("Found player for ball spawn: " .. tostring(player_id))

    -- set player to sticky phase
    engine.phase_transition(player_id, "sticky")

    -- Get player Y position from world signals
    local player_y = engine.get_scalar("player_y") or 700.0
    local ball_y = player_y - 24.0 - 6.0 -- Above the player paddle

    -- Spawn ball directly with StuckTo attached via spawn command
    -- This avoids timing issues with engine.get_entity() in ball_stuck_enter
    engine.spawn()
        :with_group("ball")
        :with_position(336, ball_y) -- Initial position (will be updated by StuckTo)
        :with_sprite("ball", 12, 12, 6, 6)
        :with_zindex(10)
        :with_collider(12, 12, 6, 6)
        :with_signals()
        :with_stuckto(player_id, true, false)    -- Follow player X only
        :with_stuckto_offset(0, 0)               -- Centered on player X
        :with_stuckto_stored_velocity(300, -300) -- Velocity when released
        :with_phase({
            initial = "stuck_to_player",
            phases = {
                stuck_to_player = {
                    -- No enter callback needed, StuckTo is already attached
                    on_update = "ball_stuck_update"
                },
                moving = {
                    on_enter = "ball_moving_enter"
                }
            }
        })
        :build()

    engine.log_info("Ball spawned with StuckTo!") ]]
end

--- Called each frame in "get_started" phase
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_get_started_update(ctx, input, dt)
    -- Transition to playing after setup
    -- engine.phase_transition(ctx.id, "playing")
end

--- Called each frame in "playing" phase
--- - Check for level cleared (no bricks)
--- - Check for ball lost (no balls)
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_playing_update(ctx, input, dt)
    --[[     -- Skip first few frames to let group counts update
    if ctx.time_in_phase < 0.1 then
        return
    end

    -- Check for level cleared (no bricks remaining)
    local brick_count = engine.get_group_count("brick")
    if brick_count ~= nil and brick_count == 0 then
        engine.log_info("All bricks destroyed - level cleared!")
        engine.phase_transition(ctx.id, "level_cleared")
        return
    end

    -- Check for ball lost (no balls remaining)
    local ball_count = engine.get_group_count("ball")
    if ball_count ~= nil and ball_count == 0 then
        engine.log_info("No balls remaining - lose life!")
        engine.phase_transition(ctx.id, "lose_life")
        return
    end ]]
end

--- Called each frame in "lose_life" phase
--- - Decrement lives
--- - Transition to game_over or get_started
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_lose_life_update(ctx, input, dt)
    --[[     local lives = engine.get_integer("lives") or 0
    lives = lives - 1
    engine.set_integer("lives", lives)
    engine.log_info(string.format("Lost a life! Remaining lives: %d", lives))

    if lives < 1 then
        engine.phase_transition(ctx.id, "game_over")
    else
        engine.phase_transition(ctx.id, "get_started")
    end ]]
end

--- Called when entering "game_over" phase
--- - Spawn "GAME OVER" text
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function scene_game_over_enter(ctx, input)
    --[[     engine.log_info("Game Over!")

    -- Spawn game over text
    engine.spawn()
        :with_group("game_over_text")
        :with_screen_position(200, 350)
        :with_text("GAME OVER", "future", 48, 255, 0, 0, 255) -- Red
        :with_zindex(100)
        :build() ]]
end

--- Called each frame in "game_over" phase
--- - After 3 seconds, switch to menu scene
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_game_over_update(ctx, input, dt)
    --[[     if ctx.time_in_phase >= 3.0 then
        engine.log_info("Game over - returning to menu")
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")

        -- show bounces and hits in log
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
    end ]]
end

--- Called when entering "level_cleared" phase
--- - Play success music
--- - Spawn "LEVEL CLEARED" text
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function scene_level_cleared_enter(ctx, input)
    --[[     engine.log_info("Level Cleared!")

    -- Play success music
    engine.play_music("success", false)

    -- Spawn level cleared text
    engine.spawn()
        :with_group("level_cleared_text")
        :with_screen_position(150, 350)
        :with_text("LEVEL CLEARED", "future", 48, 0, 255, 0, 255) -- Green
        :with_zindex(100)
        :build() ]]
end

--- Called each frame in "level_cleared" phase
--- - After 4 seconds, switch to menu scene
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_level_cleared_update(ctx, input, dt)
    --[[     if ctx.time_in_phase >= 4.0 then
        engine.log_info("Level cleared - returning to menu")
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")

        -- show bounces and hits in log
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
    end ]]
end

-- ==================== ENTITY SPAWNING ====================

-- Level constants (matching tilemap info)
--[[ local TILE_SIZE = 24
local MAP_WIDTH = 28
local MAP_HEIGHT = 32
 ]]
--- Spawn the invisible wall colliders for the level
--[[ local function spawn_walls()
    -- Left wall - position at left bottom, origin at bottom
    engine.spawn()
        :with_group("walls")
        :with_position(0, TILE_SIZE * MAP_HEIGHT) -- Left bottom
        :with_collider(
            TILE_SIZE * 1,                        -- width: 1 tile
            TILE_SIZE * (MAP_HEIGHT - 2),         -- height: map height minus 2 tiles
            0,                                    -- origin_x: 0
            TILE_SIZE * (MAP_HEIGHT - 2)          -- origin_y: at top of wall
        )
        :build()

    -- Right wall - position at right bottom, origin at bottom-right
    engine.spawn()
        :with_group("walls")
        :with_position(TILE_SIZE * MAP_WIDTH, TILE_SIZE * MAP_HEIGHT) -- Right bottom
        :with_collider(
            TILE_SIZE * 1,                                            -- width: 1 tile
            TILE_SIZE * (MAP_HEIGHT - 2),                             -- height: map height minus 2 tiles
            TILE_SIZE * 1,                                            -- origin_x: right edge
            TILE_SIZE * (MAP_HEIGHT - 2)                              -- origin_y: at top of wall
        )
        :build()

    -- Top wall - position at center top, origin at center
    engine.spawn()
        :with_group("walls")
        :with_position(TILE_SIZE * MAP_WIDTH * 0.5, TILE_SIZE * 2) -- Center top
        :with_collider(
            TILE_SIZE * (MAP_WIDTH - 2),                           -- width: map width minus 2 tiles
            TILE_SIZE * 1,                                         -- height: 1 tile
            TILE_SIZE * (MAP_WIDTH - 2) * 0.5,                     -- origin_x: center
            0                                                      -- origin_y: 0
        )
        :build()

    -- Out of bounds (bottom) wall - catches balls that fall below the play area
    engine.spawn()
        :with_group("oob_wall")
        :with_position(-(TILE_SIZE * 5), TILE_SIZE * MAP_HEIGHT) -- Left of map, at bottom
        :with_collider(
            TILE_SIZE * (MAP_WIDTH + 10),                        -- width: extra wide
            TILE_SIZE * 10,                                      -- height: 10 tiles
            0,                                                   -- origin_x: 0
            0                                                    -- origin_y: 0
        )
        :build()

    engine.log_info("Walls spawned!")
end ]]

--- Spawn the ship
local function spawn_ship()
    -- Animation is controlled directly from Lua phase callbacks (no animation controller)
    engine.spawn()
        :with_group("ship")
        :with_position(0, 0)
        :with_zindex(2)
        :with_sprite("ship_sheet", 64, 64, 32, 32)
        :with_animation("ship_idle")
        :with_collider(64, 64, 32, 32) -- Same size collider
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
    -- :with_shader("outline",
    --     {
    --         uThickness = 2.5,
    --         uColor = { 1.0, 0.0, 0.0, 1.0 } -- Red outline
    --     })
        :register_as("ship") -- Store entity ID
        :build()

    engine.log_info("Ship spawned!")
end

--- Spawn the background
local function spawn_background()
    -- We have 4 textures: space01, space02, space03, space04
    -- The textures are 512x512. We are going to create a 8x8 grid of them
    -- Each tile will have a random texture from the 4 options
    -- Each tile will have a random rotation (0, 90, 180, 270 degrees)
    -- The first tile will be at (0,0)
    local tile_size = 512
    local grid_size = 8
    for row = 0, grid_size - 1 do
        for col = 0, grid_size - 1 do
            -- Randomly select a texture
            local texture_index = math.random(1, 4)
            local texture_name = "space0" .. tostring(texture_index)
            -- Randomly select a rotation
            local rotation_options = { 0, 90, 180, 270 }
            local rotation_index = math.random(1, #rotation_options)
            local rotation = rotation_options[rotation_index]
            -- Spawn the background tile
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
    -- Spawn a template entity for the laser projectile
    engine.spawn()
        :with_sprite("asteroids-laser", 48, 83, 24, 21)
    -- :with_position(0, 0) -- Offscreen
        :with_collider(6, 6, 3, 3)
        :with_zindex(1)
    -- :with_velocity(0, -500)        -- Moves upward
        :with_signals()
        :register_as("laser_template") -- Store entity ID for cloning
        :build()

    engine.log_info("Laser template spawned!")
end

local function spawn_template_explosions()
    -- Spawn a template entity for the explosion animation
    engine.spawn()
        :with_sprite("explosion01_sheet", 64, 64, 32, 32)
        :with_animation("explosion01")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion01_animation") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_sprite("explosion02_sheet", 32, 32, 16, 16)
        :with_animation("explosion02")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion02_animation") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_sprite("explosion03_sheet", 16, 16, 8, 8)
        :with_animation("explosion03")
        :with_zindex(10)
        :with_signals()
        :register_as("explosion03_animation") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_particle_emitter({
            -- templates = { "explosion02_animation", "explosion03_animation" },
            templates = { "explosion01_animation" },
            shape = { type = "rect", width = 60, height = 60 },
            particles_per_emission = 10,
            emissions_per_second = 100,
            emissions_remaining = 25,
            arc = { 0, 360 },
            speed = { 0, 200 },
            ttl = 0.8,
        })
        :register_as("explosion_big_emitter") -- Store entity ID for cloning
        :build()

    engine.spawn()
        :with_particle_emitter({
            -- templates = { "explosion02_animation", "explosion03_animation" },
            templates = { "explosion02_animation" },
            shape = { type = "rect", width = 30, height = 30 },
            particles_per_emission = 5,
            emissions_per_second = 50,
            emissions_remaining = 25,
            arc = { 0, 360 },
            speed = { 0, 100 },
            ttl = 0.4,
        })
        :register_as("explosion_medium_emitter") -- Store entity ID for cloning
        :build()
    engine.spawn()
        :with_particle_emitter({
            -- templates = { "explosion02_animation", "explosion03_animation" },
            templates = { "explosion03_animation" },
            shape = { type = "rect", width = 15, height = 15 },
            particles_per_emission = 5,
            emissions_per_second = 50,
            emissions_remaining = 15,
            arc = { 0, 360 },
            speed = { 0, 50 },
            ttl = 0.4,
        })
        :register_as("explosion_small_emitter") -- Store entity ID for cloning
        :build()

    engine.log_info("Explosion templates spawned!")
end

local function spawn_collision_rules()
    -- Define collision rules by spawning entities with CollisionRule component
    -- Ship collides with asteroids
    engine.spawn()
        :with_lua_collision_rule("asteroids", "ship", "on_asteroid_ship_collision")
        :with_group("collision_rules")
        :build()
    -- Lasers collide with asteroids
    engine.spawn()
        :with_lua_collision_rule("asteroids", "lasers", "on_asteroid_laser_collision")
        :with_group("collision_rules")
        :build()
    engine.log_info("Collision rules spawned!")
end

--- Spawn the UI score texts
--[[ local function spawn_ui_texts()
    -- Score header text "1UP   HIGH SCORE"
    engine.spawn()
        :with_group("ui")
        :with_position(TILE_SIZE * 3, 0)
        :with_text("1UP   HIGH SCORE", "arcade", TILE_SIZE, 255, 0, 0, 255) -- Red
        :with_zindex(20)
        :build()

    -- Player score (bound to "score" signal)
    engine.spawn()
        :with_group("player_score")
        :with_position(TILE_SIZE * 3, TILE_SIZE)
        :with_text("0", "arcade", TILE_SIZE, 255, 255, 255, 255) -- White
        :with_zindex(20)
        :with_signal_binding("score")
        :build()

    -- High score (bound to "high_score" signal)
    engine.spawn()
        :with_group("high_score")
        :with_position(TILE_SIZE * 10, TILE_SIZE)
        :with_text("0", "arcade", TILE_SIZE, 255, 255, 255, 255) -- White
        :with_zindex(20)
        :with_signal_binding("high_score")
        :build()

    engine.log_info("UI texts spawned!")
end ]]

--- Spawn the bricks grid layout
--[[ local function spawn_bricks()
    -- Spawn an entity with GridLayout component
    -- The gridlayout_spawn_system will process the JSON and spawn brick entities
    engine.spawn()
        :with_grid_layout("./assets/levels/level01.json", "brick", 5)
        :build()

    engine.log_info("Bricks grid layout spawned!")

    -- TODO: Create a system to load json files and spawn entities from lua directly
end ]]

--- Spawn all entities for level 01.
--- This is called when entering the scene (before phase system starts)
function M.spawn()
    engine.log_info("Spawning level01 scene entities from Lua...")

    engine.log_info("Setting camera.")
    -- todo: get screen size from engine instead of hardcoding for calculating offset
    engine.set_camera(0, 0, 640 / 2, 360 / 2, 0.0, 1.0)

    spawn_ship()

    spawn_background()

    spawn_big_asteroids()

    spawn_template_laser()

    spawn_template_explosions()

    -- Spawn collision rule entities
    spawn_collision_rules()

    -- Spawn the scene phase entity with LuaPhase component
    -- Each phase specifies its callback function names
    engine.spawn()
        :with_group("scene_phases")
        :with_phase({
            initial = "init",
            phases = {
                init = {
                    on_enter = "scene_init_enter"
                    -- on_update = "scene_init_update",
                    -- on_exit = "scene_init_exit"
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

    engine.post_process_shader({ "bloom", "crt" })
    engine.post_process_set_float("threshold", 0.7)
    engine.post_process_set_float("intensity", 1.8)
    engine.post_process_set_float("radius", 2.0)

    engine.post_process_set_float("uResDivisor", 1.5) -- sharper, 360 ==> 240p
    engine.post_process_set_int("uMaskStyle", 1)      -- apperture grille

    --[[ engine.post_process_set_float("uCurvature", 0.1)
    engine.post_process_set_float("uScanline", 0.5)
    engine.post_process_set_float("uVignette", 0.3)
    engine.post_process_set_float("uFlicker", 0.25) ]]

    engine.log_info("Scene phase entity spawned with LuaPhase")
    engine.log_info("Level01 scene entities queued!")
end

--- Called each frame when level01 scene is active.
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function on_update_level01(input, dt)
    -- Check for back button to return to menu
    if input.digital.back.just_pressed then
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")
    end

    -- Note: Most game logic is handled
    -- by the phase system callbacks above. This is just for input handling.
end

return M
