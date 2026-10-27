-- CamY default window: floating, centred, 720×min(1220, 90% of logical height).
-- Super+T still toggles float for every other window; CamY only recentres.
o.window({ title = "^CamY" }, { float = true })
o.window({ title = "^CamY" }, { center = true })
o.window({ title = "^CamY" }, { size = "720 min(1220, monitor_h*0.90)" })
o.window({ title = "^CamY" }, { suppress_event = "fullscreen maximize" })

o.rebind("SUPER + T", "Toggle window floating/tiling", function()
  local w = hl.get_active_window()
  local title = w and tostring(w.title or "") or ""
  if title:find("^CamY") then
    hl.dispatch(hl.dsp.window.float({ action = "set", window = w }))
    hl.dispatch(hl.dsp.window.center({ window = w }))
    return
  end
  hl.dispatch(hl.dsp.window.float({ action = "toggle" }))
end)
