-- scenes/shapes_showcase.lua
-- Shapes showcase: every Shape kind, transforms, hierarchy, tint tweens, runtime edits,
-- z-order and screen space.
--
-- Seven pages sit side by side in the world, PAGE_W apart. An anchor entity carries the camera
-- target; left/right moves the anchor to the next page and the engine's camera follow glides
-- there. Back returns to the menu. The screen-space page spawns its entities once the camera
-- arrives and despawns them when it is left, because screen shapes ignore the camera and would
-- otherwise draw on every page.

local M = {}

local SCREEN_W, SCREEN_H = 640, 360
local PAGE_W = 800     -- world distance between page centres (wider than the screen, so a neighbour never peeks in)
local FOLLOW_SPEED = 14    -- camera follow rate: about 95% of the way in 0.2 s
local ARRIVED_PX = 4       -- the camera counts as arrived within this many pixels of a page

local TITLES = {
    "Kinds: outline, filled, both",
    "Transforms: origin, rotation, scale",
    "Hierarchy: children inherit transforms",
    "Tint tweens and shadows",
    "Runtime edits from on_update",
    "Z-order: shapes, sprite and text",
    "Screen space: ignores the camera",
}
local PAGE_COUNT = #TITLES
local SCREEN_PAGE = PAGE_COUNT -- the last page spawns screen-space entities

-- Page state, reset by M.spawn().
local page
local elapsed
local last_blink
local last_sides
local last_bar_width
local last_fill_width
local screen_spawned -- whether the screen-space page's entities exist

-- ─── Small helpers ──────────────────────────────────────────────────────────

--- World x of the centre of page `i` (1-based).
local function page_x(i)
    return (i - 1) * PAGE_W
end

--- Left-aligned world text.
local function text(content, x, y, r, g, b)
    engine.spawn()
        :with_text(content, "default", 14, r or 200, g or 200, b or 200, 255)
        :with_position(x, y)
        :with_z_index(20)
        :build()
end

--- World text centred on `x` (the default font is about 0.6 em per glyph).
local function label(content, x, y, r, g, b)
    text(content, x - #content * 14 * 0.3, y, r, g, b)
end

--- Points of a star: alternating outer and inner radius, first point up.
local function star(outer, inner)
    local points = {}
    for i = 0, 9 do
        local radius = (i % 2 == 0) and outer or inner
        local angle = math.rad(-90 + i * 36)
        points[#points + 1] = { x = math.cos(angle) * radius, y = math.sin(angle) * radius }
    end
    return points
end

--- Points of a regular polygon with `sides` corners.
local function ngon(sides, radius)
    local points = {}
    for i = 0, sides - 1 do
        local angle = math.rad(-90 + i * 360 / sides)
        points[#points + 1] = { x = math.cos(angle) * radius, y = math.sin(angle) * radius }
    end
    return points
end

--- Full-saturation colour for a hue in [0, 1).
local function hsv(hue)
    local sector = math.floor(hue * 6) % 6
    local fraction = hue * 6 - math.floor(hue * 6)
    local rising = math.floor(255 * fraction)
    local falling = math.floor(255 * (1 - fraction))
    if sector == 0 then return 255, rising, 0
    elseif sector == 1 then return falling, 255, 0
    elseif sector == 2 then return 0, 255, rising
    elseif sector == 3 then return 0, falling, 255
    elseif sector == 4 then return rising, 0, 255
    else return 255, 0, falling end
end

--- Adds a continuous rotation tween (a Rotation component is required for it to act on).
local function spin(builder, seconds)
    return builder
        :with_rotation(0)
        :with_tween_rotation(0, 360, seconds)
        :with_tween_rotation_loop("loop")
end

--- A small marker on a pivot point.
local function pivot(x, y)
    engine.spawn()
        :with_shape_circle(2, 255, 80, 80, 255)
        :with_shape_filled()
        :with_position(x, y)
        :with_z_index(5)
        :build()
end

local STAR = star(22, 9)
local TRIANGLE = { { x = 0, y = 0 }, { x = 34, y = 0 }, { x = 0, y = 34 } }

--- Starts a shape of `kind` on `builder`, sized for the gallery cells.
local function with_kind(builder, kind, r, g, b)
    if kind == "point" then return builder:with_shape_point(r, g, b, 255)
    elseif kind == "line" then return builder:with_shape_line(36, 18, r, g, b, 255)
    elseif kind == "rect" then return builder:with_shape_rect(36, 36, r, g, b, 255)
    elseif kind == "circle" then return builder:with_shape_circle(18, r, g, b, 255)
    elseif kind == "ellipse" then return builder:with_shape_ellipse(22, 13, r, g, b, 255)
    else return builder:with_shape_polygon(STAR, r, g, b, 255) end
end

-- ─── Pages ──────────────────────────────────────────────────────────────────

--- Page 1: every kind as an outline, filled, and filled with an outline colour.
local function build_gallery(x0)
    local kinds = { "point", "line", "rect", "circle", "ellipse", "polygon" }
    local colors = {
        { 255, 255, 255 }, { 255, 160, 60 }, { 90, 200, 255 },
        { 120, 230, 120 }, { 255, 100, 160 }, { 255, 220, 70 },
    }
    local centred = { line = { 18, 9 }, rect = { 18, 18 } } -- local pivot that centres the kind on its cell
    local rows = { "outline", "filled", "both" }

    for row, name in ipairs(rows) do
        text(name, x0 - 312, -78 + (row - 1) * 70, 150, 150, 150)
    end
    for col, kind in ipairs(kinds) do
        local c = colors[col]
        local x = x0 - 225 + (col - 1) * 90 + 20
        label(kind, x, -118, 150, 150, 150)
        for row = 1, 3 do
            local builder = with_kind(engine.spawn(), kind, c[1], c[2], c[3])
            if row >= 2 then builder = builder:with_shape_filled() end
            if row == 3 then builder = builder:with_shape_outline_color(255, 255, 255, 255) end
            if centred[kind] then builder = builder:with_shape_origin(centred[kind][1], centred[kind][2]) end
            builder:with_position(x, -70 + (row - 1) * 70):with_z_index(1):build()
        end
    end
    text("the point is a single pixel", x0 - 310, 150)
end

--- Page 2: origin as the pivot of rotation and scale, uneven scale, mirroring.
local function build_transforms(x0)
    local cols = { -240, -80, 80, 240 }
    local top, bottom = -50, 70

    -- 1: rect pivoting on its centre
    spin(engine.spawn():with_shape_rect(40, 40, 90, 200, 255, 255):with_shape_filled()
        :with_shape_origin(20, 20), 4)
        :with_position(x0 + cols[1], top):with_z_index(1):build()
    pivot(x0 + cols[1], top)
    label("centre origin", x0 + cols[1], top + 40)

    -- 2: the same rect pivoting on its corner
    spin(engine.spawn():with_shape_rect(40, 40, 255, 160, 60, 255):with_shape_filled(), 4)
        :with_position(x0 + cols[2], top):with_z_index(1):build()
    pivot(x0 + cols[2], top)
    label("corner origin", x0 + cols[2], top + 40)

    -- 3: a circle rotates only its centre about the origin
    spin(engine.spawn():with_shape_circle(10, 120, 230, 120, 255):with_shape_filled()
        :with_shape_origin(-30, 0), 3)
        :with_position(x0 + cols[3], top):with_z_index(1):build()
    pivot(x0 + cols[3], top)
    label("orbits origin", x0 + cols[3], top + 40)

    -- 4: an ellipse ignores rotation entirely
    spin(engine.spawn():with_shape_ellipse(28, 12, 255, 100, 160, 255):with_shape_filled(), 3)
        :with_position(x0 + cols[4], top):with_z_index(1):build()
    label("ellipse no spin", x0 + cols[4], top + 40)

    -- 5: a circle under uneven scale becomes an ellipse
    engine.spawn()
        :with_shape_circle(14, 90, 200, 255, 255):with_shape_filled()
        :with_scale(1, 1)
        :with_tween_scale(1, 1, 2.2, 0.6, 1.5)
        :with_tween_scale_easing("quad_in_out")
        :with_tween_scale_loop("ping_pong")
        :with_position(x0 + cols[1], bottom):with_z_index(1):build()
    label("circle->ellipse", x0 + cols[1], bottom + 40)

    -- 6: a polygon scales around its origin
    engine.spawn()
        :with_shape_polygon(STAR, 255, 220, 70, 255):with_shape_filled()
        :with_scale(1, 1)
        :with_tween_scale(0.5, 0.5, 1.5, 1.5, 1.2)
        :with_tween_scale_loop("ping_pong")
        :with_position(x0 + cols[2], bottom):with_z_index(1):build()
    label("scale tween", x0 + cols[2], bottom + 40)

    -- 7: a negative scale mirrors a filled polygon (it must stay visible)
    engine.spawn()
        :with_shape_polygon(TRIANGLE, 255, 160, 60, 255):with_shape_filled()
        :with_position(x0 + cols[3] - 40, bottom - 17):with_z_index(1):build()
    engine.spawn()
        :with_shape_polygon(TRIANGLE, 90, 200, 255, 255):with_shape_filled()
        :with_scale(-1, 1)
        :with_position(x0 + cols[3] + 40, bottom - 17):with_z_index(1):build()
    label("scale -1, 1", x0 + cols[3], bottom + 40)

    -- 8: rotation and scale together
    spin(engine.spawn():with_shape_rect(30, 30, 255, 255, 255, 255):with_shape_filled()
        :with_shape_outline_color(255, 100, 160, 255):with_shape_origin(15, 15), 3)
        :with_scale(1, 1)
        :with_tween_scale(0.6, 0.6, 1.4, 1.4, 1.5)
        :with_tween_scale_easing("quad_in_out")
        :with_tween_scale_loop("ping_pong")
        :with_position(x0 + cols[4], bottom):with_z_index(1):build()
    label("spin + scale", x0 + cols[4], bottom + 40)
end

--- Phase callback: the sun exists, so the planet can be parented to it.
function M.on_sun_enter(ctx, input)
    engine.spawn()
        :with_shape_rect(18, 18, 90, 200, 255, 255):with_shape_filled()
        :with_shape_outline_color(255, 255, 255, 255):with_shape_origin(9, 9)
        :with_parent(ctx.id)
        :with_position(95, 0)
        :with_rotation(0)
        :with_tween_rotation(0, 360, 4)
        :with_tween_rotation_loop("loop")
        :with_z_index(2)
        :with_phase("grow")
        :with_lua_phase_entered("on_planet_enter")
        :build()
end

--- Phase callback: the planet exists, so the moon can be parented to it.
function M.on_planet_enter(ctx, input)
    engine.spawn()
        :with_shape_circle(5, 230, 230, 230, 255):with_shape_filled()
        :with_parent(ctx.id)
        :with_position(30, 0)
        :with_z_index(3)
        :build()
end

--- Page 3: parent, child and grandchild shapes, each with its own rotation tween.
local function build_hierarchy(x0)
    engine.spawn()
        :with_shape_polygon(star(34, 20), 255, 200, 60, 255):with_shape_filled()
        :with_position(x0, 0)
        :with_rotation(0)
        :with_tween_rotation(0, 360, 16)
        :with_tween_rotation_loop("loop")
        :with_z_index(1)
        :with_phase("build")
        :with_lua_phase_entered("on_sun_enter")
        :build()
    text("the planet turns with the sun and on itself;", x0 - 310, 130)
    text("the moon follows the planet", x0 - 310, 150)
end

--- Page 4: white shapes whose Tint tweens, in the top row plain and in the bottom row shadowed.
local function build_tint(x0)
    local xs = { -200, -100, 0, 100, 200 }
    local easings = { "linear", "quad_in_out", "cubic_in", "quad_out", "cubic_in_out" }
    local ends = {
        { 255, 80, 80 }, { 255, 200, 60 }, { 120, 230, 120 }, { 90, 200, 255 }, { 220, 120, 255 },
    }
    local function kind(builder, i)
        if i == 1 then return builder:with_shape_circle(22, 255, 255, 255, 255)
        elseif i == 2 then return builder:with_shape_rect(40, 40, 255, 255, 255, 255):with_shape_origin(20, 20)
        elseif i == 3 then return builder:with_shape_ellipse(28, 16, 255, 255, 255, 255)
        elseif i == 4 then return builder:with_shape_polygon(STAR, 255, 255, 255, 255)
        else return builder:with_shape_polygon(ngon(6, 24), 255, 255, 255, 255) end
    end
    for row, y in ipairs({ -50, 70 }) do
        for i, x in ipairs(xs) do
            local e = ends[i]
            local builder = kind(engine.spawn(), i)
                :with_shape_filled()
                :with_tween_tint(255, 255, 255, 255, e[1], e[2], e[3], 255, 1.4)
                :with_tween_tint_easing(easings[i])
                :with_tween_tint_loop("ping_pong")
                :with_position(x0 + x, y)
                :with_z_index(1)
            if row == 2 then builder = builder:with_shadow(5, 5, 0, 0, 0, 170) end
            builder:build()
        end
    end
    label("Tint tween: white to colour, five easings", x0, -5)
    label("the same with a Shadow", x0, 115)
end

--- Page 5: shapes whose geometry and colour change every tick (see `update_runtime`).
local function build_runtime(x0)
    local xs = { -240, -120, 0, 120, 240 }
    local y = -20

    engine.spawn():with_shape_circle(14, 255, 160, 60, 255):with_shape_filled()
        :with_position(x0 + xs[1], y):with_z_index(1):register_as("rt_pulse"):build()
    label("set_shape_circle", x0 + xs[1], y + 50)

    engine.spawn():with_shape_rect(1, 14, 90, 200, 255, 255):with_shape_filled()
        :with_position(x0 + xs[2] - 40, y - 7):with_z_index(1):register_as("rt_bar"):build()
    label("set_shape_rect", x0 + xs[2], y + 50)

    engine.spawn():with_shape_rect(40, 40, 120, 230, 120, 255):with_shape_filled()
        :with_shape_outline_color(255, 255, 255, 255):with_shape_origin(20, 20)
        :with_position(x0 + xs[3], y):with_z_index(1):register_as("rt_blink"):build()
    label("set_shape_filled", x0 + xs[3], y + 50)

    engine.spawn():with_shape_polygon(ngon(3, 28), 255, 100, 160, 255):with_shape_filled()
        :with_position(x0 + xs[4], y):with_z_index(1):register_as("rt_morph"):build()
    label("set_shape_polygon", x0 + xs[4], y + 50)

    engine.spawn():with_shape_circle(22, 255, 0, 0, 255):with_shape_filled()
        :with_position(x0 + xs[5], y):with_z_index(1):register_as("rt_hue"):build()
    label("set_shape_color", x0 + xs[5], y + 50)

    text("each one calls engine.entity_set_shape_*", x0 - 310, 130)
    text("from on_update; the polygon is built per change", x0 - 310, 150)
end

--- Page 6: a sprite between shapes and texts at different ZIndex values.
local function build_zorder(x0)
    -- text under a shape
    engine.spawn():with_text("BEHIND z2", "default", 14, 255, 255, 255, 255)
        :with_position(x0 - 100, -56):with_z_index(2):build()
    engine.spawn():with_shape_rect(120, 26, 60, 160, 255, 255):with_shape_filled()
        :with_position(x0 - 120, -60):with_z_index(4):build()

    -- a filled shape behind the sprite, an outline in front of it
    engine.spawn():with_shape_circle(34, 255, 60, 60, 160):with_shape_filled()
        :with_position(x0 + 25, 25):with_z_index(4):build()
    engine.spawn():with_sprite("bunnymark-raybunny", 32, 32, 16, 16)
        :with_scale(3, 3):with_position(x0, 20):with_z_index(5):build()
    engine.spawn():with_shape_circle(44, 255, 220, 60, 255)
        :with_position(x0, 20):with_z_index(6):build()

    -- text over a shape
    engine.spawn():with_shape_rect(150, 26, 40, 200, 120, 255):with_shape_filled()
        :with_position(x0 - 190, 70):with_z_index(7):build()
    engine.spawn():with_text("IN FRONT z8", "default", 14, 255, 255, 255, 255)
        :with_position(x0 - 180, 76):with_z_index(8):build()

    text("one sort across shapes, sprites and text", x0 - 310, 150)
end

--- Screen-space entities of the last page, keyed so they can be despawned on leaving.
local SCREEN_KEYS = { "ss_frame", "ss_fill", "ss_hline", "ss_vline", "ss_ring", "ss_note" }

local function spawn_screen_page()
    engine.spawn():with_shape_rect(200, 16, 255, 255, 255, 255)
        :with_screen_position(220, 300):with_z_index(15):register_as("ss_frame"):build()
    engine.spawn():with_shape_rect(1, 12, 120, 230, 120, 255):with_shape_filled()
        :with_screen_position(222, 302):with_z_index(15):register_as("ss_fill"):build()
    engine.spawn():with_shape_line(24, 0, 255, 255, 255, 255):with_shape_origin(12, 0)
        :with_screen_position(320, 180):with_z_index(15):register_as("ss_hline"):build()
    engine.spawn():with_shape_line(0, 24, 255, 255, 255, 255):with_shape_origin(0, 12)
        :with_screen_position(320, 180):with_z_index(15):register_as("ss_vline"):build()
    engine.spawn():with_shape_circle(20, 255, 160, 60, 255)
        :with_screen_position(320, 180):with_z_index(15):register_as("ss_ring"):build()
    engine.spawn():with_text("screen shapes do not move with the camera", "default", 14, 200, 200, 200, 255)
        :with_screen_position(176, 330):with_z_index(15):register_as("ss_note"):build()
end

local function despawn_screen_page()
    for _, key in ipairs(SCREEN_KEYS) do
        local id = engine.get_entity(key)
        if id then
            engine.entity_despawn(id)
        end
    end
end

-- ─── Paging and per-tick updates ────────────────────────────────────────────

--- Moves the camera and its follow state to `x` at once.
local function snap_camera(x)
    engine.set_camera(x, 0, SCREEN_W / 2, SCREEN_H / 2, 0.0, 1.0)
    engine.camera_follow_reset_velocity()
end

--- Sends the camera anchor to `new_page`; the camera follow glides after it, and a press
--- mid-flight just retargets. Wrapping between the first and last page jumps, because a glide
--- would drag every page across the screen in a fraction of a second.
local function go_to(new_page)
    local anchor = engine.get_entity("showcase_camera")
    if not anchor then
        return -- the anchor spawns on the first tick
    end
    if page == SCREEN_PAGE then
        despawn_screen_page()
        screen_spawned = false
    end
    local from = page_x(page)
    page = new_page
    engine.set_integer("showcase_page", page)
    engine.entity_set_position(anchor, page_x(page), 0)
    if math.abs(page_x(page) - from) > PAGE_W * 1.5 then
        snap_camera(page_x(page))
    end
end

--- Spawns the screen-space page once the camera has arrived, so its entities never overlay
--- the page being left.
local function update_screen_page_arrival()
    if page ~= SCREEN_PAGE or screen_spawned then
        return
    end
    if math.abs(engine.get_camera().target_x - page_x(page)) <= ARRIVED_PX then
        screen_spawned = true
        spawn_screen_page()
    end
end

--- Page 5: edit the registered shapes through the runtime API.
local function update_runtime()
    local pulse = engine.get_entity("rt_pulse")
    if pulse then
        engine.entity_set_shape_circle(pulse, 14 + 8 * math.sin(elapsed * 4))
    end
    local bar = engine.get_entity("rt_bar")
    if bar then
        local width = 1 + math.floor(78 * ((elapsed * 0.4) % 1))
        if width ~= last_bar_width then
            last_bar_width = width
            engine.entity_set_shape_rect(bar, width, 14)
        end
    end
    local blink = engine.get_entity("rt_blink")
    if blink then
        local filled = math.floor(elapsed * 2) % 2 == 0
        if filled ~= last_blink then
            last_blink = filled
            engine.entity_set_shape_filled(blink, filled)
        end
    end
    local morph = engine.get_entity("rt_morph")
    if morph then
        local sides = 3 + math.floor(elapsed / 0.6) % 6
        if sides ~= last_sides then
            last_sides = sides
            engine.entity_set_shape_polygon(morph, ngon(sides, 28))
        end
    end
    local hue = engine.get_entity("rt_hue")
    if hue then
        local r, g, b = hsv((elapsed * 0.25) % 1)
        engine.entity_set_shape_color(hue, r, g, b, 255)
    end
end

--- Page 7: grow the progress fill.
local function update_screen()
    local fill = engine.get_entity("ss_fill")
    if fill then
        local width = 1 + math.floor(195 * ((elapsed * 0.25) % 1))
        if width ~= last_fill_width then
            last_fill_width = width
            engine.entity_set_shape_rect(fill, width, 12)
        end
    end
end

--- Called each tick while the scene is active.
--- @param input InputSnapshot Input state table
--- @param dt number Delta time in seconds
local function on_update_shapes_showcase(input, dt)
    if input.digital.back.just_pressed then
        engine.request_scene("menu")
        return
    end
    elapsed = elapsed + dt
    if input.digital.right.just_pressed then
        go_to(page % PAGE_COUNT + 1)
    elseif input.digital.left.just_pressed then
        go_to((page - 2) % PAGE_COUNT + 1)
    end
    update_screen_page_arrival()
    if page == 5 then
        update_runtime()
    elseif page == SCREEN_PAGE then
        update_screen()
    end
end

M._callbacks = {
    on_update_shapes_showcase = on_update_shapes_showcase,
    on_sun_enter = M.on_sun_enter,
    on_planet_enter = M.on_planet_enter,
}

function M.spawn()
    engine.log_debug("Spawning shapes showcase")
    engine.stop_all_music()
    engine.set_render_size(SCREEN_W, SCREEN_H)
    engine.set_background_color(40, 44, 52)
    engine.post_process_shader(nil)

    page = 1
    elapsed = 0
    last_blink = nil
    last_sides = nil
    last_bar_width = nil
    last_fill_width = nil
    screen_spawned = false

    local builders = {
        build_gallery, build_transforms, build_hierarchy, build_tint,
        build_runtime, build_zorder, function() end,
    }
    for i, build in ipairs(builders) do
        build(page_x(i))
        text(TITLES[i], page_x(i) - 310, -172, 255, 255, 255)
    end

    -- Page counter, top right, bound to a signal.
    engine.spawn()
        :with_text("< 1 / 7 >", "default", 14, 140, 140, 140, 255)
        :with_screen_position(540, 8)
        :with_z_index(30)
        :with_signal_binding("showcase_page")
        :with_signal_binding_format("< {} / " .. PAGE_COUNT .. " >")
        :build()

    -- The camera follows an anchor entity. The follow config is global and other scenes leave
    -- their own offset and bounds behind, so set everything this scene relies on.
    snap_camera(0)
    engine.camera_follow_enable(true)
    engine.camera_follow_set_mode("lerp")
    engine.camera_follow_set_easing("ease_out")
    engine.camera_follow_set_speed(FOLLOW_SPEED)
    engine.camera_follow_set_offset(0, 0)
    engine.camera_follow_clear_bounds()
    engine.spawn()
        :with_position(0, 0)
        :with_camera_target()
        :register_as("showcase_camera")
        :build()

    engine.set_integer("showcase_page", page)
end

return M
