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
      var rawCams = Array.isArray(ev.cameras) ? ev.cameras : []
      var cams = []
      for (var ci = 0; ci < rawCams.length; ci++) {
        var c = rawCams[ci] || {}
        cams.push({
          model: String(c.model) === "s3" ? "s3" : "s7",
          serial: Model.plain(c.serial || "", 64),
          id: Model.plain(c.id || c.serial || "", 64),
          usb: c.usb === true,
          control: c.control === true,
          product: Model.plain(c.product || "", 80),
          video: Model.safeVideo(c.video || ""),
          cameraIp: Model.safeIp(c.cameraIp || "")
        })
      }
      cameras = cams
      glyphCode = Number(ev.glyph || 0)
      if (ev.selected) {
        var next = String(ev.selected) === "s3" ? "s3" : "s7"
        if (next !== selectedModel) {
          unsupported = {}
          values = {}
          pending = {}
          selectedModel = next
          send({ cmd: "get-all", model: next })
        } else {
          selectedModel = next
        }
      }
      usb = cameras.length > 0
      if (!usb) {
        connected = false
        control = false
        previewOn = false
        focusRects = []
        focusArea = null
        firmware = ""
        serial = ""
        product = ""
        cameraIp = ""
        videoDevice = ""
        lastError = "YoloCam not on USB"
      }
      return
    }
    if (!eventForSelected(ev) && event !== "discover") return
    if (event === "caps") {
      if (!eventForSelected(ev)) return
      var hide = {}
      var list = ev.unsupported || []
      for (var i = 0; i < list.length; i++) hide[String(list[i])] = true
      unsupported = hide
      return
    }
    if (event === "discover") {
      usb = ev.usb === true
      serial = Model.plain(ev.serial || "", 64)
      product = Model.plain(ev.product || "", 80)
      cameraIp = Model.safeIp(ev.cameraIp || "")
      videoDevice = Model.safeVideo(ev.video || "")
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
        firmware = ""
        serial = ""
        product = ""
        cameraIp = ""
        videoDevice = ""
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
      lastError = Model.plain(ev.error || "disconnected", Model.MAX_ERROR)
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
      if (name === "peek") return
      if (ev.error) {
        lastError = Model.plain(ev.error, Model.MAX_ERROR)
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
        if (pending[name] !== undefined) {
          lastError = "Camera rejected that setting"
          clearPending(name, "Camera did not confirm")
          return
        }
        if (name === "image-version") return
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
        firmware = Model.plain(ev.value.firmware, 80)
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
      previewPath = Model.previewFileUrl(ev.path || "", 0) ? String(ev.path) : ""
      if (ev.covered === true || ev.covered === 1 || ev.covered === "1") lensCovered = true
      else if (ev.covered === false || ev.covered === 0 || ev.covered === "0") lensCovered = false
      return
    }
    if (event === "focus-box") {
      var boxes = Array.isArray(ev.rects) ? ev.rects : []
      focusRects = boxes.slice(0, 16)
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
      previewHolder = Model.plain(ev.holder || "", Model.MAX_LABEL)
      previewHolderKind = String(ev.holderKind || "")
      if (ev.error && ev.error !== "busy") lastError = Model.plain(ev.error, Model.MAX_ERROR)
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
      previewHolder = Model.plain(ev.holder || "", Model.MAX_LABEL)
      previewHolderKind = String(ev.holderKind || "")
      focusRects = []
      focusArea = null
      if (!previewBusy) lastError = Model.plain(ev.error || "preview failed", Model.MAX_ERROR)
      return
    }
    if (event === "error") {
      lastError = Model.plain(ev.error || "error", Model.MAX_ERROR)
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
    if (!usb) return
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
    if (!usb) return
    var next = Object.assign({}, values)
    next["wb-offset"] = { r: Number(r), b: Number(b) }
    values = next
    applyStatus = "Applying white balance…"
    send({ cmd: "set", name: "wb-offset", r: Number(r), b: Number(b), model: selectedModel })
  }

  function setParam(name, value) {
    if (!usb) return
    name = Model.plain(name, 32)
    value = Model.plain(value, 32)
    if (!/^[a-z][a-z0-9-]*$/.test(name) || value === "") {
      lastError = "invalid value"
      return
    }
    var numeric = /^-?\d+(\.\d+)?$/.test(value)
    var token = /^(on|off|auto|manual|line|mic|face|af-s|af-c|v0|v1)$/i.test(value)
    if (!numeric && !token) {
      lastError = "invalid value"
      return
    }
    var next = Object.assign({}, values)
    next[name] = numeric ? Number(value) : value
    values = next
    var p = Object.assign({}, pending)
    p[name] = next[name]
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
    if (on && !usb) return
    if (!previewEnabled && on) return
    if (on) lastError = ""
    send({ cmd: "preview", on: !!on, model: selectedModel })
  }

  function setAfTarget(rect) {
    if (!usb || !rect) return
    applyStatus = "Setting AF point…"
    send({ cmd: "af-target", x: rect.x, y: rect.y, w: rect.w, h: rect.h, model: selectedModel })
  }

  function setAfArea(rect) {
    if (!usb || !rect) return
    applyStatus = "Setting AF area…"
    send({ cmd: "af-area", x: rect.x, y: rect.y, w: rect.w, h: rect.h, model: selectedModel })
  }



  // Plugin bootstrap (run.sh --print-path): hashed dest only. A Process
  // so unload/disable can stop it. argv, never shell text; env -C because
  // Quickshell cwd may be qrc:/qs-blackhole. Inherited CAMY_HELPER_*
  // overrides are stripped.
  readonly property string pluginRoot: {
    var p = Model.helperPathFromUrl(Qt.resolvedUrl("run.sh"))
    if (p.length > 6 && p.substring(p.length - 6) === "run.sh")
      return p.substring(0, p.length - 7)
    return p
  }
  readonly property string runtimeDir: {
    var r = Quickshell.env("XDG_RUNTIME_DIR") || ""
    if (r.length && r.indexOf("/tmp") !== 0 && r.indexOf("/var/tmp") !== 0 && r.indexOf("/dev/shm") !== 0)
      return r + "/camy"
    var s = Quickshell.env("XDG_STATE_HOME") || ""
    if (s.length)
      return s + "/camy"
    return (Quickshell.env("HOME") || "") + "/.local/state/camy"
  }
  readonly property string bootstrapLog: runtimeDir + "/bootstrap.log"
  property string helperBin: ""
  property int bootstrapAttempts: 0
  property bool watchSawStart: false
  property string loadedVersion: ""
  property string installedVersion: ""
  readonly property bool updatePending: !!loadedVersion && !!installedVersion && installedVersion !== loadedVersion

  function pinnedHelperPath() {
    var home = Quickshell.env("HOME") || ""
    var data = Quickshell.env("XDG_DATA_HOME") || ""
    if (!data) data = home + "/.local/share"
    return data + "/camy/bin/camy"
  }

  function helperPathAllowed(p) {
    if (!p || p.charAt(0) !== "/" || p.indexOf("\n") !== -1) return false
    if (p.indexOf("/tmp/") === 0 || p === "/tmp") return false
    if (p.indexOf("/var/tmp/") === 0 || p === "/var/tmp") return false
    if (p.indexOf("/dev/shm/") === 0 || p === "/dev/shm") return false
    if (p === pinnedHelperPath()) return true
    if (p === pluginRoot + "/bin/camy") return true
    return false
  }

  function bootstrap() {
    resolveHelper()
  }

  function resolveHelper() {
    if (!active || resolveProcess.running) return
    var home = Quickshell.env("HOME") || ""
    if (!home) return
    resolveProcess.command = [
      "env", "-C", home,
      "-u", "CAMY_HELPER_BINARY",
      "-u", "CAMY_HELPER_URL",
      "-u", "CAMY_HELPER_PIN",
      "-u", "CAMY_HELPER_ASSET",
      "-u", "CAMY_HELPER_MACHINE",
      "CAMY_BOOTSTRAP_LOG=" + bootstrapLog,
      "bash", pluginRoot + "/run.sh", "--print-path"
    ]
    resolveProcess.running = true
  }

  function startWatch() {
    if (!active || watchProcess.running) return
    resolveHelper()
  }

  function stopWatch() {
    if (watchProcess.running) watchProcess.running = false
  }

  function stopBootstrap() {
    bootstrapRetry.stop()
    if (resolveProcess.running) resolveProcess.running = false
  }

  Component.onCompleted: {
    bootstrap()
    startWatch()
  }
  Component.onDestruction: {
    stopBootstrap()
    stopWatch()
  }
  onActiveChanged: {
    if (active) startWatch()
    else {
      stopBootstrap()
      stopWatch()
    }
  }

  Process {
    id: resolveProcess
    running: false
    stdout: StdioCollector { waitForEnd: true }
    onExited: function (exitCode) {
      if (!root.active) return
      if (exitCode === 0) {
        var p = String(stdout.text || "").trim()
        if (root.helperPathAllowed(p)) {
          root.helperBin = p
          root.bootstrapAttempts = 0
          if (!watchProcess.running) {
            watchProcess.command = [p, "watch"]
            watchProcess.running = true
          }
          return
        }
      }
      if (root.bootstrapAttempts >= 3) return
      root.bootstrapAttempts += 1
      bootstrapRetry.restart()
    }
  }

  Timer {
    id: bootstrapRetry
    interval: 20000
    repeat: false
    onTriggered: root.bootstrap()
  }

  FileView {
    path: root.bootstrapLog
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var lines = String(text() || "").trim().split("\n")
      if (watchProcess.running || !lines.length) return
      root.lastError = Model.plain(lines[lines.length - 1], Model.MAX_ERROR)
    }
  }

  FileView {
    path: root.pluginRoot + "/manifest.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var version = ""
      try { version = String(JSON.parse(text()).version || "") } catch (e) { return }
      if (!version) return
      if (!root.loadedVersion) root.loadedVersion = version
      root.installedVersion = version
    }
  }

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
        if (text) root.lastError = Model.plain(text, Model.MAX_ERROR)
      }
    }
    onStarted: {
      root.watchSawStart = true
      root.flushSendQueue()
    }
    onExited: function() {
      root.connected = false
      root.control = false
      root.previewOn = false
      if (!root.watchSawStart) {
        // Stale path from a previous checkout (plugin reinstall, missing
        // bin/camy). Drop it and fetch the pinned helper.
        root.helperBin = ""
        if (root.active) root.bootstrap()
      } else if (root.active) {
        watchRestart.restart()
      }
      root.watchSawStart = false
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
    target: "camy"
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
