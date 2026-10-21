import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model

Item {
  id: root

  property var shell: null
  property var settings: ({})
  property bool active: true
  property bool connected: false
  property bool usb: false
  property bool control: false
  property bool previewOn: false
  property bool previewBusy: false
  property string previewHolder: ""
  property string previewHolderKind: ""
  property string previewPath: ""
  property int previewSeq: 0
  property string lastError: ""
  property string product: ""
  property string serial: ""
  property string firmware: ""
  property string cameraIp: ""
  property string videoDevice: ""
  property var values: ({})
  property var pending: ({})
  property string applyStatus: ""
  property bool lensCovered: false
  property var focusRects: []
  property var focusArea: null
  property int heartbeatN: 0
  property var cameras: []
  property int glyphCode: 0
  property string selectedModel: "s7"
  property var unsupported: ({})
  readonly property bool bothCameras: cameras.length >= 2
  readonly property string selectedTitle: selectedModel === "s3" ? "YoloCam S3" : "YoloCam S7"

  readonly property bool previewEnabled: setting("preview", true) === true

  function setting(name, fallback) {
    var value = settings ? settings[name] : undefined
    return value === undefined || value === null ? fallback : value
  }

  function paramOk(name) {
    return unsupported[name] !== true
  }

  function eventForSelected(ev) {
    var m = ev && ev.model !== undefined && ev.model !== null ? String(ev.model) : ""
    if (!m) return true
    return m === selectedModel
  }

  function applyEvent(ev) {
    if (!ev || typeof ev !== "object") return
    var event = String(ev.event || "")
    if (event === "inventory") {
      cameras = Array.isArray(ev.cameras) ? ev.cameras : []
      glyphCode = Number(ev.glyph || 0)
      if (ev.selected) selectedModel = String(ev.selected)
      usb = cameras.length > 0
      if (!usb) {
        connected = false
        control = false
        previewOn = false
        focusRects = []
        focusArea = null
        lastError = "YoloCam not on USB"
      }
      return
    }
    if (!eventForSelected(ev) && event !== "discover" && event !== "caps") return
    if (event === "caps") {
      var hide = {}
      var list = ev.unsupported || []
      for (var i = 0; i < list.length; i++) hide[String(list[i])] = true
      unsupported = hide
      return
    }
    if (event === "discover") {
      usb = ev.usb === true
      serial = String(ev.serial || "")
      product = String(ev.product || "")
      cameraIp = String(ev.cameraIp || "")
      videoDevice = String(ev.video || "")
      if (glyphCode === 0 && usb) {
        var m = String(ev.model || selectedModel || "s7")
        glyphCode = m === "s3" ? 3 : 7
      }
      if (!usb) {
        connected = false
        control = false
        previewOn = false
        focusRects = []
        focusArea = null
        lastError = "YoloCam not on USB"
      } else if (lastError === "YoloCam not on USB" || lastError === "unplugged") {
        lastError = ""
      }
      return
    }
    if (event === "connected") {
      connected = true
      control = true
      lastError = ""
      return
    }
    if (event === "disconnected") {
      connected = false
      control = false
      lastError = String(ev.error || "disconnected")
      var why = lastError.toLowerCase()
      if (why === "unplugged" || why.indexOf("not on usb") !== -1)
        usb = false
      return
    }
    if (event === "refreshed") {
      lastError = ""
      applyStatus = "Refreshed"
      refreshWatchdog.stop()
      applyClear.restart()
      return
    }
    if (event === "param") {
      connected = true
      control = true
      var name = String(ev.name || "")
      if (ev.error) {
        lastError = String(ev.error)
        if (pending[name] !== undefined) {
          clearPending(name, "Camera did not confirm")
        } else if (name === "focus-target" || name === "focus-area") {
          applyStatus = "Applied"
          applyClear.restart()
        }
        return
      }
      lastError = ""
      if (Number(ev.resp) === 100) {
        var hide = Object.assign({}, unsupported)
        hide[name] = true
        unsupported = hide
        return
      }
      if (name === "focus-target" || name === "focus-area") {
        applyStatus = "Applied"
        applyClear.restart()
      }
      if (ev.value === null || ev.value === undefined) {
        if (pending[name] !== undefined) {
          var filled = Object.assign({}, values)
          filled[name] = pending[name]
          values = filled
          clearPending(name, "Applied")
        }
        return
      }
      if (pending[name] !== undefined) {
        if (Model.sameValue(pending[name], ev.value)) {
          clearPending(name, "Applied")
        } else {
          clearPending(name, "Camera kept its value")
        }
      }
      var next = Object.assign({}, values)
      next[name] = ev.value
      values = next
      if (name === "firmware" && ev.value && ev.value.firmware)
        firmware = String(ev.value.firmware)
      return
    }
    if (event === "covered") {
      lensCovered = ev.on === true || ev.covered === true || ev.on === 1 || ev.on === "1"
      return
    }
    if (event === "frame") {
      previewOn = true
      previewBusy = false
      previewSeq += 1
      previewPath = String(ev.path || "")
      if (ev.covered === true || ev.covered === 1 || ev.covered === "1") lensCovered = true
      else if (ev.covered === false || ev.covered === 0 || ev.covered === "0") lensCovered = false
      return
    }
    if (event === "focus-box") {
      focusRects = Array.isArray(ev.rects) ? ev.rects : []
      return
    }
    if (event === "focus-area") {
      var areas = Array.isArray(ev.rects) ? ev.rects : []
      focusArea = areas.length ? areas[0] : null
      return
    }
    if (event === "preview") {
      previewOn = ev.on === true
      previewBusy = String(ev.error || "") === "busy"
      previewHolder = String(ev.holder || "")
      previewHolderKind = String(ev.holderKind || "")
      if (ev.error && ev.error !== "busy") lastError = String(ev.error)
      if (previewOn) {
        previewPath = ""
      }
      if (!previewOn) {
        previewPath = ""
        previewHolderKind = ""
        lensCovered = false
        focusRects = []
        focusArea = null
      }
      return
    }
    if (event === "preview-error") {
      previewOn = false
      previewBusy = String(ev.error || "") === "busy"
      previewHolder = String(ev.holder || "")
      previewHolderKind = String(ev.holderKind || "")
      focusRects = []
      focusArea = null
      if (!previewBusy) lastError = String(ev.error || "preview failed")
      return
    }
    if (event === "error") {
      lastError = String(ev.error || "error")
    }
  }

  property var sendQueue: []

  function send(obj) {
    if (!obj) return
    if (!obj.model) obj = Object.assign({ model: selectedModel }, obj)
    if (!watchProcess.running) {
      sendQueue = sendQueue.concat([obj])
      return
    }
    watchProcess.write(JSON.stringify(obj) + "\n")
  }

  function flushSendQueue() {
    var q = sendQueue
    sendQueue = []
    for (var i = 0; i < q.length; i++) send(q[i])
  }

  function clearPending(name, status) {
    var p = Object.assign({}, pending)
    delete p[name]
    pending = p
    applyStatus = status || ""
    applyClear.restart()
  }

  function selectModel(model) {
    var m = String(model || "s7")
    if (m !== "s3" && m !== "s7") return
    if (m === selectedModel && bothCameras) {
      send({ cmd: "select", model: m })
      return
    }
    selectedModel = m
    values = {}
    pending = {}
    unsupported = {}
    previewOn = false
    previewPath = ""
    previewBusy = false
    lensCovered = false
    focusRects = []
    focusArea = null
    send({ cmd: "select", model: m })
    send({ cmd: "get-all", model: m })
  }

  function setWbOffset(r, b) {
    var next = Object.assign({}, values)
    next["wb-offset"] = { r: Number(r), b: Number(b) }
    values = next
    applyStatus = "Applying white balance…"
    send({ cmd: "set", name: "wb-offset", r: Number(r), b: Number(b), model: selectedModel })
  }

  function setParam(name, value) {
    var next = Object.assign({}, values)
    next[name] = value
    values = next
    var p = Object.assign({}, pending)
    p[name] = value
    pending = p
    applyStatus = "Applying " + name + "…"
    send({ cmd: "set", name: name, value: String(value), model: selectedModel })
  }

  function valueOf(name, fallback) {
    if (pending[name] !== undefined && pending[name] !== null) return pending[name]
    var v = values[name]
    return v === undefined || v === null ? fallback : v
  }

  function refresh() {
    applyStatus = "Refreshing…"
    refreshWatchdog.restart()
    send({ cmd: "refresh", model: selectedModel })
  }

  function setPreview(on) {
    if (!previewEnabled && on) return
    if (on) lastError = ""
    send({ cmd: "preview", on: !!on, model: selectedModel })
  }

  function setAfTarget(rect) {
    if (!rect) return
    applyStatus = "Setting AF point…"
    send({ cmd: "af-target", x: rect.x, y: rect.y, w: rect.w, h: rect.h, model: selectedModel })
  }

  function setAfArea(rect) {
    if (!rect) return
    applyStatus = "Setting AF area…"
    send({ cmd: "af-area", x: rect.x, y: rect.y, w: rect.w, h: rect.h, model: selectedModel })
  }



  readonly property string helperBin: Model.helperPathFromUrl(Qt.resolvedUrl("bin/yolocam"))

  function startWatch() {
    if (!active || watchProcess.running) return
    watchProcess.command = [helperBin, "watch"]
    watchProcess.running = true
  }

  function stopWatch() {
    if (watchProcess.running) watchProcess.running = false
  }

  Component.onCompleted: startWatch()
  onActiveChanged: active ? startWatch() : stopWatch()

  Process {
    id: watchProcess
    running: false
    stdinEnabled: true
    stdout: SplitParser {
      onRead: function(data) { root.applyEvent(Model.parseLine(data)) }
    }
    stderr: SplitParser {
      onRead: function(data) {
        var text = String(data || "").trim()
        if (text) root.lastError = text
      }
    }
    onStarted: root.flushSendQueue()
    onExited: function() {
      root.connected = false
      root.control = false
      root.previewOn = false
      if (root.active) watchRestart.restart()
    }
  }

  Timer {
    id: refreshWatchdog
    interval: 2500
    repeat: false
    onTriggered: if (root.applyStatus === "Refreshing…") root.applyStatus = ""
  }

  Timer {
    id: watchRestart
    interval: 2000
    repeat: false
    onTriggered: root.startWatch()
  }

  Timer {
    id: applyClear
    interval: 1600
    repeat: false
    onTriggered: root.applyStatus = ""
  }

  IpcHandler {
    target: "yolocam"
    function refresh(): string { root.refresh(); return "ok" }
    function iso(v: string): string { root.setParam("iso", v); return "ok" }
    function shutter(v: string): string { root.setParam("shutter-us", v); return "ok" }
    function scene(_v: string): string { return "unsupported" }
    function focusMode(v: string): string { root.setParam("focus-mode", v); return "ok" }
    function zoom(v: string): string { root.setParam("zoom", v); return "ok" }
    function state(): string {
      return JSON.stringify({
        usb: root.usb,
        control: root.control,
        glyph: root.glyphCode,
        selected: root.selectedModel,
        cameras: root.cameras,
        serial: root.serial,
        firmware: root.firmware,
        preview: root.previewOn,
        previewBusy: root.previewBusy,
        previewHolder: root.previewHolder,
        lensCovered: root.lensCovered,
        applyStatus: root.applyStatus,
        error: root.lastError,
        values: root.values
      })
    }
  }

}
