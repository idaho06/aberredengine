-- scenes/level01.lua
-- Level 01 scene - entity spawning and phase callbacks
-- Called when switching to the "level01" scene

local M = {}

-- local ball_bounces = 0
-- local player_hits = 0

-- ==================== COLLISION CALLBACK FUNCTIONS ====================
-- These are called when collisions occur between entities with matching groups.
-- ctx = {
--   a = { id, group, pos={x,y}, vel={x,y}, rect={x,y,w,h}, signals={flags={...}, integers={...}} },
--   b = { ...same... },
--   sides = { a = {"left","top",...}, b = {...} }
-- }


-- ==================== PHASE CALLBACK FUNCTIONS ====================
-- These are named functions called directly by the engine based on
-- the phase definitions in :with_phase()
-- Phase callbacks now receive EntityContext (ctx) instead of entity_id

--- Called when entering "init" phase (not used, we just transition immediately)
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function scene_init_update(ctx, input, dt)
    -- Immediately transition to get_started
    engine.phase_transition(ctx.id, "get_started")
end

--- Called when entering "get_started" phase
--- - Play "player_ready" music
--- - Spawn the ball (stuck to player paddle)
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function scene_get_started_enter(ctx, input)
    engine.log_info("Entering get_started phase - spawning ball")

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

-- ==================== BALL PHASE CALLBACKS ====================

--- Called when ball enters "stuck_to_player" phase
--- Attach ball to player with stored velocity
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function ball_stuck_enter(ctx, input)
    --[[     engine.log_info("Ball stuck to player - attaching... ball_id=" ..
        tostring(ctx.id) .. " prev=" .. tostring(ctx.previous_phase))

    -- Get player entity
    local player_id = engine.get_entity("player")
    if not player_id then
        engine.log_error("Player entity not found! Cannot attach ball.")
        return
    end
    engine.log_info("Found player entity: " .. tostring(player_id))

    -- Get stored ball data from world signals
    local offset_x = engine.get_scalar("ball_stick_offset_x") or 0
    local vx = engine.get_scalar("ball_stick_vx") or 300
    local vy = engine.get_scalar("ball_stick_vy") or -300

    engine.log_info("Ball stick data: offset_x=" ..
        tostring(offset_x) .. " vx=" .. tostring(vx) .. " vy=" .. tostring(vy))

    -- Stop the ball and attach to player
    engine.entity_set_velocity(ctx.id, 0, 0)
    engine.entity_insert_stuckto(ctx.id, player_id, true, false, offset_x, 0, vx, vy)

    engine.log_info("Ball attached to player with StuckTo!") ]]
end

--- Called each frame while ball is stuck to player
--- After 2 seconds, release the ball
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
function ball_stuck_update(ctx, input, dt)
    --[[     if ctx.time_in_phase >= 2.0 then
        engine.log_info("Releasing ball!")
        engine.phase_transition(ctx.id, "moving")
    end ]]
end

--- Called when ball enters "moving" phase (released from paddle)
--- Remove StuckTo component and restore stored velocity to RigidBody
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
function ball_moving_enter(ctx, input)
    --[[     engine.log_info("Ball released and moving!")
    -- Release from StuckTo - this removes the component and adds RigidBody with stored velocity
    engine.release_stuckto(ctx.id) ]]
end

-- ==================== SHIP PHASE CALLBACKS AND HELPERS ====================

--- Helper function to rotate the ship based on input
--- @param ctx EntityContext Entity context table
--- @param input Input Input state table
--- @param dt number Delta time in seconds
local function rotate_ship(ctx, input, dt)
    local rotation_speed = 360.0   -- degrees per second
    local propulsion_accel = 300.0 -- units per second squared
    -- engine.log_info("Current rotation: " .. tostring(CURRENT_ROTATION))
    -- Note: ctx.rotation contains current rotation if available
    current_rotation = ctx.rotation or 0.0

    -- calculate acceleration vector based on current rotation
    local radians = math.rad(current_rotation)
    local accel_x = math.sin(radians) * propulsion_accel
    local accel_y = -math.cos(radians) * propulsion_accel
    engine.entity_set_force_value(ctx.id, "propulsion", accel_x, accel_y)

    if input.digital.left.pressed then
        -- engine.log_info("Rotating left")
        current_rotation = current_rotation - rotation_speed * dt
    end
    if input.digital.right.pressed then
        -- engine.log_info("Rotating right")
        current_rotation = current_rotation + rotation_speed * dt
    end

    engine.entity_set_rotation(ctx.id, current_rotation)
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

    -- Handle rotation input
    rotate_ship(ctx, input, dt)

    -- If "up" is just pressed, switch to propulsion phase
    if input.digital.up.just_pressed then
        engine.phase_transition(ctx.id, "propulsion")
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
    -- Handle rotation input
    rotate_ship(ctx, input, dt)
    -- When "up" is released, go back to idle
    if input.digital.up.just_released then
        engine.phase_transition(ctx.id, "idle")
    end
end

-- ==================== SCENE GAME STATE CALLBACKS ====================

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
        :with_zindex(1)
        :with_sprite("ship_sheet", 64, 64, 32, 32)
        :with_animation("ship_idle")
        :with_collider(64, 64, 32, 32) -- Same size collider
        :with_rotation(0.0)
        :with_velocity(0, 0)
        :with_friction(1.0)
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
        :register_as("ship") -- Store entity ID for ball attachment
        :build()

    engine.log_info("Ship spawned!")
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

    --[[     -- reset ball bounce and player hit counters
    ball_bounces = 0
    player_hits = 0

    -- Reset score and lives for a new game
    engine.set_integer("score", 0)
    engine.set_integer("lives", 3)

    -- Set camera to center of the level
    -- target: center of map (tile_size * map_width/2, tile_size * map_height/2)
    -- offset: center of screen (needs screen width/height from engine)
    -- For now we use the known tilemap dimensions: 28x32 tiles at 24px each
    local camera_target_x = TILE_SIZE * MAP_WIDTH * 0.5  -- 24 * 28 * 0.5 = 336
    local camera_target_y = TILE_SIZE * MAP_HEIGHT * 0.5 -- 24 * 32 * 0.5 = 384
    -- Screen offset: assuming 672x768 window (standard Arkanoid resolution)
    local camera_offset_x = 336.0                        -- 672 / 2
    local camera_offset_y = 384.0                        -- 768 / 2
    engine.set_camera(camera_target_x, camera_target_y, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- Track groups for entity counting (used by phase callbacks)
    engine.track_group("ball")
    engine.track_group("brick")

    -- Spawn tiles from loaded tilemap
    engine.spawn_tiles("level01")

    -- Spawn invisible wall colliders
    spawn_walls()

    -- Spawn the player paddle
    spawn_player()

    -- Spawn UI score texts
    spawn_ui_texts()

    -- Spawn bricks from grid layout
    spawn_bricks()

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
                    on_update = "scene_init_update"
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
 ]]
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

    -- Note: Most game logic (ball physics, brick destruction, etc.) is handled
    -- by the phase system callbacks above. This is just for input handling.
end

return M
