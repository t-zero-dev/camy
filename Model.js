.pragma library

function glyphCode(s3, s7) {
  return (s3 ? 3 : 0) + (s7 ? 7 : 0)
}

function modelTitle(model) {
  return String(model) === "s3" ? "YoloCam S3" : "YoloCam S7"
}

function helperPathFromUrl(url) {
  var s = String(url || "")
  if (s.indexOf("file://") === 0) s = s.substring(7)
  return s
}

function parseLine(text) {
  var raw = String(text || "").trim()
  if (!raw) return null
  try { return JSON.parse(raw) } catch (e) { return { event: "error", error: raw } }
}

function shutterLabel(us) {
  var n = Number(us)
  if (!(n > 0)) return "—"
  var denom = Math.round(1000000 / n)
  return "1/" + denom
}

function zoomMult(raw) {
  var n = Number(raw)
  if (!(n > 0)) return 1
  var m = 0.0625 * n - 2.125
  if (m < 1) return 1
  if (m > 4) return 4
  return m
}

function zoomRaw(mult) {
  var m = Number(mult)
  if (!(m > 0)) m = 1
  var raw = Math.round((m + 2.125) / 0.0625)
  if (raw < 50) return 50
  if (raw > 98) return 98
  return raw
}

function zoomLabel(raw) {
  return zoomMult(raw).toFixed(1) + "×"
}

function sameValue(a, b) {
  if (a === b) return true
  if (a === undefined || b === undefined || a === null || b === null) return false
  return String(a) === String(b)
}

function evLabel(raw) {
  var n = Number(raw)
  if (isNaN(n)) return "0"
  var ev = (n - 8) * 0.5
  return (ev > 0 ? "+" : "") + ev.toFixed(1)
}

function enumLabel(options, value) {
  var v = String(value)
  for (var i = 0; i < options.length; i++) {
    if (String(options[i].value) === v) return options[i].label
  }
  return v
}

function isoTip() {
  return "Sensor gain (higher is brighter and grainier)."
}

function shutterTip() {
  return "Shutter speed."
}

function zoomTip() {
  return "Optical zoom."
}

function focusModeTip(mode) {
  switch (Number(mode)) {
    case 1:
      return "Single autofocus, pick a point on the preview."
    case 3:
      return "Continuous autofocus. The lens keeps focusing in the area you set."
    case 4:
      return "Face — the camera finds a face and tracks it."
    case 2:
      return "Manual — you focus the lens; the camera does not autofocus."
    default:
      return "Focus mode."
  }
}

function focusModeLabel(mode) {
  return enumLabel([
    { value: 1, label: "AF-S" },
    { value: 2, label: "Manual" },
    { value: 3, label: "AF-C" },
    { value: 4, label: "Face" }
  ], mode)
}

var TABS = [
  { id: "video", label: "Video" },
  { id: "image", label: "Image" },
  { id: "focus", label: "Focus" },
  { id: "zoom", label: "Zoom" },
  { id: "exposure", label: "Exposure" },
  { id: "wb", label: "WB" },
  { id: "audio", label: "Audio" },
  { id: "status", label: "Status" }
]

function tabOptions() {
  return TABS.map(function(t) { return { value: t.id, label: t.label } })
}

function eventForSelected(ev, selectedModel) {
  if (!ev || ev.model === undefined || ev.model === null || ev.model === "") return true
  return String(ev.model) === String(selectedModel || "s7")
}

function afSize(mode, box) {
  mode = Number(mode)
  box = Number(box)
  if (mode === 1) return box === 1 ? 0.07 : (box === 2 ? 0.045 : 0.10)
  if (mode === 3) return box === 1 ? 0.72 : (box === 2 ? 0.50 : 0.92)
  return 0
}

function boxAt(cx, cy, size) {
  size = Math.max(0.03, Math.min(1, Number(size)))
  var x = Math.max(0, Math.min(1 - size, Number(cx) - size / 2))
  var y = Math.max(0, Math.min(1 - size, Number(cy) - size / 2))
  return { x: x, y: y, w: size, h: size }
}

function clampRect(r) {
  if (!r) return { x: 0, y: 0, w: 0.2, h: 0.2 }
  var x = Number(r.x), y = Number(r.y), w = Math.abs(Number(r.w)), h = Math.abs(Number(r.h))
  if (!(w > 0) || !(h > 0) || !isFinite(x) || !isFinite(y)) return { x: 0, y: 0, w: 0.2, h: 0.2 }
  if (x < 0) { w += x; x = 0 }
  if (y < 0) { h += y; y = 0 }
  w = Math.max(0.03, Math.min(1 - x, w))
  h = Math.max(0.03, Math.min(1 - y, h))
  if (x + w > 1) x = Math.max(0, 1 - w)
  if (y + h > 1) y = Math.max(0, 1 - h)
  return { x: x, y: y, w: w, h: h }
}

function afRegion(mode, box) {
  var size = afSize(mode, box)
  if (!(size > 0)) return null
  return boxAt(0.5, 0.5, size)
}

function previewNorm(frameW, frameH, mx, my) {
  var ar = 16 / 9
  var fw = Math.max(1, Number(frameW) - 2)
  var fh = Math.max(1, Number(frameH) - 2)
  var boxW, boxH, ox, oy
  if (fw / fh > ar) {
    boxH = fh
    boxW = fh * ar
    ox = (fw - boxW) / 2 + 1
    oy = 1
  } else {
    boxW = fw
    boxH = fw / ar
    ox = 1
    oy = (fh - boxH) / 2 + 1
  }
  var nx = (Number(mx) - ox) / boxW
  var ny = (Number(my) - oy) / boxH
  var inside = nx >= 0 && nx <= 1 && ny >= 0 && ny <= 1
  return { x: nx, y: ny, inside: inside }
}
