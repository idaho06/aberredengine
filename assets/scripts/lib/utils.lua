-- lib/utils.lua
-- Shared Lua utilities

local M = {}

-- Pretty-print Lua values (especially tables) for debugging.
-- - `max_depth` prevents huge logs
-- - `visited` prevents infinite loops on cyclic tables
function M.dump_value(value, max_depth, indent, visited, force_float)
    max_depth = max_depth or 6
    indent = indent or 0
    visited = visited or {}
    force_float = force_float or true

    local t = type(value)
    if t == "string" then
        return string.format("%q", value)
    end
    if t == "number" then
        if force_float then
            local s = string.format("%.6f", value)
            s = s:gsub("0+$", ""):gsub("%.$", ".0")
            return s
        end
        return tostring(value)
    end
    if t ~= "table" then
        return tostring(value)
    end

    if visited[value] then
        return "<cycle>"
    end
    visited[value] = true

    if max_depth <= 0 then
        return "{...}"
    end

    local pad = string.rep("  ", indent)
    local pad_in = string.rep("  ", indent + 1)
    local parts = { "{" }

    -- Sort keys for stable logs
    local keys = {}
    for k in pairs(value) do
        keys[#keys + 1] = k
    end
    table.sort(keys, function(a, b)
        local ta, tb = type(a), type(b)
        if ta == tb then
            return tostring(a) < tostring(b)
        end
        return ta < tb
    end)

    for _, k in ipairs(keys) do
        local v = value[k]
        local key_repr
        if type(k) == "string" and k:match("^[%a_][%w_]*$") then
            key_repr = k
        else
            key_repr = "[" .. M.dump_value(k, max_depth - 1, indent + 1, visited, false) .. "]"
        end

        local child_force_float = false
        if type(k) == "string" then
            if k == "scalars" or k == "vel" then
                child_force_float = true
            elseif k == "speed_sq" or k == "time_in_phase" then
                child_force_float = true
            end
        end

        parts[#parts + 1] = string.format(
            "%s%s = %s,",
            pad_in,
            key_repr,
            M.dump_value(v, max_depth - 1, indent + 1, visited, child_force_float)
        )
    end

    parts[#parts + 1] = pad .. "}"
    return table.concat(parts, "\n")
end

return M
