-- CamY stays a floating window. No fullscreen, no full-tile.
-- Same split o.window calls as Omarchy About / LocalSend.
-- Names keep the rules alive after this file is required.
o.window({ title = "^CamY" }, { name = "camy-window", float = true })
o.window({ title = "^CamY" }, { name = "camy-center", center = true })
o.window({ title = "^CamY" }, { name = "camy-size", size = "720 min(1220, monitor_h*0.90)" })
o.window({ title = "^CamY" }, { name = "camy-suppress", suppress_event = "fullscreen maximize" })

-- Super+T still toggles float for every other window; CamY only recentres.
o.rebind("SUPER + T", "Toggle window floating/tiling", function()
  local w = hl.get_active_window()
  local title = w and tostring(w.title or "") or ""
  if title:find("^CamY") then
    hl.dispatch(hl.dsp.window.float({ action = "on", window = w }))
    hl.dispatch(hl.dsp.window.center({ window = w }))
    return
  end
  hl.dispatch(hl.dsp.window.float({ action = "toggle" }))
end)
