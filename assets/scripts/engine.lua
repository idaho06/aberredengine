---@meta

---@class engine
---Engine API provided by Aberred Engine (Rust)
---All functions are available globally via the `engine` table
engine = {}

-- ==================== Logging Functions ====================

---General purpose logging to stderr with "[Lua]" prefix
---@param message string The message to log
function engine.log(message) end

---Info level logging with "[Lua INFO]" prefix
---@param message string The message to log
function engine.log_info(message) end

---Warning level logging with "[Lua WARN]" prefix
---@param message string The message to log
function engine.log_warn(message) end

---Error level logging with "[Lua ERROR]" prefix
---@param message string The message to log
function engine.log_error(message) end

-- ==================== Input Functions ====================

---Check if the back/cancel button (ESC) is currently held down
---@return boolean pressed True if ESC is currently pressed
function engine.is_action_back_pressed() end

---Check if the back/cancel button (ESC) was just pressed this frame
---@return boolean just_pressed True if ESC was just pressed this frame
function engine.is_action_back_just_pressed() end

---Check if the confirm/action button (SPACE) is currently held down
---@return boolean pressed True if SPACE is currently pressed
function engine.is_action_confirm_pressed() end

---Check if the confirm/action button (SPACE) was just pressed this frame
---@return boolean just_pressed True if SPACE was just pressed this frame
function engine.is_action_confirm_just_pressed() end

-- ==================== Asset Loading ====================

---Load a texture from disk
---@param id string Texture identifier for later reference
---@param path string File path to the texture
function engine.load_texture(id, path) end

---Load a TrueType font with specified point size
---@param id string Font identifier for later reference
---@param path string File path to the TTF font
---@param size integer Point size for the font
function engine.load_font(id, path, size) end

---Load a music track (supports XM tracker format)
---@param id string Music identifier for later reference
---@param path string File path to the music file
function engine.load_music(id, path) end

---Load a sound effect (supports WAV format)
---@param id string Sound identifier for later reference
---@param path string File path to the sound file
function engine.load_sound(id, path) end

---Load a tilemap from JSON and texture atlas
---@param id string Tilemap identifier for later reference
---@param path_without_extension string Path without .json/.png extension
function engine.load_tilemap(id, path_without_extension) end

-- ==================== Audio Playback ====================

---Play a loaded music track
---@param id string Music identifier (from load_music)
---@param looped boolean Whether to loop the music
function engine.play_music(id, looped) end

---Play a loaded sound effect
---@param id string Sound identifier (from load_sound)
function engine.play_sound(id) end

---Stop all currently playing music
function engine.stop_all_music() end

-- ==================== Entity Spawning ====================

---@class EntityBuilder
---Fluent builder for creating entities
local EntityBuilder = {}

---Set entity's world position
---@param x number X coordinate
---@param y number Y coordinate
---@return EntityBuilder
function EntityBuilder:with_position(x, y) end

---Set entity's sprite
---@param tex_key string Texture identifier
---@param width number Sprite width in pixels
---@param height number Sprite height in pixels
---@param origin_x number Origin X in pixels (pivot point)
---@param origin_y number Origin Y in pixels (pivot point)
---@return EntityBuilder
function EntityBuilder:with_sprite(tex_key, width, height, origin_x, origin_y) end

---Set sprite offset for spritesheet frames
---@param offset_x number X offset into texture
---@param offset_y number Y offset into texture
---@return EntityBuilder
function EntityBuilder:with_sprite_offset(offset_x, offset_y) end

---Set sprite flipping
---@param flip_h boolean Flip horizontally
---@param flip_v boolean Flip vertically
---@return EntityBuilder
function EntityBuilder:with_sprite_flip(flip_h, flip_v) end

---Set entity's Z-index for render order
---@param z integer Z-index (higher = rendered on top)
---@return EntityBuilder
function EntityBuilder:with_zindex(z) end

---Set entity's velocity (adds RigidBody component)
---@param vx number Velocity X
---@param vy number Velocity Y
---@return EntityBuilder
function EntityBuilder:with_velocity(vx, vy) end

---Set entity's box collider
---@param width number Collider width
---@param height number Collider height
---@param origin_x number Origin X for collider
---@param origin_y number Origin Y for collider
---@return EntityBuilder
function EntityBuilder:with_collider(width, height, origin_x, origin_y) end

---Set collider offset
---@param offset_x number X offset
---@param offset_y number Y offset
---@return EntityBuilder
function EntityBuilder:with_collider_offset(offset_x, offset_y) end

---Make entity follow mouse cursor
---@param follow_x boolean Follow mouse X
---@param follow_y boolean Follow mouse Y
---@return EntityBuilder
function EntityBuilder:with_mouse_controlled(follow_x, follow_y) end

---Set entity's rotation in degrees
---@param degrees number Rotation angle
---@return EntityBuilder
function EntityBuilder:with_rotation(degrees) end

---Set entity's scale
---@param sx number Scale X
---@param sy number Scale Y
---@return EntityBuilder
function EntityBuilder:with_scale(sx, sy) end

---Mark entity as persistent across scene changes
---@return EntityBuilder
function EntityBuilder:with_persistent() end

---Add a group tag to the entity
---@param name string Group name
---@return EntityBuilder
function EntityBuilder:with_group(name) end

---Add a scalar signal to the entity
---@param key string Signal key
---@param value number Signal value
---@return EntityBuilder
function EntityBuilder:with_signal_scalar(key, value) end

---Add an integer signal to the entity
---@param key string Signal key
---@param value integer Signal value
---@return EntityBuilder
function EntityBuilder:with_signal_integer(key, value) end

---Add a flag signal to the entity
---@param key string Signal key
---@return EntityBuilder
function EntityBuilder:with_signal_flag(key) end

---Add a string signal to the entity
---@param key string Signal key
---@param value string Signal value
---@return EntityBuilder
function EntityBuilder:with_signal_string(key, value) end

---Add empty Signals component
---@return EntityBuilder
function EntityBuilder:with_signals() end

---Set entity's screen position (for UI elements)
---@param x number Screen X
---@param y number Screen Y
---@return EntityBuilder
function EntityBuilder:with_screen_position(x, y) end

---Add dynamic text to the entity
---@param content string Text content
---@param font string Font identifier
---@param font_size number Font size
---@param r integer Red (0-255)
---@param g integer Green (0-255)
---@param b integer Blue (0-255)
---@param a integer Alpha (0-255)
---@return EntityBuilder
function EntityBuilder:with_text(content, font, font_size, r, g, b, a) end

---Add a menu component
---@param items table[] Array of {id: string, label: string}
---@param origin_x number Menu origin X
---@param origin_y number Menu origin Y
---@param font string Font identifier
---@param font_size number Font size
---@param item_spacing number Spacing between items
---@param use_screen_space boolean Use screen coordinates
---@return EntityBuilder
function EntityBuilder:with_menu(items, origin_x, origin_y, font, font_size, item_spacing, use_screen_space) end

---Set menu colors
---@param nr integer Normal red
---@param ng integer Normal green
---@param nb integer Normal blue
---@param na integer Normal alpha
---@param sr integer Selected red
---@param sg integer Selected green
---@param sb integer Selected blue
---@param sa integer Selected alpha
---@return EntityBuilder
function EntityBuilder:with_menu_colors(nr, ng, nb, na, sr, sg, sb, sa) end

---Set menu dynamic text mode
---@param dynamic boolean Enable dynamic text
---@return EntityBuilder
function EntityBuilder:with_menu_dynamic_text(dynamic) end

---Set menu cursor entity
---@param key string WorldSignals key for cursor entity
---@return EntityBuilder
function EntityBuilder:with_menu_cursor(key) end

---Set menu selection sound
---@param sound_key string Sound identifier
---@return EntityBuilder
function EntityBuilder:with_menu_selection_sound(sound_key) end

---Add menu action to set scene
---@param item_id string Menu item ID
---@param scene string Scene name
---@return EntityBuilder
function EntityBuilder:with_menu_action_set_scene(item_id, scene) end

---Add menu action to show submenu
---@param item_id string Menu item ID
---@param submenu string Submenu name
---@return EntityBuilder
function EntityBuilder:with_menu_action_show_submenu(item_id, submenu) end

---Add menu action to quit game
---@param item_id string Menu item ID
---@return EntityBuilder
function EntityBuilder:with_menu_action_quit(item_id) end

---Add Lua phase state machine to entity
---@param phase_table table Phase definition: {initial: string, phases: table}
---@return EntityBuilder
function EntityBuilder:with_phase(phase_table) end

---Attach entity to another entity (StuckTo component)
---@param target_entity_id integer Target entity ID
---@param follow_x boolean Follow target X
---@param follow_y boolean Follow target Y
---@return EntityBuilder
function EntityBuilder:with_stuckto(target_entity_id, follow_x, follow_y) end

---Set StuckTo offset
---@param offset_x number X offset
---@param offset_y number Y offset
---@return EntityBuilder
function EntityBuilder:with_stuckto_offset(offset_x, offset_y) end

---Set StuckTo stored velocity (restored when detached)
---@param vx number Velocity X
---@param vy number Velocity Y
---@return EntityBuilder
function EntityBuilder:with_stuckto_stored_velocity(vx, vy) end

---Add timer component
---@param duration number Timer duration in seconds
---@param signal string Signal to emit when timer expires
---@return EntityBuilder
function EntityBuilder:with_timer(duration, signal) end

---Bind DynamicText to a WorldSignal value
---@param key string Signal key to bind to
---@return EntityBuilder
function EntityBuilder:with_signal_binding(key) end

---Set format string for signal binding
---@param format string Format string with {} placeholder
---@return EntityBuilder
function EntityBuilder:with_signal_binding_format(format) end

---Add grid layout spawner
---@param path string Path to JSON grid layout file
---@param group string Group name for spawned entities
---@param zindex integer Z-index for spawned entities
---@return EntityBuilder
function EntityBuilder:with_grid_layout(path, group, zindex) end

---Add position tween animation
---@param from_x number Start X
---@param from_y number Start Y
---@param to_x number End X
---@param to_y number End Y
---@param duration number Duration in seconds
---@return EntityBuilder
function EntityBuilder:with_tween_position(from_x, from_y, to_x, to_y, duration) end

---Set position tween easing
---@param easing string Easing function: "linear", "quad_in", "quad_out", etc.
---@return EntityBuilder
function EntityBuilder:with_tween_position_easing(easing) end

---Set position tween loop mode
---@param loop_mode string Loop mode: "once", "loop", "ping_pong"
---@return EntityBuilder
function EntityBuilder:with_tween_position_loop(loop_mode) end

---Add rotation tween animation
---@param from number Start rotation in degrees
---@param to number End rotation in degrees
---@param duration number Duration in seconds
---@return EntityBuilder
function EntityBuilder:with_tween_rotation(from, to, duration) end

---Set rotation tween easing
---@param easing string Easing function
---@return EntityBuilder
function EntityBuilder:with_tween_rotation_easing(easing) end

---Set rotation tween loop mode
---@param loop_mode string Loop mode
---@return EntityBuilder
function EntityBuilder:with_tween_rotation_loop(loop_mode) end

---Add scale tween animation
---@param from_x number Start scale X
---@param from_y number Start scale Y
---@param to_x number End scale X
---@param to_y number End scale Y
---@param duration number Duration in seconds
---@return EntityBuilder
function EntityBuilder:with_tween_scale(from_x, from_y, to_x, to_y, duration) end

---Set scale tween easing
---@param easing string Easing function
---@return EntityBuilder
function EntityBuilder:with_tween_scale_easing(easing) end

---Set scale tween loop mode
---@param loop_mode string Loop mode
---@return EntityBuilder
function EntityBuilder:with_tween_scale_loop(loop_mode) end

---Add Lua collision rule
---@param group_a string First group name
---@param group_b string Second group name
---@param callback string Lua callback function name
---@return EntityBuilder
function EntityBuilder:with_lua_collision_rule(group_a, group_b, callback) end

---Add animation component
---@param animation_key string Animation identifier
---@return EntityBuilder
function EntityBuilder:with_animation(animation_key) end

---Add animation controller
---@param fallback_key string Default animation key
---@return EntityBuilder
function EntityBuilder:with_animation_controller(fallback_key) end

---Add animation rule to controller
---@param condition_table table Condition definition
---@param set_key string Animation key to set when condition is true
---@return EntityBuilder
function EntityBuilder:with_animation_rule(condition_table, set_key) end

---Register spawned entity in WorldSignals with this key
---@param key string WorldSignals key for this entity
---@return EntityBuilder
function EntityBuilder:register_as(key) end

---Build and queue the entity for spawning
function EntityBuilder:build() end

---Start building a new entity
---@return EntityBuilder
function engine.spawn() end

-- ==================== World Signals ====================

---Get a scalar (float) signal value
---@param key string Signal key
---@return number|nil value The signal value or nil if not found
function engine.get_scalar(key) end

---Get an integer signal value
---@param key string Signal key
---@return integer|nil value The signal value or nil if not found
function engine.get_integer(key) end

---Get a string signal value
---@param key string Signal key
---@return string|nil value The signal value or nil if not found
function engine.get_string(key) end

---Check if a flag signal is set
---@param key string Signal key
---@return boolean set True if the flag is set
function engine.has_flag(key) end

---Set a scalar signal value
---@param key string Signal key
---@param value number Signal value
function engine.set_scalar(key, value) end

---Set an integer signal value
---@param key string Signal key
---@param value integer Signal value
function engine.set_integer(key, value) end

---Set a string signal value
---@param key string Signal key
---@param value string Signal value
function engine.set_string(key, value) end

---Set a flag signal
---@param key string Signal key
function engine.set_flag(key) end

---Clear a flag signal
---@param key string Signal key
function engine.clear_flag(key) end

---Get entity ID by WorldSignals key
---@param key string Entity registration key (from :register_as())
---@return integer|nil entity_id The entity ID or nil if not found
function engine.get_entity(key) end

-- ==================== Entity Commands ====================

---Set entity position
---@param entity_id integer Entity ID
---@param x number New X position
---@param y number New Y position
function engine.entity_set_position(entity_id, x, y) end

---Set entity velocity
---@param entity_id integer Entity ID
---@param vx number New velocity X
---@param vy number New velocity Y
function engine.entity_set_velocity(entity_id, vx, vy) end

---Set entity rotation
---@param entity_id integer Entity ID
---@param degrees number Rotation in degrees
function engine.entity_set_rotation(entity_id, degrees) end

---Set entity scale
---@param entity_id integer Entity ID
---@param sx number Scale X
---@param sy number Scale Y
function engine.entity_set_scale(entity_id, sx, sy) end

---Despawn an entity
---@param entity_id integer Entity ID
function engine.entity_despawn(entity_id) end

---Set entity scalar signal
---@param entity_id integer Entity ID
---@param key string Signal key
---@param value number Signal value
function engine.entity_signal_set_scalar(entity_id, key, value) end

---Set entity integer signal
---@param entity_id integer Entity ID
---@param key string Signal key
---@param value integer Signal value
function engine.entity_signal_set_integer(entity_id, key, value) end

---Set entity string signal
---@param entity_id integer Entity ID
---@param key string Signal key
---@param value string Signal value
function engine.entity_signal_set_string(entity_id, key, value) end

---Set entity flag signal
---@param entity_id integer Entity ID
---@param key string Signal key
function engine.entity_signal_set_flag(entity_id, key) end

---Clear entity flag signal
---@param entity_id integer Entity ID
---@param key string Signal key
function engine.entity_signal_clear_flag(entity_id, key) end

---Insert StuckTo component on an entity
---@param entity_id integer Entity ID
---@param target_id integer Target entity ID
---@param follow_x boolean Follow target X
---@param follow_y boolean Follow target Y
---@param offset_x number X offset
---@param offset_y number Y offset
---@param vx number Stored velocity X
---@param vy number Stored velocity Y
function engine.entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, vx, vy) end

---Release entity from StuckTo
---@param entity_id integer Entity ID
function engine.release_stuckto(entity_id) end

---Set entity animation
---@param entity_id integer Entity ID
---@param animation_key string Animation identifier
function engine.entity_set_animation(entity_id, animation_key) end

-- ==================== Phase Control ====================

---Transition entity to a new phase
---@param entity_id integer Entity ID with LuaPhase component
---@param next_phase string Name of the phase to transition to
function engine.phase_transition(entity_id, next_phase) end

-- ==================== Collision API (called from collision callbacks) ====================

---Play sound during collision (collision-scoped)
---@param sound_key string Sound identifier
function engine.collision_play_sound(sound_key) end

---Set integer signal during collision (collision-scoped)
---@param key string Signal key
---@param value integer Signal value
function engine.collision_set_integer(key, value) end

-- ==================== Group Tracking ====================

---Track a group for entity counting
---@param group_name string Group name to track
function engine.track_group(group_name) end

---Get count of entities in a tracked group
---@param group_name string Group name
---@return integer|nil count Entity count or nil if group not tracked
function engine.get_group_count(group_name) end

-- ==================== Camera Control ====================

---Set camera position and properties
---@param target_x number Camera target X
---@param target_y number Camera target Y
---@param offset_x number Screen offset X
---@param offset_y number Screen offset Y
---@param rotation number Camera rotation
---@param zoom number Camera zoom
function engine.set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom) end

-- ==================== Tilemap Rendering ====================

---Spawn tiles from a loaded tilemap
---@param tilemap_id string Tilemap identifier (from load_tilemap)
function engine.spawn_tiles(tilemap_id) end

-- ==================== Animation Registration ====================

---Register an animation resource
---@param id string Animation identifier
---@param tex_key string Texture identifier
---@param pos_x number Starting X position in texture
---@param pos_y number Starting Y position in texture
---@param displacement number X displacement between frames
---@param frame_count integer Number of frames
---@param fps number Frames per second
---@param looped boolean Whether animation loops
function engine.register_animation(id, tex_key, pos_x, pos_y, displacement, frame_count, fps, looped) end
