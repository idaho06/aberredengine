-- scenes/level01.lua
-- Level 01 scene - entity spawning and phase callbacks
-- Called when switching to the "level01" scene

local M = {}

local ball_bounces = 0
local player_hits = 0

-- ==================== COLLISION CALLBACK FUNCTIONS ====================
-- These are called when collisions occur between entities with matching groups.
-- ctx = {
--   a = { id, group, pos={x,y}, vel={x,y}, rect={x,y,w,h}, signals={flags={...}, integers={...}} },
--   b = { ...same... },
--   sides = { a = {"left","top",...}, b = {...} }
-- }

--- Player-Walls collision: Clamp player X position
function on_player_walls(ctx)
    -- Player is 'a' (alphabetically first), wall is 'b'
    local player_x = ctx.a.pos.x
    -- Clamp to playable area (72 to 600)
    local clamped_x = math.max(72, math.min(600, player_x))
    if clamped_x ~= player_x then
        engine.collision_entity_set_position(ctx.a.id, clamped_x, ctx.a.pos.y)
    end
end

--- Ball-Walls collision: Bounce ball off walls
function on_ball_walls(ctx)
    -- Ball is 'a', wall is 'b'
    local ball_id = ctx.a.id
    local ball_pos = ctx.a.pos
    local ball_vel = ctx.a.vel
    local ball_rect = ctx.a.rect
    local wall_pos = ctx.b.pos

    local new_vx = ball_vel.x
    local new_vy = ball_vel.y
    local new_x = ball_pos.x
    local new_y = ball_pos.y

    -- Determine wall type based on relative position
    -- Wall positions: lateral walls at bottom left/right, top wall at center top
    if wall_pos.y < ball_pos.y then
        -- Collision with top wall: bounce down
        new_vy = math.abs(ball_vel.y)
        -- Fix position to be just below the wall
        -- Get wall collider height from rect
        local wall_height = ctx.b.rect and ctx.b.rect.h or 24
        new_y = wall_pos.y + wall_height + (ball_rect.h * 0.5)
    else
        -- Collision with lateral wall
        local wall_width = ctx.b.rect and ctx.b.rect.w or 24
        if ball_pos.x < wall_pos.x then
            -- Ball is left of wall position -> right wall collision
            new_vx = -math.abs(ball_vel.x)
            new_x = wall_pos.x - wall_width - (ball_rect.w * 0.5)
        else
            -- Ball is right of wall position -> left wall collision
            new_vx = math.abs(ball_vel.x)
            new_x = wall_pos.x + wall_width + (ball_rect.w * 0.5)
        end
    end

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_entity_set_position(ball_id, new_x, new_y)

    -- Increment bounce counter
    ball_bounces = ball_bounces + 1
end

--- Ball-Player collision: Reflect ball based on hit position, handle sticky
function on_ball_player(ctx)
    -- Ball is 'a', player is 'b'
    local ball_id = ctx.a.id
    local ball_pos = ctx.a.pos
    local ball_vel = ctx.a.vel
    local ball_rect = ctx.a.rect
    local player_id = ctx.b.id
    local player_pos = ctx.b.pos
    local player_rect = ctx.b.rect
    local player_signals = ctx.b.signals

    -- Calculate reflection angle based on hit position
    local hit_pos = ball_pos.x - player_pos.x
    local paddle_half_width = 48.0                        -- 96 / 2
    local relative_hit_pos = hit_pos / paddle_half_width
    local bounce_angle = relative_hit_pos * (math.pi / 3) -- Max 60 degrees
    local speed = math.sqrt(ball_vel.x * ball_vel.x + ball_vel.y * ball_vel.y)

    local new_vx = speed * math.sin(bounce_angle)
    local new_vy = -speed * math.cos(bounce_angle)
    local new_y = player_pos.y - player_rect.h - (ball_rect.h * 0.5)

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_entity_set_position(ball_id, ball_pos.x, new_y)

    -- Check for sticky powerup
    local is_sticky = false
    if player_signals and player_signals.flags then
        for _, flag in ipairs(player_signals.flags) do
            if flag == "sticky" then
                is_sticky = true
                break
            end
        end
    end

    if is_sticky then
        -- Store ball data in world signals for the ball's phase to use
        local offset_x = ball_pos.x - player_pos.x
        engine.set_scalar("ball_stick_offset_x", offset_x)
        engine.set_scalar("ball_stick_vx", new_vx)
        engine.set_scalar("ball_stick_vy", new_vy)

        -- Transition ball to stuck_to_player phase (ball's phase will handle StuckTo)
        engine.phase_transition(ball_id, "stuck_to_player")
    end

    -- Transition player to "hit" phase (handles animation flag)
    engine.phase_transition(player_id, "hit")

    -- Play ping sound
    engine.collision_play_sound("ping")

    -- Increment player hit counter
    player_hits = player_hits + 1

    -- Increment ball bounce counter
    ball_bounces = ball_bounces + 1
end

--- Ball-Brick collision: Bounce, decrement HP, update score, despawn if dead
function on_ball_brick(ctx)
    -- Ball is 'a', brick is 'b'
    local ball_id = ctx.a.id
    local ball_rect = ctx.a.rect
    local ball_vel = ctx.a.vel
    local ball_pos = ctx.a.pos
    local brick_id = ctx.b.id
    local brick_rect = ctx.b.rect
    local brick_signals = ctx.b.signals

    -- Bounce ball based on colliding sides of the brick
    local new_vx = ball_vel.x
    local new_vy = ball_vel.y
    local new_x = ball_pos.x
    local new_y = ball_pos.y

    for _, side in ipairs(ctx.sides.b) do
        if side == "top" then
            new_vy = -math.abs(ball_vel.y)
            new_y = brick_rect.y - (ball_rect.h * 0.5)
        elseif side == "bottom" then
            new_vy = math.abs(ball_vel.y)
            new_y = brick_rect.y + brick_rect.h + (ball_rect.h * 0.5)
        elseif side == "left" then
            new_vx = -math.abs(ball_vel.x)
            new_x = brick_rect.x - (ball_rect.w * 0.5)
        elseif side == "right" then
            new_vx = math.abs(ball_vel.x)
            new_x = brick_rect.x + brick_rect.w + (ball_rect.w * 0.5)
        end
    end

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_entity_set_position(ball_id, new_x, new_y)

    -- Handle brick HP and score
    local hp = 1
    local points = 0
    if brick_signals and brick_signals.integers then
        hp = brick_signals.integers.hp or 1
        points = brick_signals.integers.points or 0
    end

    if hp > 1 then
        -- Just decrement HP
        engine.collision_entity_signal_set_integer(brick_id, "hp", hp - 1)
    else
        -- Brick destroyed
        if points > 0 then
            local current_score = engine.get_integer("score") or 0
            engine.collision_set_integer("score", current_score + points)

            -- Update high score if necessary
            local high_score = engine.get_integer("high_score") or 0
            if current_score + points > high_score then
                engine.collision_set_integer("high_score", current_score + points)
            end
        end
        -- Despawn brick
        engine.collision_entity_despawn(brick_id)
    end

    -- Play ding sound
    engine.collision_play_sound("ding")

    -- Increment ball bounce counter
    ball_bounces = ball_bounces + 1
end

--- Ball-OOB collision: Despawn ball when fully inside OOB zone
function on_ball_oob(ctx)
    -- Ball is 'a', oob_wall is 'b'
    -- If all 4 sides of ball are colliding (ball fully inside oob), despawn it
    local ball_sides = ctx.sides.a
    if ball_sides and #ball_sides == 4 then
        engine.collision_entity_despawn(ctx.a.id)
    end
end

--- Spawn the collision rule entities for level01
local function spawn_collision_rules()
    -- Player-Walls collision rule
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("player", "walls", "on_player_walls")
        :build()

    -- Ball-Walls collision rule
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "walls", "on_ball_walls")
        :build()

    -- Ball-Player collision rule
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "player", "on_ball_player")
        :build()

    -- Ball-Brick collision rule
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "brick", "on_ball_brick")
        :build()

    -- Ball-OOB collision rule
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "oob_wall", "on_ball_oob")
        :build()

    engine.log_info("Collision rules spawned!")
end

-- ==================== PHASE CALLBACK FUNCTIONS ====================
-- These are named functions called directly by the engine based on
-- the phase definitions in :with_phase()

--- Called when entering "init" phase (not used, we just transition immediately)
function scene_init_update(entity_id, input, time_in_phase, dt)
    -- Immediately transition to get_started
    engine.phase_transition(entity_id, "get_started")
end

--- Called when entering "get_started" phase
--- - Play "player_ready" music
--- - Spawn the ball (stuck to player paddle)
function scene_get_started_enter(entity_id, input, previous_phase)
    engine.log_info("Entering get_started phase - spawning ball")

    -- Play "player_ready" music (no loop)
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

    engine.log_info("Ball spawned with StuckTo!")
end

--- Called each frame in "get_started" phase
function scene_get_started_update(entity_id, input, time_in_phase, dt)
    -- Transition to playing after setup
    engine.phase_transition(entity_id, "playing")
end

-- ==================== BALL PHASE CALLBACKS ====================

--- Called when ball enters "stuck_to_player" phase
--- Attach ball to player with stored velocity
function ball_stuck_enter(entity_id, input, previous_phase)
    engine.log_info("Ball stuck to player - attaching... ball_id=" ..
        tostring(entity_id) .. " prev=" .. tostring(previous_phase))

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
    engine.entity_set_velocity(entity_id, 0, 0)
    engine.entity_insert_stuckto(entity_id, player_id, true, false, offset_x, 0, vx, vy)

    engine.log_info("Ball attached to player with StuckTo!")
end

--- Called each frame while ball is stuck to player
--- After 2 seconds, release the ball
function ball_stuck_update(entity_id, input, time_in_phase, dt)
    if time_in_phase >= 2.0 then
        engine.log_info("Releasing ball!")
        engine.phase_transition(entity_id, "moving")
    end
end

--- Called when ball enters "moving" phase (released from paddle)
--- Remove StuckTo component and restore stored velocity to RigidBody
function ball_moving_enter(entity_id, input, previous_phase)
    engine.log_info("Ball released and moving!")
    -- Release from StuckTo - this removes the component and adds RigidBody with stored velocity
    engine.release_stuckto(entity_id)
end

-- ==================== PLAYER PHASE CALLBACKS ====================

--- Called when player enters "sticky" phase
--- Set the "sticky" flag so ball sticks on collision, and play glowing animation
function player_sticky_enter(entity_id, input, previous_phase)
    engine.log_info("Player entering sticky phase")
    engine.entity_signal_set_flag(entity_id, "sticky")
    engine.entity_set_animation(entity_id, "vaus_glowing")
end

--- Called each frame while player is in "sticky" phase
--- After 3 seconds, transition to "glowing" phase
function player_sticky_update(entity_id, input, time_in_phase, dt)
    if time_in_phase >= 3.0 then
        engine.log_info("Sticky powerup expired!")
        engine.phase_transition(entity_id, "glowing")
    end
end

--- Called when player enters "glowing" phase
--- Clear the "sticky" flag and play glowing animation
function player_glowing_enter(entity_id, input, previous_phase)
    engine.log_info("Player entering glowing phase")
    engine.entity_signal_clear_flag(entity_id, "sticky")
    engine.entity_set_animation(entity_id, "vaus_glowing")
end

--- Called when player enters "hit" phase
--- Play the hit animation from frame 0
function player_hit_enter(entity_id, input, previous_phase)
    engine.log_info("Player entering hit phase")
    engine.entity_set_animation(entity_id, "vaus_hit")
end

--- Called each frame while player is in "hit" phase
--- After 0.5 seconds, transition back to "glowing" phase
function player_hit_update(entity_id, input, time_in_phase, dt)
    if time_in_phase >= 0.5 then
        engine.phase_transition(entity_id, "glowing")
    end
end

-- ==================== SCENE GAME STATE CALLBACKS ====================

--- Called each frame in "playing" phase
--- - Check for level cleared (no bricks)
--- - Check for ball lost (no balls)
function scene_playing_update(entity_id, input, time_in_phase, dt)
    -- Skip first few frames to let group counts update
    if time_in_phase < 0.1 then
        return
    end

    -- Check for level cleared (no bricks remaining)
    local brick_count = engine.get_group_count("brick")
    if brick_count ~= nil and brick_count == 0 then
        engine.log_info("All bricks destroyed - level cleared!")
        engine.phase_transition(entity_id, "level_cleared")
        return
    end

    -- Check for ball lost (no balls remaining)
    local ball_count = engine.get_group_count("ball")
    if ball_count ~= nil and ball_count == 0 then
        engine.log_info("No balls remaining - lose life!")
        engine.phase_transition(entity_id, "lose_life")
        return
    end
end

--- Called each frame in "lose_life" phase
--- - Decrement lives
--- - Transition to game_over or get_started
function scene_lose_life_update(entity_id, input, time_in_phase, dt)
    local lives = engine.get_integer("lives") or 0
    lives = lives - 1
    engine.set_integer("lives", lives)
    engine.log_info(string.format("Lost a life! Remaining lives: %d", lives))

    if lives < 1 then
        engine.phase_transition(entity_id, "game_over")
    else
        engine.phase_transition(entity_id, "get_started")
    end
end

--- Called when entering "game_over" phase
--- - Spawn "GAME OVER" text
function scene_game_over_enter(entity_id, input, previous_phase)
    engine.log_info("Game Over!")

    -- Spawn game over text
    engine.spawn()
        :with_group("game_over_text")
        :with_screen_position(200, 350)
        :with_text("GAME OVER", "future", 48, 255, 0, 0, 255) -- Red
        :with_zindex(100)
        :build()
end

--- Called each frame in "game_over" phase
--- - After 3 seconds, switch to menu scene
function scene_game_over_update(entity_id, input, time_in_phase, dt)
    if time_in_phase >= 3.0 then
        engine.log_info("Game over - returning to menu")
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")

        -- show bounces and hits in log
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
    end
end

--- Called when entering "level_cleared" phase
--- - Play success music
--- - Spawn "LEVEL CLEARED" text
function scene_level_cleared_enter(entity_id, input, previous_phase)
    engine.log_info("Level Cleared!")

    -- Play success music
    engine.play_music("success", false)

    -- Spawn level cleared text
    engine.spawn()
        :with_group("level_cleared_text")
        :with_screen_position(150, 350)
        :with_text("LEVEL CLEARED", "future", 48, 0, 255, 0, 255) -- Green
        :with_zindex(100)
        :build()
end

--- Called each frame in "level_cleared" phase
--- - After 4 seconds, switch to menu scene
function scene_level_cleared_update(entity_id, input, time_in_phase, dt)
    if time_in_phase >= 4.0 then
        engine.log_info("Level cleared - returning to menu")
        engine.set_string("scene", "menu")
        engine.set_flag("switch_scene")

        -- show bounces and hits in log
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
    end
end

-- ==================== ENTITY SPAWNING ====================

-- Level constants (matching tilemap info)
local TILE_SIZE = 24
local MAP_WIDTH = 28
local MAP_HEIGHT = 32

--- Spawn the invisible wall colliders for the level
local function spawn_walls()
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
end

--- Spawn the player paddle (Vaus)
local function spawn_player()
    -- Calculate player Y position: near bottom of play area
    local player_y = (TILE_SIZE * MAP_HEIGHT) - 36.0

    -- Store player_y in world signals for ball spawn positioning
    engine.set_scalar("player_y", player_y)

    -- The Vaus - the player paddle
    -- Animation is controlled directly from Lua phase callbacks (no animation controller)
    engine.spawn()
        :with_group("player")
        :with_position(400, player_y)
        :with_zindex(10)
        :with_sprite("vaus_sheet", 96, 24, 48, 24) -- 96x24 sprite, origin at bottom center
        :with_animation("vaus_glowing")            -- Start with glowing animation
        :with_collider(96, 24, 48, 24)             -- Same size collider
        :with_mouse_controlled(true, false)        -- Follow mouse X only
        :with_signals()
        :with_phase({
            initial = "sticky",
            phases = {
                sticky = {
                    on_enter = "player_sticky_enter",
                    on_update = "player_sticky_update"
                },
                glowing = {
                    on_enter = "player_glowing_enter"
                },
                hit = {
                    on_enter = "player_hit_enter",
                    on_update = "player_hit_update"
                }
            }
        })
        :register_as("player") -- Store entity ID for ball attachment
        :build()

    engine.log_info("Player paddle spawned!")
end

--- Spawn the UI score texts
local function spawn_ui_texts()
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
end

--- Spawn the bricks grid layout
local function spawn_bricks()
    -- Spawn an entity with GridLayout component
    -- The gridlayout_spawn_system will process the JSON and spawn brick entities
    engine.spawn()
        :with_grid_layout("./assets/levels/level01.json", "brick", 5)
        :build()

    engine.log_info("Bricks grid layout spawned!")

    -- TODO: Create a system to load json files and spawn entities from lua directly
end

--- Spawn all entities for level 01.
--- This is called when entering the scene (before phase system starts)
function M.spawn()
    engine.log_info("Spawning level01 scene entities from Lua...")

    -- reset ball bounce and player hit counters
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

    engine.log_info("Scene phase entity spawned with LuaPhase")
    engine.log_info("Level01 scene entities queued!")
end

--- Called each frame when level01 scene is active.
--- @param input table Input state table with digital/analog inputs
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
