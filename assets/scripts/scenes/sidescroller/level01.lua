-- scenes/sidescroller/level01.lua
-- Sidescroller level 01 scene

local utils = require("lib.utils")
local M = {}

local running_speed = 80
local walking_speed = 40
local jump_speed = -100
local debug_log = true

-- ─── Helper functions (local) ─────────────────────────────────────────────────
local function log_debug(message)
    if debug_log then
        engine.log_debug(message)
    end
end

--- Updates the player's facing direction based on input.
--- @param id number
--- @param input InputSnapshot
local function update_facing_direction(id, input)
    if input.digital.left.pressed and not input.digital.right.pressed then
        log_debug(string.format("update_facing_direction id=%d -> LEFT", id))
        engine.entity_signal_clear_flag(id, "facing_right")
        engine.entity_signal_set_flag(id, "facing_left")
        engine.entity_set_sprite_flip(id, true, false)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        log_debug(string.format("update_facing_direction id=%d -> RIGHT", id))
        engine.entity_signal_clear_flag(id, "facing_left")
        engine.entity_signal_set_flag(id, "facing_right")
        engine.entity_set_sprite_flip(id, false, false)
    end
end

--- Sets horizontal velocity unless the wall on that side is blocking.
--- Negative vx checks touching_wall_left; positive vx checks touching_wall_right.
--- @param ctx EntityContext
--- @param vx number
--- @param vy number
local function set_hvel(ctx, vx, vy)
    local wall = vx < 0 and "touching_wall_left" or "touching_wall_right"
    local blocked = utils.has_flag(ctx.signals.flags, wall)
    log_debug(string.format("set_hvel id=%d vx=%.1f vy=%.1f wall=%s blocked=%s",
        ctx.id, vx, vy, wall, tostring(blocked)))
    if not blocked then
        engine.entity_set_velocity(ctx.id, vx, vy)
    end
end

--- Applies wall-stop: sets signal, snaps x position, zeroes x velocity.
--- @param ctx CollisionContext
--- @param flag string
--- @param snap_x number
local function apply_wall_stop(ctx, flag, snap_x)
    log_debug(string.format("apply_wall_stop flag=%s snap_x=%.1f cur_x=%.1f vel_x=%.1f vel_y=%.1f",
        flag, snap_x, ctx.b.pos.x, ctx.b.vel.x, ctx.b.vel.y))
    engine.collision_entity_signal_set_flag(ctx.b.id, flag)
    engine.collision_entity_set_position(ctx.b.id, snap_x, ctx.b.pos.y)
    engine.collision_entity_set_velocity(ctx.b.id, 0, ctx.b.vel.y)
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

    -- Pin background layers to the camera view each frame
    local rect = engine.get_camera_view_rect()
    local bg1 = engine.get_entity("bg_layer01")
    if bg1 then engine.entity_set_position(bg1, rect.x, rect.y) end
    local bg2 = engine.get_entity("bg_layer02")
    if bg2 then engine.entity_set_position(bg2, rect.x, rect.y) end
    local bg3 = engine.get_entity("bg_layer03")
    if bg3 then engine.entity_set_position(bg3, rect.x, rect.y) end

    -- cleared each frame; collision callbacks re-set these if contact persists
    local player_id = engine.get_entity("player")
    if player_id then
        engine.entity_signal_clear_flag(player_id, "on_ground")
        engine.entity_signal_clear_flag(player_id, "touching_wall_left")
        engine.entity_signal_clear_flag(player_id, "touching_wall_right")
        engine.entity_signal_clear_flag(player_id, "touching_ceiling")
        -- engine.entity_signal_set_flag(player_id, "falling")
        -- engine.entity_set_force_enabled(player_id, "gravity", true)
        log_debug(string.format(
            "on_update: cleared flags for player_id=%d | back.just_pressed=%s dt=%.4f",
            player_id, tostring(input.digital.back.just_pressed), dt))
    end
end

--- Called when entering the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_running_on_enter(ctx, input)
    log_debug(string.format(
        "player_running_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s left=%s right=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y,
        tostring(ctx.previous_phase),
        tostring(input.digital.left.pressed), tostring(input.digital.right.pressed)))
    engine.entity_signal_set_flag(ctx.id, "running")
    if input.digital.left.pressed and not input.digital.right.pressed then
        set_hvel(ctx, -running_speed, 0)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        set_hvel(ctx, running_speed, 0)
    end
end

--- Called each frame while in the running phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_running_on_update(ctx, input, dt)
    local on_ground = utils.has_flag(ctx.signals.flags, "on_ground")
    log_debug(string.format(
        "player_running_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s left=%s right=%s action_1=%s action_2=%s action_3=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(on_ground),
        tostring(input.digital.left.pressed), tostring(input.digital.right.pressed),
        tostring(input.digital.action_1.just_pressed), tostring(input.digital.action_2.just_pressed),
        tostring(input.digital.action_3.pressed), dt))
    -- Transition to falling if we walked off a ledge
    if not on_ground and ctx.vel.y >= 0 then
        log_debug("player_running_on_update: walked off ledge -> falling")
        return "falling"
    end
    -- Check for attack input
    if input.digital.action_1.just_pressed then
        log_debug("player_running_on_update: action_1 -> attack")
        return "attack"
    end
    -- Check for jump input
    if input.digital.action_2.just_pressed and not input.digital.down.pressed then
        log_debug(string.format("player_running_on_update: jump impulse vel_y=%.1f -> jumping", jump_speed))
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end
    -- Check for directional input; if no directional input, transition to idle
    if input.digital.left.pressed or input.digital.right.pressed then
        -- Keep running (or switch to walking if action_3 held)
        update_facing_direction(ctx.id, input)
        local has_single_direction = not (input.digital.left.pressed and input.digital.right.pressed)
        if has_single_direction and input.digital.action_3.pressed then
            log_debug("player_running_on_update: action_3 held -> walking")
            return "walking"
        end
        if input.digital.left.pressed and not input.digital.right.pressed then
            set_hvel(ctx, -running_speed, 0)
        elseif input.digital.right.pressed and not input.digital.left.pressed then
            set_hvel(ctx, running_speed, 0)
        end
    else
        log_debug("player_running_on_update: no direction -> idle")
        return "idle"
    end
end

--- Called when exiting the running phase.
--- @param ctx EntityContext Entity state
local function player_running_on_exit(ctx)
    log_debug(string.format("player_running_on_exit id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f)",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y))
    engine.entity_signal_clear_flag(ctx.id, "running")
end

--- Called when entering the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_walking_on_enter(ctx, input)
    log_debug(string.format(
        "player_walking_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s left=%s right=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y,
        tostring(ctx.previous_phase),
        tostring(input.digital.left.pressed), tostring(input.digital.right.pressed)))
    engine.entity_signal_set_flag(ctx.id, "walking")
    update_facing_direction(ctx.id, input)
    if input.digital.left.pressed and not input.digital.right.pressed then
        set_hvel(ctx, -walking_speed, 0)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        set_hvel(ctx, walking_speed, 0)
    end
end

--- Called each frame while in the walking phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_walking_on_update(ctx, input, dt)
    local on_ground = utils.has_flag(ctx.signals.flags, "on_ground")
    local has_direction = (input.digital.left.pressed or input.digital.right.pressed)
        and not (input.digital.left.pressed and input.digital.right.pressed)
    log_debug(string.format(
        "player_walking_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s has_direction=%s action_3=%s action_1=%s action_2=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(on_ground),
        tostring(has_direction), tostring(input.digital.action_3.pressed),
        tostring(input.digital.action_1.just_pressed), tostring(input.digital.action_2.just_pressed), dt))
    if not on_ground and ctx.vel.y >= 0 then
        log_debug("player_walking_on_update: off ledge -> falling")
        return "falling"
    end
    if input.digital.action_1.just_pressed then
        log_debug("player_walking_on_update: action_1 -> attack")
        return "attack"
    end
    if input.digital.action_2.just_pressed then
        log_debug(string.format("player_walking_on_update: jump impulse vel_y=%.1f -> jumping", jump_speed))
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end

    if not has_direction then
        log_debug("player_walking_on_update: no direction -> idle")
        return "idle"
    end

    update_facing_direction(ctx.id, input)

    if not input.digital.action_3.pressed then
        log_debug("player_walking_on_update: action_3 released -> running")
        return "running"
    end

    if input.digital.left.pressed then
        set_hvel(ctx, -walking_speed, 0)
    else
        set_hvel(ctx, walking_speed, 0)
    end
end

--- Called when exiting the walking phase.
--- @param ctx EntityContext Entity state
local function player_walking_on_exit(ctx)
    log_debug(string.format("player_walking_on_exit id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f)",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y))
    engine.entity_signal_clear_flag(ctx.id, "walking")
end

--- Called when entering the falling phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_falling_on_enter(ctx, input)
    log_debug(string.format(
        "player_falling_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(ctx.previous_phase)))
    engine.entity_signal_set_flag(ctx.id, "falling")
end

--- Called each frame while in the falling phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_falling_on_update(ctx, input, dt)
    local on_ground = utils.has_flag(ctx.signals.flags, "on_ground")
    log_debug(string.format(
        "player_falling_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s left=%s right=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(on_ground),
        tostring(input.digital.left.pressed), tostring(input.digital.right.pressed), dt))
    if on_ground then
        log_debug("player_falling_on_update: on_ground -> idle")
        return "idle"
    end
    if input.digital.left.pressed and not input.digital.right.pressed then
        update_facing_direction(ctx.id, input)
        set_hvel(ctx, -running_speed, ctx.vel.y)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        update_facing_direction(ctx.id, input)
        set_hvel(ctx, running_speed, ctx.vel.y)
    elseif not input.digital.right.pressed and not input.digital.left.pressed then
        log_debug(string.format("player_falling_on_update: no direction, zeroing vx (vy=%.1f)", ctx.vel.y))
        engine.entity_set_velocity(ctx.id, 0, ctx.vel.y)
    end
end

--- Called when exiting the falling phase.
--- @param ctx EntityContext Entity state
local function player_falling_on_exit(ctx)
    log_debug(string.format("player_falling_on_exit id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f)",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y))
    engine.entity_signal_clear_flag(ctx.id, "falling")
end

--- Called when entering the jumping phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_jumping_on_enter(ctx, input)
    log_debug(string.format(
        "player_jumping_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(ctx.previous_phase)))
    engine.entity_signal_set_flag(ctx.id, "jumping")
end

--- Called each frame while in the jumping phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_jumping_on_update(ctx, input, dt)
    local on_ground = utils.has_flag(ctx.signals.flags, "on_ground")
    log_debug(string.format(
        "player_jumping_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s left=%s right=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(on_ground),
        tostring(input.digital.left.pressed), tostring(input.digital.right.pressed), dt))
    if on_ground then
        log_debug("player_jumping_on_update: on_ground -> idle")
        return "idle"
    end
    if ctx.vel.y > 0 then
        log_debug(string.format("player_jumping_on_update: apex passed vel_y=%.1f -> falling", ctx.vel.y))
        return "falling"
    end
    if input.digital.left.pressed and not input.digital.right.pressed then
        update_facing_direction(ctx.id, input)
        set_hvel(ctx, -running_speed, ctx.vel.y)
    elseif input.digital.right.pressed and not input.digital.left.pressed then
        update_facing_direction(ctx.id, input)
        set_hvel(ctx, running_speed, ctx.vel.y)
    elseif not input.digital.right.pressed and not input.digital.left.pressed then
        log_debug(string.format("player_jumping_on_update: no direction, zeroing vx (vy=%.1f)", ctx.vel.y))
        engine.entity_set_velocity(ctx.id, 0, ctx.vel.y)
    end
end

--- Called when exiting the jumping phase.
--- @param ctx EntityContext Entity state
local function player_jumping_on_exit(ctx)
    log_debug(string.format("player_jumping_on_exit id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f)",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y))
    engine.entity_signal_clear_flag(ctx.id, "jumping")
end

--- Called when entering the idle phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_idle_on_enter(ctx, input)
    log_debug(string.format(
        "player_idle_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(ctx.previous_phase)))
    -- engine.entity_set_animation(ctx.id, "sidescroller-char_red_idle")
    engine.entity_set_velocity(ctx.id, 0, ctx.vel.y)
end

--- Called each frame while in the idle phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function player_idle_on_update(ctx, input, dt)
    local flags = ctx.signals.flags
    local on_ground = utils.has_flag(flags, "on_ground")
    local has_direction = (input.digital.left.pressed or input.digital.right.pressed)
        and not (input.digital.left.pressed and input.digital.right.pressed)
    log_debug(string.format(
        "player_idle_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s has_direction=%s action_1=%s action_2=%s action_3=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(on_ground),
        tostring(has_direction), tostring(input.digital.action_1.just_pressed),
        tostring(input.digital.action_2.just_pressed), tostring(input.digital.action_3.pressed), dt))
    if not on_ground and ctx.vel.y >= 0 then
        log_debug("player_idle_on_update: off ledge -> falling")
        return "falling"
    end

    if input.digital.action_1.just_pressed then
        log_debug("player_idle_on_update: action_1 -> attack")
        return "attack"
    end

    if input.digital.action_2.just_pressed then
        log_debug(string.format("player_idle_on_update: jump impulse vel_y=%.1f -> jumping", jump_speed))
        engine.entity_set_velocity(ctx.id, ctx.vel.x, jump_speed)
        return "jumping"
    end

    if has_direction then
        update_facing_direction(ctx.id, input)
        if input.digital.action_3.pressed then
            log_debug("player_idle_on_update: direction + action_3 -> walking")
            return "walking"
        else
            log_debug("player_idle_on_update: direction -> running")
            return "running"
        end
    end
end

--- Called when entering the attack phase.
--- @param ctx EntityContext Entity state
--- @param input InputSnapshot Input state table
local function player_attack_on_enter(ctx, input)
    local facing_left = utils.has_flag(ctx.signals.flags, "facing_left")
    local facing_right = utils.has_flag(ctx.signals.flags, "facing_right")
    log_debug(string.format(
        "player_attack_on_enter id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) prev=%s facing_left=%s facing_right=%s",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y, tostring(ctx.previous_phase),
        tostring(facing_left), tostring(facing_right)))
    engine.entity_signal_set_flag(ctx.id, "attack")
    -- engine.entity_freeze(ctx.id)
    -- engine.entity_restart_animation(ctx.id)
    engine.entity_set_velocity(ctx.id, 0, 0)
    -- Spawn hitbox child entity in front of the player
    local offset_x = 20
    if facing_left then
        offset_x = -20
    end
    log_debug(string.format("player_attack_on_enter: hitbox offset_x=%d", offset_x))
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
    local on_ground = utils.has_flag(ctx.signals.flags, "on_ground")
    local anim_ended = utils.has_flag(ctx.signals.flags, "animation_ended")
    log_debug(string.format(
        "player_attack_on_update id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f) on_ground=%s anim_ended=%s dt=%.4f",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y,
        tostring(on_ground), tostring(anim_ended), dt))
    if not on_ground and ctx.vel.y >= 0 then
        log_debug("player_attack_on_update: off ground during attack -> falling")
        return "falling"
    end
    if anim_ended then
        log_debug("player_attack_on_update: animation_ended -> idle")
        engine.entity_signal_clear_flag(ctx.id, "animation_ended")
        return "idle"
    end
end

--- Called when exiting the attack phase.
--- @param ctx EntityContext Entity state
local function player_attack_on_exit(ctx)
    log_debug(string.format("player_attack_on_exit id=%d pos=(%.1f,%.1f) vel=(%.1f,%.1f)",
        ctx.id, ctx.pos.x, ctx.pos.y, ctx.vel.x, ctx.vel.y))
    engine.entity_signal_clear_flag(ctx.id, "attack")
    -- engine.entity_unfreeze(ctx.id)
    -- Despawn the hitbox child
    local hitbox_id = engine.get_entity("attack_hitbox")
    if hitbox_id then
        log_debug(string.format("player_attack_on_exit: despawning hitbox_id=%d", hitbox_id))
        engine.entity_despawn(hitbox_id)
        engine.remove_entity("attack_hitbox")
    else
        log_debug("player_attack_on_exit: attack_hitbox not found (already despawned?)")
    end
end

--- Collision solid/player callback.
--- @param ctx CollisionContext
local function collision_solid_player(ctx)
    -- entity ctx.a is solid, entity ctx.b is player
    local sides_a = table.concat(ctx.sides.a, ",")
    local sides_b = table.concat(ctx.sides.b, ",")
    log_debug(string.format(
        "collision_solid_player solid_id=%d pos_a=(%.1f,%.1f) rect_a=(%.1f,%.1f,%.1f,%.1f) sides_a=[%s] | player_id=%d pos_b=(%.1f,%.1f) vel_b=(%.1f,%.1f) rect_b=(%.1f,%.1f,%.1f,%.1f) sides_b=[%s]",
        ctx.a.id, ctx.a.pos.x, ctx.a.pos.y, ctx.a.rect.x, ctx.a.rect.y, ctx.a.rect.w, ctx.a.rect.h, sides_a,
        ctx.b.id, ctx.b.pos.x, ctx.b.pos.y, ctx.b.vel.x, ctx.b.vel.y,
        ctx.b.rect.x, ctx.b.rect.y, ctx.b.rect.w, ctx.b.rect.h, sides_b))

    local player_on_ground = false
    local player_touching_ceiling = false
    local player_touching_wall_left = false
    local player_touching_wall_right = false

    if utils.has_flag(ctx.sides.b, "bottom") and utils.has_flag(ctx.sides.a, "top") then
        player_on_ground = true
    end

    if utils.has_flag(ctx.sides.b, "top") and utils.has_flag(ctx.sides.a, "bottom") then
        player_touching_ceiling = true
    end

    if utils.has_flag(ctx.sides.b, "left") and utils.has_flag(ctx.sides.a, "right") then
        player_touching_wall_left = true
    end

    if utils.has_flag(ctx.sides.b, "right") and utils.has_flag(ctx.sides.a, "left") then
        player_touching_wall_right = true
    end

    log_debug(string.format(
        "collision_solid_player detected: on_ground=%s ceiling=%s wall_left=%s wall_right=%s",
        tostring(player_on_ground), tostring(player_touching_ceiling),
        tostring(player_touching_wall_left), tostring(player_touching_wall_right)))

    if player_touching_ceiling then
        local clamped_vy = math.min(ctx.b.vel.y, 0)
        log_debug(string.format("collision_solid_player: ceiling clamp vel_y %.1f -> %.1f",
            ctx.b.vel.y, clamped_vy))
        engine.collision_entity_set_velocity(ctx.b.id, ctx.b.vel.x, clamped_vy)
    end

    if (player_touching_wall_left or player_touching_wall_right) and not player_on_ground then
        if player_touching_wall_left then
            local snap_x = ctx.b.pos.x + (ctx.a.rect.x + ctx.a.rect.w - ctx.b.rect.x)
            apply_wall_stop(ctx, "touching_wall_left", snap_x)
        end
        if player_touching_wall_right then
            local snap_x = ctx.b.pos.x - (ctx.b.rect.x + ctx.b.rect.w - ctx.a.rect.x)
            apply_wall_stop(ctx, "touching_wall_right", snap_x)
        end
    end

    if player_on_ground then
        local solid_rect = ctx.a.rect
        local snap_y = solid_rect.y
        log_debug(string.format(
            "collision_solid_player: ground snap pos_y %.1f -> %.1f vel_y %.1f -> 0",
            ctx.b.pos.y, snap_y, ctx.b.vel.y))
        engine.collision_entity_signal_set_flag(ctx.b.id, "on_ground")
        engine.collision_entity_signal_clear_flag(ctx.b.id, "falling")
        engine.collision_entity_signal_clear_flag(ctx.b.id, "jumping")
        local vel = ctx.b.vel
        engine.collision_entity_set_velocity(ctx.b.id, vel.x, 0)
        engine.collision_entity_set_position(ctx.b.id, ctx.b.pos.x, snap_y)
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
    collision_solid_player = collision_solid_player,
}

-- ─── Spawn ──────────────────────────────────────────────────────────────────

--- Spawn all entities for the sidescroller level01 scene.
function M.spawn()
    engine.log_debug("Spawning sidescroller level01 scene...")
    engine.stop_all_music()

    -- debug_log = true
    log_debug("Debug logging enabled for sidescroller level01.")

    -- Set render resolution
    engine.set_render_size(320, 180)

    -- Set camera
    engine.set_camera(0, 0, 320 / 2, 180 / 2, 0.0, 1.0) -- target to 0,0, centered, no rotation, default zoom
    engine.camera_follow_enable(true)
    engine.camera_follow_set_mode("lerp")
    engine.camera_follow_set_easing("linear")
    engine.camera_follow_set_speed(6.0)
    engine.camera_follow_set_offset(0, -48)
    engine.set_vsync(true)
    engine.set_target_fps(120)

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
        :with_camera_target()
        :build()

    -- Spawn collision rules for solid-player
    engine.spawn()
        :with_group("collision_rules")
        :with_lua_collision_rule("solids", "player", "collision_solid_player")
        :build()

    -- Spawn background layers (below all world geometry)
    -- Positioned each frame in on_update via get_camera_view_rect()
    engine.spawn()
        :with_sprite("back_layer01", 320, 180, 0, 0)
        :with_position(0, 0)
        :with_zindex(-100)
        :register_as("bg_layer01")
        :build()
    engine.spawn()
        :with_sprite("back_layer02", 320, 180, 0, 0)
        :with_position(0, 0)
        :with_zindex(-99)
        :register_as("bg_layer02")
        :build()
    engine.spawn()
        :with_sprite("back_layer03", 320, 180, 0, 0)
        :with_position(0, 0)
        :with_zindex(-98)
        :register_as("bg_layer03")
        :build()

    engine.load_map("./assets/tilemaps/sidescroller_test01/sidescroller01_assets.map")

    engine.log_debug("Sidescroller level01 scene entities queued!")
end

return M
