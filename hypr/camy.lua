-- CamY default window: floating, centred, 720×min(1220, 90% of logical height).
-- Super+T still toggles float for every other window; CamY only recentres.
-- Keep rule, event, and timer handles on the module so they survive GC.

local function is_camy(w)
  if not w then return false end
  local title = tostring(w.title or "")
  local initial = tostring(w.initial_title or "")
  return title:find("^CamY$") ~= nil or initial:find("^CamY$") ~= nil
end

local function desired_height(w)
  local h = 1220
  local ok, mon = pcall(function() return w.monitor end)
  if ok and mon then
    local height = tonumber(mon.height)
    local scale = tonumber(mon.scale)
    if height and scale and scale > 0 then
      h = math.min(1220, math.floor(math.floor(height / scale) * 0.90))
      if h < 720 then h = 720 end
    end
  end
  return h
end

-- Look the window up by title each time: the handle from window.open
-- goes stale. A long-lived 400ms timer (kept on _G) covers the later
-- layout pass. Do not hook window.update_rules — that re-enters on
-- every float/resize and can freeze Hyprland.
local SEL = "title:^CamY$"
local keep_ticks = 0

local function leave_special(w)
  local ws = w and w.workspace
  local id = ws and tonumber(ws.id)
  if not id or id >= 1 then return end
  local dest = hl.get_active_workspace()
  local did = dest and tonumber(dest.id)
  if not did or did < 1 then return end
  hl.dispatch(hl.dsp.window.move({
    workspace = tostring(did),
    follow = true,
    window = SEL,
  }))
end

local function place_camy(w)
  w = w or hl.get_window(SEL)
  if not is_camy(w) then return end
  leave_special(w)
  local h = desired_height(w)
  hl.dispatch(hl.dsp.window.float({ action = "on", window = SEL }))
  hl.dispatch(hl.dsp.window.resize({ x = 720, y = h, relative = false, window = SEL }))
  hl.dispatch(hl.dsp.window.center({ window = SEL }))
end

local function arm_keep()
  keep_ticks = 8
end

local function maybe_arm(w)
  if is_camy(w) then
    arm_keep()
    place_camy(w)
    return
  end
  if not w then return end
  local title = tostring(w.title or "")
  local class = tostring(w.class or w.initial_class or "")
  if title == "" and class:find("quickshell", 1, true) then
    arm_keep()
  end
end

local keep_timer = hl.timer(function()
  if keep_ticks <= 0 then return end
  keep_ticks = keep_ticks - 1
  place_camy(hl.get_window(SEL))
end, { type = "repeat", timeout = 400 })

local rule = hl.window_rule({
  name = "camy-window",
  match = { title = "^CamY$" },
  float = true,
  center = true,
  size = "720 min(1220, monitor_h*0.90)",
  suppress_event = "fullscreen maximize",
})
if o.floating_apps and o.floating_apps.title then
  table.insert(o.floating_apps.title, "^CamY$")
end

local open_sub = hl.on("window.open", maybe_arm)
local title_sub = hl.on("window.title", maybe_arm)

_G._camy_hypr = {
  rule = rule,
  open = open_sub,
  title = title_sub,
  keep = keep_timer,
}

o.rebind("SUPER + T", "Toggle window floating/tiling", function()
  local w = hl.get_active_window()
  local title = w and tostring(w.title or "") or ""
  if title:find("^CamY$") then
    hl.dispatch(hl.dsp.window.float({ action = "on", window = w }))
    hl.dispatch(hl.dsp.window.center({ window = w }))
    return
  end
  hl.dispatch(hl.dsp.window.float({ action = "toggle" }))
end)
