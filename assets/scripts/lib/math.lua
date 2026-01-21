-- lib/math.lua
-- Math helpers

local M = {}

--- Linear interpolation between two numbers.
--- @param a number
--- @param b number
--- @param t number
--- @return number
function M.lerp(a, b, t)
    return a + (b - a) * t
end

--- Inverse linear interpolation between two numbers.
--- @param a number
--- @param b number
--- @param v number
--- @return number
function M.inv_lerp(a, b, v)
    if a == b then
        return 0.0
    end
    return (v - a) / (b - a)
end

--- Remap a number from one range to another.
--- Given `in_min` to `in_max`, maps `v` to the corresponding value in `out_min` to `out_max`.
--- @param in_min number
--- @param in_max number
--- @param out_min number
--- @param out_max number
--- @param v number
--- @return number
function M.remap(in_min, in_max, out_min, out_max, v)
    local t = M.inv_lerp(in_min, in_max, v)
    return M.lerp(out_min, out_max, t)
end

--- Linear interpolation between two 2D vectors.
--- @param ax number
--- @param ay number
--- @param bx number
--- @param by number
--- @param t number
--- @return number, number
function M.lerp2(ax, ay, bx, by, t)
    return M.lerp(ax, bx, t), M.lerp(ay, by, t)
end

return M
