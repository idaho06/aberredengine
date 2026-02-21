-- scenes/arkanoid/level01.lua
-- Arkanoid example — entity spawning and phase callbacks
-- Scene name: "arkanoid_level01"
-- Internal render resolution: 672x768

local M = {}

local ball_bounces = 0
local player_hits = 0

-- Level constants (matching tilemap info)
local TILE_SIZE = 24
local MAP_WIDTH = 28
local MAP_HEIGHT = 32

-- ==================== COLLISION CALLBACK FUNCTIONS ====================
-- ctx = {
--   a = { id, group, pos={x,y}, vel={x,y}, rect={x,y,w,h}, signals={...} },
--   b = { ...same... },
--   sides = { a = {"left","top",...}, b = {...} }
-- }

--- Player-Walls collision: Clamp player X position
local function on_player_walls(ctx)
    local player_x = ctx.a.pos.x
    local clamped_x = math.max(72, math.min(600, player_x))
    if clamped_x ~= player_x then
        engine.collision_entity_set_position(ctx.a.id, clamped_x, ctx.a.pos.y)
    end
end

--- Ball-Walls collision: Bounce ball off walls
local function on_ball_walls(ctx)
    local ball_id = ctx.a.id
    local ball_pos = ctx.a.pos
    local ball_vel = ctx.a.vel
    local ball_rect = ctx.a.rect
    local wall_pos = ctx.b.pos

    local new_vx = ball_vel.x
    local new_vy = ball_vel.y
    local new_x = ball_pos.x
    local new_y = ball_pos.y

    if wall_pos.y < ball_pos.y then
        -- Collision with top wall: bounce down
        new_vy = math.abs(ball_vel.y)
        local wall_height = ctx.b.rect and ctx.b.rect.h or 24
        new_y = wall_pos.y + wall_height + (ball_rect.h * 0.5)
    else
        -- Collision with lateral wall
        local wall_width = ctx.b.rect and ctx.b.rect.w or 24
        if ball_pos.x < wall_pos.x then
            new_vx = -math.abs(ball_vel.x)
            new_x = wall_pos.x - wall_width - (ball_rect.w * 0.5)
        else
            new_vx = math.abs(ball_vel.x)
            new_x = wall_pos.x + wall_width + (ball_rect.w * 0.5)
        end
    end

    engine.collision_entity_set_velocity(ball_id, new_vx, new_vy)
    engine.collision_entity_set_position(ball_id, new_x, new_y)

    ball_bounces = ball_bounces + 1
end

--- Ball-Player collision: Reflect ball based on hit position, handle sticky
local function on_ball_player(ctx)
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
    local paddle_half_width = 48.0
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
        local offset_x = ball_pos.x - player_pos.x
        engine.collision_set_scalar("ball_stick_offset_x", offset_x)
        engine.collision_set_scalar("ball_stick_vx", new_vx)
        engine.collision_set_scalar("ball_stick_vy", new_vy)
        engine.collision_phase_transition(ball_id, "stuck_to_player")
    end

    -- Transition player to "hit" phase
    engine.collision_phase_transition(player_id, "hit")

    engine.collision_play_sound("arkanoid-ping")

    player_hits = player_hits + 1
    ball_bounces = ball_bounces + 1
end

--- Ball-Brick collision: Bounce, decrement HP, update score, despawn if dead
local function on_ball_brick(ctx)
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
        engine.collision_entity_signal_set_integer(brick_id, "hp", hp - 1)
    else
        if points > 0 then
            local current_score = engine.get_integer("score") or 0
            engine.collision_set_integer("score", current_score + points)

            local high_score = engine.get_integer("high_score") or 0
            if current_score + points > high_score then
                engine.collision_set_integer("high_score", current_score + points)
            end
        end
        engine.collision_entity_despawn(brick_id)
    end

    engine.collision_play_sound("arkanoid-ding")

    ball_bounces = ball_bounces + 1
end

--- Ball-OOB collision: Despawn ball when fully inside OOB zone
local function on_ball_oob(ctx)
    local ball_sides = ctx.sides.a
    if ball_sides and #ball_sides == 4 then
        engine.collision_entity_despawn(ctx.a.id)
    end
end

-- ==================== PHASE CALLBACK FUNCTIONS ====================

--- Scene init phase: immediately transition to get_started
local function scene_init_update(ctx, input, dt)
    engine.phase_transition(ctx.id, "get_started")
end

--- Get started phase: spawn the ball stuck to the player paddle
local function scene_get_started_enter(ctx, input)
    engine.log_info("Entering get_started phase - spawning ball")

    engine.play_music("arkanoid-player_ready", false)

    local player_id = engine.get_entity("player")
    if not player_id then
        engine.log_error("Player entity not found in world signals! Cannot spawn ball.")
        return
    end

    engine.phase_transition(player_id, "sticky")

    local player_y = engine.get_scalar("player_y") or 700.0
    local ball_y = player_y - 24.0 - 6.0

    engine.spawn()
        :with_group("ball")
        :with_position(336, ball_y)
        :with_sprite("arkanoid-ball", 12, 12, 6, 6)
        :with_zindex(10)
        :with_collider(12, 12, 6, 6)
        :with_signals()
        :with_stuckto(player_id, true, false)
        :with_stuckto_offset(0, 0)
        :with_stuckto_stored_velocity(300, -300)
        :with_phase({
            initial = "stuck_to_player",
            phases = {
                stuck_to_player = {
                    on_enter = "ball_stuck_enter",
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

--- Get started phase update: transition to playing
local function scene_get_started_update(ctx, input, dt)
    return "playing"
end

-- ==================== BALL PHASE CALLBACKS ====================

--- Ball stuck to player: re-attach after collision bounce-back
local function ball_stuck_enter(ctx, input)
    engine.log_info("Ball stuck to player - attaching...")

    local player_id = engine.get_entity("player")
    if not player_id then
        engine.log_error("Player entity not found! Cannot attach ball.")
        return
    end

    local offset_x = engine.get_scalar("ball_stick_offset_x") or 0
    local vx = engine.get_scalar("ball_stick_vx") or 300
    local vy = engine.get_scalar("ball_stick_vy") or -300

    engine.entity_set_velocity(ctx.id, 0, 0)
    engine.entity_insert_stuckto(ctx.id, player_id, true, false, offset_x, 0, vx, vy)
end

--- Ball stuck update: release after 2 seconds
local function ball_stuck_update(ctx, input, dt)
    if ctx.time_in_phase >= 2.0 then
        engine.log_info("Releasing ball!")
        return "moving"
    end
end

--- Ball moving: release from paddle
local function ball_moving_enter(ctx, input)
    engine.log_info("Ball released and moving!")
    engine.release_stuckto(ctx.id)
end

-- ==================== PLAYER PHASE CALLBACKS ====================

--- Player sticky phase: set flag so ball sticks on collision
local function player_sticky_enter(ctx, input)
    engine.log_info("Player entering sticky phase")
    engine.entity_signal_set_flag(ctx.id, "sticky")
    engine.entity_set_animation(ctx.id, "arkanoid-vaus_glowing")
end

--- Player sticky update: expire after 3 seconds
local function player_sticky_update(ctx, input, dt)
    if ctx.time_in_phase >= 3.0 then
        engine.log_info("Sticky powerup expired!")
        return "glowing"
    end
end

--- Player glowing phase: clear sticky, keep glowing animation
local function player_glowing_enter(ctx, input)
    engine.log_info("Player entering glowing phase")
    engine.entity_signal_clear_flag(ctx.id, "sticky")
    engine.entity_set_animation(ctx.id, "arkanoid-vaus_glowing")
end

--- Player hit phase: play hit animation
local function player_hit_enter(ctx, input)
    engine.log_info("Player entering hit phase")
    engine.entity_set_animation(ctx.id, "arkanoid-vaus_hit")
end

--- Player hit update: return to glowing after 0.5s
local function player_hit_update(ctx, input, dt)
    if ctx.time_in_phase >= 0.5 then
        return "glowing"
    end
end

-- ==================== SCENE GAME STATE CALLBACKS ====================

--- Playing phase: check for level cleared or ball lost
local function scene_playing_update(ctx, input, dt)
    if ctx.time_in_phase < 0.1 then
        return
    end

    local brick_count = engine.get_group_count("brick")
    if brick_count ~= nil and brick_count == 0 then
        engine.log_info("All bricks destroyed - level cleared!")
        return "level_cleared"
    end

    local ball_count = engine.get_group_count("ball")
    if ball_count ~= nil and ball_count == 0 then
        engine.log_info("No balls remaining - lose life!")
        return "lose_life"
    end
end

--- Lose life phase: decrement lives, transition to game_over or get_started
local function scene_lose_life_update(ctx, input, dt)
    local lives = engine.get_integer("lives") or 0
    lives = lives - 1
    engine.set_integer("lives", lives)
    engine.log_info(string.format("Lost a life! Remaining lives: %d", lives))

    if lives < 1 then
        return "game_over"
    else
        return "get_started"
    end
end

--- Game over phase enter: spawn text
local function scene_game_over_enter(ctx, input)
    engine.log_info("Game Over!")

    engine.spawn()
        :with_group("game_over_text")
        :with_screen_position(200, 350)
        :with_text("GAME OVER", "future", 48, 255, 0, 0, 255)
        :with_zindex(100)
        :build()
end

--- Game over phase update: return to menu after 3 seconds
local function scene_game_over_update(ctx, input, dt)
    if ctx.time_in_phase >= 3.0 then
        engine.log_info("Game over - returning to menu")
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
        engine.change_scene("menu")
    end
end

--- Level cleared phase enter: play music, spawn text
local function scene_level_cleared_enter(ctx, input)
    engine.log_info("Level Cleared!")

    engine.play_music("arkanoid-success", false)

    engine.spawn()
        :with_group("level_cleared_text")
        :with_screen_position(150, 350)
        :with_text("LEVEL CLEARED", "future", 48, 0, 255, 0, 255)
        :with_zindex(100)
        :build()
end

--- Level cleared phase update: return to menu after 4 seconds
local function scene_level_cleared_update(ctx, input, dt)
    if ctx.time_in_phase >= 4.0 then
        engine.log_info("Level cleared - returning to menu")
        engine.log_info(string.format("Total ball bounces: %d", ball_bounces))
        engine.log_info(string.format("Total player hits: %d", player_hits))
        engine.change_scene("menu")
    end
end

-- ==================== SCENE UPDATE ====================

--- Called each frame when arkanoid_level01 scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_arkanoid_level01(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
end

-- ─── Callback registry ───────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- Keys must exactly match the strings passed to the engine.

M._callbacks = {
    -- Scene update
    on_update_arkanoid_level01  = on_update_arkanoid_level01,
    -- Collision callbacks
    on_player_walls             = on_player_walls,
    on_ball_walls               = on_ball_walls,
    on_ball_player              = on_ball_player,
    on_ball_brick               = on_ball_brick,
    on_ball_oob                 = on_ball_oob,
    -- Scene state phase callbacks
    scene_init_update           = scene_init_update,
    scene_get_started_enter     = scene_get_started_enter,
    scene_get_started_update    = scene_get_started_update,
    scene_playing_update        = scene_playing_update,
    scene_lose_life_update      = scene_lose_life_update,
    scene_game_over_enter       = scene_game_over_enter,
    scene_game_over_update      = scene_game_over_update,
    scene_level_cleared_enter   = scene_level_cleared_enter,
    scene_level_cleared_update  = scene_level_cleared_update,
    -- Ball phase callbacks
    ball_stuck_enter            = ball_stuck_enter,
    ball_stuck_update           = ball_stuck_update,
    ball_moving_enter           = ball_moving_enter,
    -- Player phase callbacks
    player_sticky_enter         = player_sticky_enter,
    player_sticky_update        = player_sticky_update,
    player_glowing_enter        = player_glowing_enter,
    player_hit_enter            = player_hit_enter,
    player_hit_update           = player_hit_update,
}

-- ==================== ENTITY SPAWNING ====================

--- Spawn collision rule entities
local function spawn_collision_rules()
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("player", "walls", "on_player_walls")
        :build()
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "walls", "on_ball_walls")
        :build()
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "player", "on_ball_player")
        :build()
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "brick", "on_ball_brick")
        :build()
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ball", "oob_wall", "on_ball_oob")
        :build()
    engine.log_info("Collision rules spawned!")
end

--- Spawn invisible wall colliders
local function spawn_walls()
    -- Left wall
    engine.spawn()
        :with_group("walls")
        :with_position(0, TILE_SIZE * MAP_HEIGHT)
        :with_collider(
            TILE_SIZE * 1,
            TILE_SIZE * (MAP_HEIGHT - 2),
            0,
            TILE_SIZE * (MAP_HEIGHT - 2)
        )
        :build()

    -- Right wall
    engine.spawn()
        :with_group("walls")
        :with_position(TILE_SIZE * MAP_WIDTH, TILE_SIZE * MAP_HEIGHT)
        :with_collider(
            TILE_SIZE * 1,
            TILE_SIZE * (MAP_HEIGHT - 2),
            TILE_SIZE * 1,
            TILE_SIZE * (MAP_HEIGHT - 2)
        )
        :build()

    -- Top wall
    engine.spawn()
        :with_group("walls")
        :with_position(TILE_SIZE * MAP_WIDTH * 0.5, TILE_SIZE * 2)
        :with_collider(
            TILE_SIZE * (MAP_WIDTH - 2),
            TILE_SIZE * 1,
            TILE_SIZE * (MAP_WIDTH - 2) * 0.5,
            0
        )
        :build()

    -- Out of bounds (bottom)
    engine.spawn()
        :with_group("oob_wall")
        :with_position(-(TILE_SIZE * 5), TILE_SIZE * MAP_HEIGHT)
        :with_collider(
            TILE_SIZE * (MAP_WIDTH + 10),
            TILE_SIZE * 10,
            0,
            0
        )
        :build()

    engine.log_info("Walls spawned!")
end

--- Spawn the player paddle (Vaus)
local function spawn_player()
    local player_y = (TILE_SIZE * MAP_HEIGHT) - 36.0
    engine.set_scalar("player_y", player_y)

    engine.spawn()
        :with_group("player")
        :with_position(400, player_y)
        :with_zindex(10)
        :with_sprite("arkanoid-vaus_sheet", 96, 24, 48, 24)
        :with_animation("arkanoid-vaus_glowing")
        :with_collider(96, 24, 48, 24)
        :with_mouse_controlled(true, false)
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
        :register_as("player")
        :build()

    engine.log_info("Player paddle spawned!")
end

--- Spawn UI score texts
local function spawn_ui_texts()
    engine.spawn()
        :with_group("ui")
        :with_position(TILE_SIZE * 3, 0)
        :with_text("1UP   HIGH SCORE", "arcade", TILE_SIZE, 255, 0, 0, 255)
        :with_zindex(20)
        :build()

    engine.spawn()
        :with_group("player_score")
        :with_position(TILE_SIZE * 3, TILE_SIZE)
        :with_text("0", "arcade", TILE_SIZE, 255, 255, 255, 255)
        :with_zindex(20)
        :with_signal_binding("score")
        :build()

    engine.spawn()
        :with_group("high_score")
        :with_position(TILE_SIZE * 10, TILE_SIZE)
        :with_text("0", "arcade", TILE_SIZE, 255, 255, 255, 255)
        :with_zindex(20)
        :with_signal_binding("high_score")
        :build()

    engine.log_info("UI texts spawned!")
end

--- Spawn bricks via grid layout
local function spawn_bricks()
    engine.spawn()
        :with_grid_layout("./assets/levels/arkanoid/level01.json", "brick", 5)
        :build()
    engine.log_info("Bricks grid layout spawned!")
end

--- Spawn all entities for the Arkanoid level.
function M.spawn()
    engine.log_info("Spawning Arkanoid level01 scene entities...")

    -- Set render resolution for Arkanoid (672x768)
    engine.set_render_size(672, 768)

    -- Reset counters
    ball_bounces = 0
    player_hits = 0

    -- Reset score and lives
    engine.set_integer("score", 0)
    engine.set_integer("lives", 3)

    -- Camera centered on tilemap
    local camera_target_x = TILE_SIZE * MAP_WIDTH * 0.5   -- 336
    local camera_target_y = TILE_SIZE * MAP_HEIGHT * 0.5  -- 384
    local camera_offset_x = 336.0                         -- 672 / 2
    local camera_offset_y = 384.0                         -- 768 / 2
    engine.set_camera(camera_target_x, camera_target_y, camera_offset_x, camera_offset_y, 0.0, 1.0)

    -- Track groups for entity counting
    engine.track_group("ball")
    engine.track_group("brick")

    -- Spawn tilemap background
    engine.spawn_tiles("arkanoid-level01")

    -- Spawn game entities
    spawn_walls()
    spawn_player()
    spawn_ui_texts()
    spawn_bricks()
    spawn_collision_rules()

    -- Scene phase state machine
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

    engine.log_info("Arkanoid level01 scene entities queued!")
end

return M
