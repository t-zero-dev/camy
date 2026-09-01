import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

Panel {
  id: root
  moduleName: "yolocam"
  ipcTarget: "yolocam"
  manageIpc: false

  property string tab: "exposure"
  property bool cursorActive: false
  readonly property int focusMode: Number(val("focus-mode", 4))
  readonly property string holderIcon: {
    var k = String(service.previewHolderKind || "")
    if (k === "meet") return Qt.resolvedUrl("assets/holder-meet.svg")
    if (k === "zoom") return Qt.resolvedUrl("assets/holder-zoom.svg")
    if (k === "obs") return Qt.resolvedUrl("assets/holder-obs.svg")
    return ""
  }
  property bool afcCustom: false
  property var localAf: null
  property var afDraft: null
  property string afHint: ""
  property bool afDragging: false
  property bool afMoving: false
  property real afX0: 0
  property real afY0: 0
  property real afGrabOffX: 0
  property real afGrabOffY: 0

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var sharedService: bar && bar.shell && typeof bar.shell.serviceFor === "function"
    ? bar.shell.serviceFor(moduleName) : null
  readonly property var service: sharedService || localService
  readonly property bool present: service.usb === true
  readonly property string camGlyph: present ? "󰄀" : "󰵝"
  readonly property color iconColor: service.control ? Color.accent : (present ? foreground : dim)
  property bool previewShowA: true
  property bool previewReady: false
  property string heroHoverTip: ""
  property string focusHoverTip: ""

  function pushSettings() { if (service) service.settings = settings }
  onSettingsChanged: pushSettings()
  onServiceChanged: pushSettings()
  Component.onCompleted: pushSettings()

  onTabChanged: if (service && typeof service.send === "function") service.send({ cmd: "tab", name: tab })

  onOpenedChanged: {
    if (!service) return
    if (opened && typeof service.send === "function") service.send({ cmd: "tab", name: tab })
    if (!opened) {
      service.setPreview(false)
      root.previewReady = false
      previewA.source = ""
      previewB.source = ""
    }
  }

  Connections {
    target: service
    function onPreviewPathChanged() {
      var path = String(service.previewPath || "")
      if (path === "") return
      var url = "file://" + path + "?" + service.previewSeq
      var useA = path.indexOf("preview-a") !== -1
      if (useA) {
        if (previewA.status === Image.Loading) return
        previewA.source = url
      } else {
        if (previewB.status === Image.Loading) return
        previewB.source = url
      }
    }
    function onPreviewOnChanged() {
      root.previewReady = false
      previewA.source = ""
      previewB.source = ""
    }
  }

  function val(name, fallback) { return service.valueOf(name, fallback) }

  function setNum(name, value) {
    service.setParam(name, Math.round(value))
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Service {
    id: localService
    active: sharedService === null
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { service.refresh(); return "ok" }
    function iso(v: string): string { service.setParam("iso", v); return "ok" }
    function shutter(v: string): string { service.setParam("shutter-us", v); return "ok" }
    function scene(_v: string): string { return "unsupported" }
    function focusMode(v: string): string { service.setParam("focus-mode", v); return "ok" }
    function zoom(v: string): string { service.setParam("zoom", v); return "ok" }
    function state(): string {
      return JSON.stringify({
        usb: service.usb,
        control: service.control,
        serial: service.serial,
        firmware: service.firmware,
        preview: service.previewOn,
        previewBusy: service.previewBusy,
        previewHolder: service.previewHolder,
        previewPath: service.previewPath,
        previewReady: root.previewReady,
        lensCovered: service.lensCovered,
        focusRects: service.focusRects,
        applyStatus: service.applyStatus,
        error: service.lastError,
        values: service.values
      })
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.camGlyph
    active: service.control && present
    activeColor: Color.accent
    useActiveColor: true
    dimmed: false
    tooltipText: !present
      ? "YoloCam S7 · unplugged"
      : service.previewBusy
        ? "YoloCam S7 · in use by " + (service.previewHolder || "another app")
        : service.control
          ? "YoloCam S7 · connected"
          : "YoloCam S7 · USB only, no control"
    onPressed: function(b) {
      if (b === Qt.RightButton) service.setParam("focus-mode", val("focus-mode", 4) === 4 ? 3 : 4)
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(520))
    contentHeight: panel.fittedContentHeight(Style.space(720), Style.space(720))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (dy !== 0) {
          var tabs = Model.TABS
          var i = 0
          for (var t = 0; t < tabs.length; t++) if (tabs[t].id === root.tab) i = t
          var n = (i + dy + tabs.length) % tabs.length
          root.tab = tabs[n].id
        }
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      ColumnLayout {
        anchors.fill: parent
        spacing: Style.space(12)

          Item {
            Layout.fillWidth: true
            Layout.preferredHeight: Math.max(heroIcon.height, heroLabels.implicitHeight)
            width: parent.width
            implicitHeight: Math.max(heroIcon.height, heroLabels.implicitHeight)

            Text {
              id: heroIcon
              text: root.camGlyph
              color: root.iconColor
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
            }

            Column {
              id: heroLabels
              anchors.left: heroIcon.right
              anchors.leftMargin: Style.space(14)
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(3)

              Text {
                text: "YoloCam S7"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
              }

              Item {
                width: parent.width
                height: heroLine1Metrics.height

                Text {
                  id: heroLine1Metrics
                  visible: false
                  text: "X"
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                }

                Text {
                  anchors.fill: parent
                  visible: root.heroReadouts.length === 0
                  text: heroStatus().toUpperCase()
                  color: root.heroAlarm ? (bar ? bar.urgent : Color.urgent) : root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  elide: Text.ElideRight
                  verticalAlignment: Text.AlignVCenter
                }

                Row {
                  id: heroStats
                  anchors.fill: parent
                  visible: root.heroReadouts.length > 0
                  spacing: Style.space(8)
                  Repeater {
                    model: root.heroReadouts
                    Row {
                      spacing: Style.space(8)
                      Text {
                        visible: index > 0
                        text: "·"
                        color: root.dim
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.bodySmall
                      }
                      Text {
                        text: String(modelData.label || "").toUpperCase()
                        color: root.dim
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.bodySmall
                        HoverHandler {
                          onHoveredChanged: {
                            if (hovered)
                              root.heroHoverTip = String(modelData.tip || "")
                            else if (root.heroHoverTip === String(modelData.tip || ""))
                              root.heroHoverTip = ""
                          }
                        }
                      }
                    }
                  }
                }
              }

              Text {
                width: parent.width
                text: root.heroReadouts.length > 0 && root.heroHoverTip !== "" ? root.heroHoverTip : " "
                elide: Text.ElideRight
                maximumLineCount: 1
                wrapMode: Text.NoWrap
                opacity: root.heroReadouts.length > 0 && root.heroHoverTip !== "" ? 1 : 0
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                Behavior on opacity { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
              }
            }
          }

          Rectangle {
            id: previewFrame
            Layout.fillWidth: true
            Layout.preferredHeight: Math.round(keyCatcher.width * 9 / 16)
            width: parent.width
            height: Math.round(width * 9 / 16)
            radius: Style.cornerRadius
            color: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.06)
            readonly property bool live: root.previewReady && service.previewOn
            border.width: live ? Math.max(2, Style.normalBorderWidth) : Style.normalBorderWidth
            border.color: live ? Color.accent : Qt.rgba(foreground.r, foreground.g, foreground.b, 0.12)
            clip: true

            Image {
              id: previewA
              anchors.fill: parent
              anchors.margins: 1
              fillMode: Image.PreserveAspectFit
              asynchronous: true
              cache: false
              smooth: false
              visible: root.previewShowA && status === Image.Ready
              sourceSize.width: width
              sourceSize.height: height
              onStatusChanged: if (status === Image.Ready) { root.previewShowA = true; root.previewReady = true }
            }

            Image {
              id: previewB
              anchors.fill: parent
              anchors.margins: 1
              fillMode: Image.PreserveAspectFit
              asynchronous: true
              cache: false
              smooth: false
              visible: !root.previewShowA && status === Image.Ready
              sourceSize.width: width
              sourceSize.height: height
              onStatusChanged: if (status === Image.Ready) { root.previewShowA = false; root.previewReady = true }
            }

            Column {
              anchors.centerIn: parent
              spacing: Style.space(10)
              visible: !root.previewReady
              width: parent.width - Style.space(24)

              Image {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 32
                height: 32
                visible: service.previewBusy && root.holderIcon !== ""
                source: root.holderIcon
                fillMode: Image.PreserveAspectFit
                smooth: true
                cache: true
              }
              Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: !present ? "Camera unplugged"
                      : service.previewBusy ? ("In use by " + (service.previewHolder || "another app"))
                      : service.previewOn ? "Starting preview…"
                      : "Click for live preview"
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
              }
            }

            Repeater {
              model: root.afOverlay
              Rectangle {
                x: previewFrame.width * Number(modelData && modelData.x)
                y: previewFrame.height * Number(modelData && modelData.y)
                width: previewFrame.width * Number(modelData && modelData.w)
                height: previewFrame.height * Number(modelData && modelData.h)
                color: "transparent"
                border.width: 2
                border.color: Color.accent
                radius: 1
                visible: width > 4 && height > 4
              }
            }

            Rectangle {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.bottom: parent.bottom
              height: coverLabel.implicitHeight + Style.space(12)
              visible: root.previewReady && service.lensCovered
              color: Qt.rgba(0, 0, 0, 0.72)
              Text {
                id: coverLabel
                anchors.centerIn: parent
                text: "Lens looks covered"
                color: bar ? bar.urgent : Color.urgent
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                font.bold: true
              }
            }

            MouseArea {
              id: previewClick
              anchors.fill: parent
              enabled: root.opened && present
              hoverEnabled: true
              preventStealing: root.tab === "focus" && service.previewOn
              cursorShape: root.tab === "focus" && service.previewOn && root.afInteractive()
                ? Qt.CrossCursor : Qt.ArrowCursor

              function norm(mouse) {
                return Model.previewNorm(previewFrame.width, previewFrame.height, mouse.x, mouse.y)
              }

              onPressed: function(mouse) {
                if (!service.previewOn || root.tab !== "focus") return
                var n = norm(mouse)
                if (!n.inside) {
                  root.afHint = "Stay inside the frame."
                  return
                }
                var mode = Number(root.val("focus-mode", 4))
                if (mode === 4) return
                if (mode === 1) {
                  root.placeAfS(n.x, n.y)
                  return
                }
                if (mode !== 3) return
                if (root.afcCustom) {
                  var box = root.afDraft || root.localAf
                  if (box && n.x >= box.x && n.x <= box.x + box.w && n.y >= box.y && n.y <= box.y + box.h) {
                    root.afMoving = true
                    root.afGrabOffX = n.x - box.x
                    root.afGrabOffY = n.y - box.y
                    root.afDraft = { x: box.x, y: box.y, w: box.w, h: box.h }
                  } else {
                    root.afDragging = true
                    root.afX0 = n.x
                    root.afY0 = n.y
                    root.afDraft = { x: n.x, y: n.y, w: 0.03, h: 0.03 }
                  }
                  root.afHint = "Drag on the preview to draw the AF box."
                  return
                }
                root.placeAfC(n.x, n.y)
              }
              onPositionChanged: function(mouse) {
                if (!root.afDragging && !root.afMoving) return
                var n = norm(mouse)
                if (!n.inside) root.afHint = "Stay inside the frame."
                else if (root.afcCustom) root.afHint = "Drag on the preview to draw the AF box."
                var nx = Math.max(0, Math.min(1, n.x))
                var ny = Math.max(0, Math.min(1, n.y))
                if (root.afMoving && root.afDraft) {
                  var w = root.afDraft.w, h = root.afDraft.h
                  var x = Math.max(0, Math.min(1 - w, nx - root.afGrabOffX))
                  var y = Math.max(0, Math.min(1 - h, ny - root.afGrabOffY))
                  if (nx - root.afGrabOffX < 0 || ny - root.afGrabOffY < 0 || nx - root.afGrabOffX + w > 1 || ny - root.afGrabOffY + h > 1)
                    root.afHint = "Stay inside the frame."
                  root.afDraft = { x: x, y: y, w: w, h: h }
                  return
                }
                if (root.afDragging) {
                  var x0 = Math.min(root.afX0, nx), y0 = Math.min(root.afY0, ny)
                  var x1 = Math.max(root.afX0, nx), y1 = Math.max(root.afY0, ny)
                  root.afDraft = Model.clampRect({ x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
                }
              }
              onReleased: function(mouse) {
                if (root.afDragging || root.afMoving) {
                  var box = root.afDraft ? Model.clampRect(root.afDraft) : null
                  root.afDragging = false
                  root.afMoving = false
                  root.afDraft = null
                  if (box && box.w >= 0.03 && box.h >= 0.03) {
                    root.localAf = box
                    service.setAfArea(box)
                    root.afHint = ""
                  }
                  return
                }
                if (root.tab === "focus" && service.previewOn) return
                void mouse
                service.setPreview(!service.previewOn)
              }
              onClicked: function(mouse) {
                if (root.tab === "focus" && service.previewOn) {
                  void mouse
                  return
                }
              }
            }
          }

          Text {
            Layout.fillWidth: true
            visible: service.previewOn && root.previewReady && root.tab !== "focus"
            wrapMode: Text.Wrap
            horizontalAlignment: Text.AlignHCenter
            text: "Click the picture to stop the live view"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          ButtonGroup {
            Layout.fillWidth: true
            width: parent.width
            options: Model.tabOptions()
            value: root.tab
            foreground: root.foreground
            background: Color.popups.background
            accent: Color.accent
            fontFamily: root.fontFamily
            fontSize: Style.font.caption
            onChanged: function(v) { root.tab = v }
          }

          ScrollView {
            id: scrollArea
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            Column {
              id: panelColumn
              width: scrollArea.availableWidth
              spacing: Style.space(10)

              Loader {
                width: parent.width
                sourceComponent: root.tab === "video" ? videoTab
                  : root.tab === "image" ? imageTab
                  : root.tab === "focus" ? focusTab
                  : root.tab === "zoom" ? zoomTab
                  : root.tab === "exposure" ? exposureTab
                  : root.tab === "wb" ? wbTab
                  : root.tab === "audio" ? audioTab
                  : statusTab
              }
            }
          }
        }
    }
  }

  function scrollPanel(wheel) {
    var bar = scrollArea.ScrollBar.vertical
    if (!bar || bar.size >= 1) return
    var contentH = Math.max(1, panelColumn.implicitHeight)
    var step = (-wheel.angleDelta.y * 0.5) / contentH
    bar.position = Math.max(0, Math.min(1 - bar.size, bar.position + step))
  }

  // Face: CID 59. AF-S: click target (CID 24). AF-C: size presets or a drawn box (CID 34).
  readonly property var afOverlay: {
    var _draft = root.afDraft
    var _local = root.localAf
    var _custom = root.afcCustom
    if (!root.previewReady || !service.previewOn || root.tab !== "focus") return []
    return root.afRects()
  }

  function afInteractive() {
    var mode = Number(val("focus-mode", 4))
    return mode === 1 || mode === 3
  }

  function afRects() {
    var mode = Number(val("focus-mode", 4))
    if (mode === 2) return []
    if (mode === 4) {
      var live = service.focusRects
      if (live && live.length) return [live[0]]
      return []
    }
    if (root.afDraft) return [root.afDraft]
    if (root.localAf && Number(root.localAf.w) > 0) return [root.localAf]
    if (mode !== 1 && service.focusArea && Number(service.focusArea.w) > 0)
      return [service.focusArea]
    var region = Model.afRegion(mode, mode === 1 ? 0 : val("focus-box", 0))
    return region ? [region] : []
  }

  function placeAfS(nx, ny) {
    var size = Model.afSize(1, 0)
    var box = Model.boxAt(nx, ny, size)
    root.localAf = box
    root.afHint = ""
    service.setAfTarget(box)
  }

  function placeAfC(nx, ny) {
    var size = Model.afSize(3, val("focus-box", 0))
    var box = Model.boxAt(nx, ny, size)
    root.localAf = box
    root.afHint = ""
    service.setAfArea(box)
  }

  function setAfcShape(v) {
    var n = Number(v)
    root.afcCustom = n === 3
    if (n === 3) {
      if (!root.localAf) root.localAf = Model.afRegion(3, val("focus-box", 0))
      root.afHint = "Drag on the preview to draw the AF box."
      return
    }
    root.afHint = "Click the preview to place the AF area."
    root.setNum("focus-box", n)
    var box = Model.afRegion(3, n)
    if (box) {
      root.localAf = box
      service.setAfArea(box)
    }
  }

  readonly property bool heroAlarm: {
    if (service.lensCovered) return true
    var s = heroStatus()
    if (s === "Lens looks covered") return true
    if (service.applyStatus !== "") return service.applyStatus.indexOf("did not confirm") >= 0
    return service.lastError !== "" && s === service.lastError
  }

  function heroStatus() {
    if (service.applyStatus !== "") return service.applyStatus
    if (service.lensCovered) return "Lens looks covered"
    if (!present) return service.lastError !== "" ? service.lastError : "Unplugged"
    if (service.previewBusy) return "In use by " + (service.previewHolder || "another app")
    if (!service.control) return service.lastError !== "" ? service.lastError : "USB only — no control"
    var bits = []
    bits.push("ISO " + val("iso", "—"))
    bits.push(Model.shutterLabel(val("shutter-us", 0)))
    bits.push(Model.zoomLabel(val("zoom", 50)))
    bits.push(focusModeName(val("focus-mode", 4)))
    return bits.join(" · ")
  }

  function focusModeName(mode) {
    var n = Number(mode)
    if (n === 1) return "AF-S"
    if (n === 3) return "AF-C"
    if (n === 4) return "Face"
    if (n === 2) return "Manual"
    return String(mode)
  }

  function focusModeTip(mode) {
    var n = Number(mode)
    if (n === 1) return "Single autofocus, pick a point on the preview."
    if (n === 3) return "Continuous autofocus. The lens keeps focusing in the area you set."
    if (n === 4) return "Face — the camera finds a face and tracks it."
    if (n === 2) return "Manual — you focus the lens; the camera does not autofocus."
    return "Focus mode."
  }

  readonly property var heroReadouts: {
    var _vals = service.values
    var _pend = service.pending
    if (service.applyStatus !== "") return []
    if (service.lensCovered) return []
    if (!present) return []
    if (service.previewBusy) return []
    if (!service.control) return []
    var mode = val("focus-mode", 4)
    return [
      { label: "ISO " + val("iso", "—"), tip: "Sensor gain (higher is brighter and grainier)." },
      { label: Model.shutterLabel(val("shutter-us", 0)), tip: "Shutter speed." },
      { label: Model.zoomLabel(val("zoom", 50)), tip: "Optical zoom." },
      { label: focusModeName(mode), tip: "Focus mode." }
    ]
  }

  component EnumRow: Column {
    property string title
    property string name
    property var options: []
    property var fallback: options.length ? options[0].value : ""
    width: parent ? parent.width : 0
    spacing: Style.space(4)
    Text {
      text: title
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
    }
    ButtonGroup {
      width: parent.width
      options: parent.options
      value: String(root.val(name, fallback))
      foreground: root.foreground
      background: Color.popups.background
      fontFamily: root.fontFamily
      fontSize: Style.font.caption
      onChanged: function(v) { root.service.setParam(name, v) }
    }
  }

  component SliderRow: Column {
    property string title
    property string name
    property real minimum: 0
    property real maximum: 100
    property string readout: String(root.val(name, minimum))
    width: parent ? parent.width : 0
    spacing: Style.space(4)
    Item {
      width: parent.width
      height: label.implicitHeight
      Text {
        id: label
        text: title
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
      Text {
        anchors.right: parent.right
        text: readout
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
    PanelSlider {
      width: parent.width
      bar: root.bar
      minimum: parent.minimum
      maximum: parent.maximum
      step: 1
      integer: true
      value: Number(root.val(name, minimum))
      onReleased: function(v) { root.setNum(name, v) }
      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        onWheel: function(wheel) {
          root.scrollPanel(wheel)
          wheel.accepted = true
        }
      }
    }
  }

  Component {
    id: videoTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      EnumRow { title: "Frame rate"; name: "frame-rate"; options: [{value:"25",label:"25"},{value:"30",label:"30"},{value:"50",label:"50"},{value:"60",label:"60"}] }
      EnumRow { title: "HDMI format"; name: "hdmi-format"; options: [{value:"0",label:"Auto"},{value:"1",label:"4K"},{value:"2",label:"1080p"},{value:"3",label:"720p"}] }
      EnumRow { title: "HDMI color"; name: "hdmi-color"; options: [{value:"0",label:"Auto"},{value:"1",label:"RGB"},{value:"2",label:"YUV422"}] }
    }
  }

  Component {
    id: imageTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      SliderRow { title: "Sharpness"; name: "sharpness"; minimum: -10; maximum: 30 }
      SliderRow { title: "Contrast"; name: "contrast" }
      SliderRow { title: "Saturation"; name: "saturation" }
      SliderRow { title: "Brightness"; name: "brightness" }
      SliderRow { title: "Hue"; name: "hue" }
      SliderRow { title: "Gamma"; name: "gamma" }
      Row {
        width: parent.width
        spacing: Style.space(10)
        EnumRow { width: (parent.width - parent.spacing) / 2; title: "Distortion"; name: "distortion"; fallback: "1"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
        EnumRow { width: (parent.width - parent.spacing) / 2; title: "Vignette"; name: "shading"; fallback: "1"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      }
      EnumRow { title: "Color LUT"; name: "clut"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      SliderRow { title: "LUT mix"; name: "clut-mix"; maximum: 255 }
    }
  }

  Component {
    id: afcAreaControls
    Column {
      width: parent ? parent.width : 0
      spacing: Style.space(4)
      Text {
        text: "AF area"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
      ButtonGroup {
        width: parent.width
        options: [{value:"0",label:"Large"},{value:"1",label:"Medium"},{value:"2",label:"Small"},{value:"3",label:"Custom"}]
        value: root.afcCustom ? "3" : String(root.val("focus-box", 0))
        foreground: root.foreground
        background: Color.popups.background
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        onChanged: function(v) { root.setAfcShape(v) }
      }
    }
  }

  Component {
    id: focusTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      Column {
        width: parent.width
        spacing: Style.space(4)
        Text {
          text: "Focus mode"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
        ButtonGroup {
          width: parent.width
          options: [
            { value: "1", label: "AF-S" },
            { value: "3", label: "AF-C" },
            { value: "4", label: "Face" }
          ]
          value: String(root.val("focus-mode", 4))
          foreground: root.foreground
          background: Color.popups.background
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onHovered: function(index, on) {
            var tips = [root.focusModeTip(1), root.focusModeTip(3), root.focusModeTip(4)]
            root.focusHoverTip = on && index >= 0 && index < tips.length ? tips[index] : ""
          }
          onChanged: function(v) {
            root.service.setParam("focus-mode", v)
            var mode = Number(v)
            if (mode === 3 && !root.afcCustom) {
              var box = Model.afRegion(3, root.val("focus-box", 0))
              if (box) {
                root.localAf = box
                root.service.setAfArea(box)
              }
              root.afHint = "Click the preview to place the AF area."
            } else if (mode === 1) {
              if (root.localAf)
                root.placeAfS(root.localAf.x + root.localAf.w / 2, root.localAf.y + root.localAf.h / 2)
              else
                root.placeAfS(0.5, 0.5)
            } else if (mode === 4) {
              root.afHint = ""
            }
          }
        }
        Text {
          width: parent.width
          text: root.focusHoverTip !== "" ? root.focusHoverTip : " "
          elide: Text.ElideRight
          maximumLineCount: 1
          wrapMode: Text.NoWrap
          opacity: root.focusHoverTip !== "" ? 1 : 0
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          Behavior on opacity { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
        }
      }
      Loader {
        width: parent.width
        active: root.focusMode === 3
        visible: active
        height: active && item ? item.implicitHeight : 0
        sourceComponent: afcAreaControls
      }
      Text {
        width: parent.width
        visible: root.focusMode === 1 || root.focusMode === 3
        wrapMode: Text.Wrap
        text: root.afHint !== "" ? root.afHint
              : root.focusMode === 1
                ? "Click the preview to set the focus point."
                : (root.afcCustom ? "Drag on the preview to draw the AF box. Drag the box to move it."
                                  : "Click the preview to place the AF area.")
        color: root.afHint.indexOf("Stay") === 0 ? (root.bar ? root.bar.urgent : Color.urgent) : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      SliderRow { title: "Focus speed"; name: "focus-speed"; maximum: 5 }
    }
  }

  Component {
    id: zoomTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      Column {
        width: parent.width
        spacing: Style.space(4)
        Item {
          width: parent.width
          height: zLabel.implicitHeight
          Text {
            id: zLabel
            text: "Zoom"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }
          Text {
            anchors.right: parent.right
            text: Model.zoomLabel(root.val("zoom", 50))
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
        PanelSlider {
          width: parent.width
          bar: root.bar
          minimum: 1
          maximum: 4
          step: 0.1
          integer: false
          value: Model.zoomMult(root.val("zoom", 50))
          onReleased: function(v) { root.service.setParam("zoom", Model.zoomRaw(v)) }
          MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.NoButton
            onWheel: function(wheel) {
              root.scrollPanel(wheel)
              wheel.accepted = true
            }
          }
        }
        Text {
          width: parent.width
          wrapMode: Text.Wrap
          text: "1.0× is the full sensor. 4.0× is digital zoom (cleanest at 1080p)."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
      SliderRow { title: "Zoom speed"; name: "zoom-speed"; minimum: 1; maximum: 5 }
    }
  }

  Component {
    id: exposureTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      EnumRow { title: "Exposure"; name: "exposure-mode"; options: [{value:"0",label:"Auto"},{value:"1",label:"Manual"}] }
      SliderRow { title: "EV"; name: "ev"; maximum: 16; readout: Model.evLabel(root.val("ev", 8)) }
      EnumRow { title: "Anti-flicker"; name: "anti-flicker"; options: [{value:"0",label:"Off"},{value:"1",label:"50 Hz"},{value:"2",label:"60 Hz"}] }
      SliderRow { title: "ISO"; name: "iso"; minimum: 100; maximum: 9600 }
      SliderRow {
        title: "Shutter"
        name: "shutter-us"
        minimum: 125
        maximum: 40000
        readout: Model.shutterLabel(root.val("shutter-us", 16666))
      }
      SliderRow { title: "Aperture"; name: "aperture"; maximum: 160 }
      Row {
        width: parent.width
        spacing: Style.space(10)
        EnumRow { width: (parent.width - parent.spacing) / 2; title: "HDR"; name: "hdr"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
        EnumRow { width: (parent.width - parent.spacing) / 2; title: "Lock"; name: "exposure-lock"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      }
    }
  }

  Component {
    id: wbTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      EnumRow { title: "White balance"; name: "wb-mode"; options: [{value:"1",label:"Auto"},{value:"0",label:"Manual"}] }
      SliderRow { title: "Color temperature"; name: "wb-temp"; minimum: 2000; maximum: 10000; readout: String(root.val("wb-temp", 5500)) + " K" }
      EnumRow { title: "WB lock"; name: "wb-lock"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
    }
  }

  Component {
    id: audioTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      SliderRow { title: "Volume"; name: "audio-volume" }
      EnumRow { title: "Denoise"; name: "audio-denoise"; options: [{value:"0",label:"Off"},{value:"1",label:"Weak"},{value:"2",label:"Middle"},{value:"3",label:"Strong"}] }
      EnumRow { title: "Auto gain"; name: "audio-agc"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      EnumRow { title: "Input"; name: "mic-type"; options: [{value:"0",label:"Line"},{value:"1",label:"Mic"}] }
      SliderRow { title: "Delay (ms)"; name: "audio-delay"; maximum: 400 }
    }
  }

  Component {
    id: statusTab
    Column {
      width: parent.width
      spacing: Style.space(8)
      Text {
        width: parent.width
        wrapMode: Text.Wrap
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        text: "Firmware  " + (service.firmware || "—") + "\n"
            + "Serial    " + (service.serial || "—") + "\n"
            + "Control   " + (service.cameraIp || "—") + "\n"
            + "UVC       " + (service.videoDevice || "—") + "\n"
            + "Lens      " + (root.val("lens-status", "—") === 1 || root.val("lens-status", "—") === "1" ? "mounted" : String(root.val("lens-status", "—")))
      }
      PanelActionButton {
        iconText: "󰑐"
        tooltipText: "Refresh from camera"
        foreground: root.foreground
        fontFamily: root.fontFamily
        onClicked: service.refresh()
      }
    }
  }
}
