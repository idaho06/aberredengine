-- scenes/sidescroller/level01.lua
-- Sidescroller level 01 scene

local utils = require("lib.utils")
local M = {}

local running_speed = 80
local walking_speed = 40
local jump_speed = -100

-- ─── Helper functions (local) ─────────────────────────────────────────────────
--- Updates the player's facing direction based on input.
--- @param id number
--- @param input InputSnapshot
local function update_facing_direction(id, input)
    if input.digital.left.pressed and not input.digital.right.pressed then
        engine.entity_signal_clear_flag(id, "facing_right")
        engine.entity_signal_set_flag(id, "facing_left")
        engine.entity_set_sprite_flip(id, true, false)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        engine.entity_signal_clear_flag(id, "facing_left")
        engine.entity_signal_set_flag(id, "facing_right")
        engine.entity_set_sprite_flip(id, false, false)
    end
end

-- ─── Callbacks (local — injected into _G by main.lua) ───────────────────────

--- Called each frame when sidescroller level01 scene is active.
--- This is called AFTER collision and phase callbacks!
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_sidescroller_level01(input, dt)
    if input.digital.back.just_pressed then
        engine.change_scene("menu")
    end
    -- reset the player "on_ground" signal at the start of each frame, it will be set again by the collision callback if we are still on the ground
    local player_id = engine.get_entity("player")
    if player_id then
        engine.entity_signal_clear_flag(player_id, "on_ground")
        -- engine.entity_signal_set_flag(player_id, "falling")
        -- engine.entity_set_force_enabled(player_id, "gravity", true)
        engine.log_info("Cleared player on_ground signal at end of frame.")
    end
end

--- Called when entering the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_running_on_enter(ctx, input)
    engine.log_info("Player started running!")
    engine.entity_signal_set_flag(ctx.id, "running")
    -- Check facing direction and set velocity accordingly
    if input.digital.left.pressed and not input.digital.right.pressed then
        engine.entity_set_velocity(ctx.id, -running_speed, 0)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        engine.entity_set_velocity(ctx.id, running_speed, 0)
    end
end

--- Called each frame while in the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_running_on_update(ctx, input, dt)
    if not utils.has_flag(ctx.signals.flags, "on_ground") and ctx.vel.y >= 0 then
        return "falling"
    end
    if input.digital.action_1.just_pressed then
        return "attack"
    end
    if input.digital.action_2.just_pressed then
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end
    if input.digital.left.pressed or input.digital.right.pressed then
        -- Keep running (or switch to walking if action_3 held)
        update_facing_direction(ctx.id, input)
        local has_single_direction = not (input.digital.left.pressed and input.digital.right.pressed)
        if has_single_direction and input.digital.action_3.pressed then
            return "walking"
        end
        if input.digital.left.pressed and not input.digital.right.pressed then
            engine.entity_set_velocity(ctx.id, -running_speed, 0)
        elseif input.digital.right.pressed and not input.digital.left.pressed then
            engine.entity_set_velocity(ctx.id, running_speed, 0)
        end
    else
        -- Transition to idle
        return "idle"
    end
end

--- Called when exiting the running phase.
--- @param ctx EntityContext Entity state
local function player_running_on_exit(ctx)
    engine.log_info("Player stopped running.")
    engine.entity_signal_clear_flag(ctx.id, "running")
end

--- Called when entering the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_walking_on_enter(ctx, input)
    engine.log_info("Player started walking!")
    engine.entity_signal_set_flag(ctx.id, "walking")
    update_facing_direction(ctx.id, input)
    if input.digital.left.pressed and not input.digital.right.pressed then
        engine.entity_set_velocity(ctx.id, -walking_speed, 0)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        engine.entity_set_velocity(ctx.id, walking_speed, 0)
    end
end

--- Called each frame while in the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_walking_on_update(ctx, input, dt)
    if not utils.has_flag(ctx.signals.flags, "on_ground") and ctx.vel.y >= 0 then
        return "falling"
    end
    if input.digital.action_1.just_pressed then
        return "attack"
    end
    if input.digital.action_2.just_pressed then
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end

    local has_direction = (input.digital.left.pressed or input.digital.right.pressed)
        and not (input.digital.left.pressed and input.digital.right.pressed)

    if not has_direction then
        return "idle"
    end

    update_facing_direction(ctx.id, input)

    if not input.digital.action_3.pressed then
        -- action_3 released while still moving — switch to running
        return "running"
    end

    -- Keep walking at walking speed
    if input.digital.left.pressed then
        engine.entity_set_velocity(ctx.id, -walking_speed, 0)
    else
        engine.entity_set_velocity(ctx.id, walking_speed, 0)
    end
end

--- Called when exiting the walking phase.
--- @param ctx EntityContext Entity state
local function player_walking_on_exit(ctx)
    engine.log_info("Player stopped walking.")
    engine.entity_signal_clear_flag(ctx.id, "walking")
end

--- Called when entering the falling phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_falling_on_enter(ctx, input)
    engine.log_info("Player started falling!")
    engine.entity_signal_set_flag(ctx.id, "falling")
end

--- Called each frame while in the falling phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_falling_on_update(ctx, input, dt)
    if utils.has_flag(ctx.signals.flags, "on_ground") then
        return "idle"
    end
    -- Allow horizontal steering at walking speed; preserve vertical velocity so gravity accumulates
    if input.digital.left.pressed and not input.digital.right.pressed then
        update_facing_direction(ctx.id, input)
        engine.entity_set_velocity(ctx.id, -walking_speed, ctx.vel.y)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        update_facing_direction(ctx.id, input)
        engine.entity_set_velocity(ctx.id, walking_speed, ctx.vel.y)
    end
end

--- Called when exiting the falling phase.
--- @param ctx EntityContext Entity state
local function player_falling_on_exit(ctx)
    engine.log_info("Player stopped falling.")
    engine.entity_signal_clear_flag(ctx.id, "falling")
end

--- Called when entering the jumping phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_jumping_on_enter(ctx, input)
    engine.log_info("Player started jumping!")
    engine.entity_signal_set_flag(ctx.id, "jumping")
end

--- Called each frame while in the jumping phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_jumping_on_update(ctx, input, dt)
    if utils.has_flag(ctx.signals.flags, "on_ground") then
        return "idle"
    end
    if ctx.vel.y > 0 then
        return "falling"
    end
    -- Allow horizontal steering while rising
    if input.digital.left.pressed and not input.digital.right.pressed then
        update_facing_direction(ctx.id, input)
        engine.entity_set_velocity(ctx.id, -walking_speed, ctx.vel.y)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        update_facing_direction(ctx.id, input)
        engine.entity_set_velocity(ctx.id, walking_speed, ctx.vel.y)
    end
end

--- Called when exiting the jumping phase.
--- @param ctx EntityContext Entity state
local function player_jumping_on_exit(ctx)
    engine.log_info("Player stopped jumping.")
    engine.entity_signal_clear_flag(ctx.id, "jumping")
end

--- Called when entering the idle phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_idle_on_enter(ctx, input)
    engine.log_info("Player is idle.")
    -- engine.entity_set_animation(ctx.id, "sidescroller-char_red_idle")
    -- remove horizontal movement when entering idle
    engine.entity_set_velocity(ctx.id, 0, ctx.vel.y)
end

--- Called each frame while in the idle phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_idle_on_update(ctx, input, dt)
    -- check for presence of "on_ground" signal, if not present, transition to falling
    -- engine.log_info(utils.dump_value(ctx, 4))
    local flags = ctx.signals.flags
    -- engine.log_info(utils.dump_value(flags, 4))
    if not utils.has_flag(flags, "on_ground") and ctx.vel.y >= 0 then
        engine.log_info("Player update: walked off ledge, transitioning to falling.")
        return "falling"
    end

    if input.digital.action_1.just_pressed then
        return "attack"
    end

    if input.digital.action_2.just_pressed then
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end

    local has_direction = (input.digital.left.pressed or input.digital.right.pressed)
        and not (input.digital.left.pressed and input.digital.right.pressed)

    if has_direction then
        update_facing_direction(ctx.id, input)
        if input.digital.action_3.pressed then
            return "walking"
        else
            return "running"
        end
    end
end

--- Called when entering the attack phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_attack_on_enter(ctx, input)
    engine.log_info("Player is attacking!")
    engine.entity_signal_set_flag(ctx.id, "attack")
    -- engine.entity_freeze(ctx.id)
    -- engine.entity_restart_animation(ctx.id)
    engine.entity_set_velocity(ctx.id, 0, 0)
    -- Spawn hitbox child entity in front of the player
    local offset_x = 20
    if utils.has_flag(ctx.signals.flags, "facing_left") then
        offset_x = -20
    end
    engine.spawn()
        :with_position(offset_x, 0)
        :with_collider(14, 30, 7, 30)
        :with_group("player_damage")
        :with_parent(ctx.id)
        :register_as("attack_hitbox")
        :build()
end

--- Called each frame while in the attack phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_attack_on_update(ctx, input, dt)
    if not utils.has_flag(ctx.signals.flags, "on_ground") and ctx.vel.y >= 0 then
        return "falling"
    end
    if utils.has_flag(ctx.signals.flags, "animation_ended") then
        engine.entity_signal_clear_flag(ctx.id, "animation_ended")
        return "idle"
    end
end

--- Called when exiting the attack phase.
--- @param ctx EntityContext Entity state
local function player_attack_on_exit(ctx)
    engine.log_info("Player finished attacking.")
    engine.entity_signal_clear_flag(ctx.id, "attack")
    -- engine.entity_unfreeze(ctx.id)
    -- Despawn the hitbox child
    local hitbox_id = engine.get_entity("attack_hitbox")
    if hitbox_id then
        engine.entity_despawn(hitbox_id)
        engine.remove_entity("attack_hitbox")
    end
end

--- Collision ground/player callback.
--- @param ctx CollisionContext
local function collision_ground_player(ctx)
    -- entity A is ground, entity B is player
    -- check that the player side colliding with the ground is the bottom
    -- engine.log_info(utils.dump_value(ctx, 4))

    -- look for "bottom" in ctx.sides.b
    local player_on_ground = false

    -- for _, side in pairs(ctx.sides.b) do
    --     if side == "bottom" then
    --         player_on_ground = true
    --         break
    --     end -- TODO: If we have a collision, but it's not the bottom, then we are touching a wall or ceiling!
    -- end

    if utils.has_flag(ctx.sides.b, "bottom") and utils.has_flag(ctx.sides.a, "top") then
        player_on_ground = true
    end


    if player_on_ground then
        engine.collision_entity_signal_set_flag(ctx.b.id, "on_ground")
        engine.collision_entity_signal_clear_flag(ctx.b.id, "falling")
        engine.collision_entity_signal_clear_flag(ctx.b.id, "jumping")
        -- engine.entity_signal_set_flag(ctx.b.id, "on_ground")
        -- engine.entity_signal_clear_flag(ctx.b.id, "falling")
        engine.log_info("collision: setting player `on_ground` signal")
        -- engine.collision_entity_set_force_enabled(ctx.b.id, "gravity", false)
        -- reset vertical velocity to 0 to prevent sliding down slopes
        local vel = ctx.b.vel
        engine.collision_entity_set_velocity(ctx.b.id, vel.x, 0)
        -- reset vertical position to be exactly on top of the ground to prevent sinking due to gravity
        -- local player_pos = ctx.b.pos
        local ground_rect = ctx.a.rect
        engine.collision_entity_set_position(ctx.b.id, ctx.b.pos.x, ground_rect.y)
    else
        engine.collision_entity_signal_clear_flag(ctx.b.id, "on_ground")
        engine.log_info("collision: removing player `on_ground` signal")
        engine.collision_entity_signal_set_flag(ctx.b.id, "falling")
        engine.collision_entity_set_force_enabled(ctx.b.id, "gravity", true)
    end
end

-- ─── Callback registry ──────────────────────────────────────────────────────
-- Every function the engine calls by name must appear here.
-- Keys must exactly match the strings passed to the engine.

M._callbacks = {
    on_update_sidescroller_level01 = on_update_sidescroller_level01,
    player_running_on_enter = player_running_on_enter,
    player_running_on_update = player_running_on_update,
    player_running_on_exit = player_running_on_exit,
    player_walking_on_enter = player_walking_on_enter,
    player_walking_on_update = player_walking_on_update,
    player_walking_on_exit = player_walking_on_exit,
    player_falling_on_enter = player_falling_on_enter,
    player_falling_on_update = player_falling_on_update,
    player_falling_on_exit = player_falling_on_exit,
    player_jumping_on_enter = player_jumping_on_enter,
    player_jumping_on_update = player_jumping_on_update,
    player_jumping_on_exit = player_jumping_on_exit,
    player_idle_on_enter = player_idle_on_enter,
    player_idle_on_update = player_idle_on_update,
    player_attack_on_enter = player_attack_on_enter,
    player_attack_on_update = player_attack_on_update,
    player_attack_on_exit = player_attack_on_exit,
    collision_ground_player = collision_ground_player,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the sidescroller level01 scene.
function M.spawn()
    engine.log_info("Spawning sidescroller level01 scene...")

    -- Set render resolution
    engine.set_render_size(640, 360)

    -- Set camera
    engine.set_camera(0, 0, 640 / 2, 360 / 2, 0.0, 1.0) -- target to 0,0, centered, no rotation, default zoom

    -- Set background color
    engine.set_background_color(20, 20, 30)

    -- Clear post-processing shader
    engine.post_process_shader(nil)

    -- Spawn player character
    engine.spawn()
        :with_sprite("sidescroller-char_red_1_sheet", 56, 56, 56 / 2, 56)
        :with_animation("sidescroller-char_red_idle")
        :with_zindex(0)
        :with_position(0, 0)
        :with_group("player")
    --:with_signal_flag("on_ground")
        :with_signal_flag("facing_right")
        :with_phase({
            initial = "idle",
            phases = {
                idle = {
                    on_enter = "player_idle_on_enter",
                    on_update = "player_idle_on_update"
                    -- on_exit = "player_idle_on_exit"
                },
                running = {
                    on_enter = "player_running_on_enter",
                    on_update = "player_running_on_update",
                    on_exit = "player_running_on_exit"
                },
                walking = {
                    on_enter = "player_walking_on_enter",
                    on_update = "player_walking_on_update",
                    on_exit = "player_walking_on_exit"
                },
                attack = {
                    on_enter = "player_attack_on_enter",
                    on_update = "player_attack_on_update",
                    on_exit = "player_attack_on_exit"
                },
                falling = {
                    on_enter = "player_falling_on_enter",
                    on_update = "player_falling_on_update",
                    on_exit = "player_falling_on_exit"
                },
                jumping = {
                    on_enter = "player_jumping_on_enter",
                    on_update = "player_jumping_on_update",
                    on_exit = "player_jumping_on_exit"
                }
            }
        })
        :with_collider(20, 24, 10, 24)
        :with_collider_offset(0, 0)
        :with_accel("gravity", 0, 180, true)
        :with_animation_controller("sidescroller-char_red_idle")
    -- :with_animation_rule({
    --     type = "all",
    --     conditions = {
    --         { type = "has_flag", key = "on_ground" },
    --         { type = "has_flag", key = "attack" }
    --     }
    -- }, "sidescroller-char_red_attack")
        :with_animation_rule({
            type = "has_flag", key = "attack"
        }, "sidescroller-char_red_attack")
        :with_animation_rule({
            type = "has_flag", key = "walking"
        }, "sidescroller-char_red_walk")
        :with_animation_rule({
            type = "has_flag", key = "jumping"
        }, "sidescroller-char_red_jump")
        :with_animation_rule({
            type = "has_flag", key = "falling"
        }, "sidescroller-char_red_jump_falling")
        :with_animation_rule({
            type = "has_flag", key = "running"
        }, "sidescroller-char_red_run")
        :register_as("player")
        :build()

    -- Spawn ground platforms
    engine.spawn()
        :with_collider(360, 32, 0, 0)
        :with_position(-320, 20)
        :with_group("ground")
        :build()
    engine.spawn()
        :with_collider(360, 32, 0, 0)
        :with_position(0, 20 + 32)
        :with_group("ground")
        :build()

    -- Spawn collision rules for ground-player
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("ground", "player", "collision_ground_player")
        :build()

    engine.log_info("Sidescroller level01 scene entities queued!")
end

return M
