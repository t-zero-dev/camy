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
  property string previewOrientation: "16:9"
  property bool previewFlipV: false
  readonly property bool portraitPreview: previewOrientation === "9:16"
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
  readonly property int glyphCode: Number(service.glyphCode || 0)
  readonly property bool hasS7: glyphCode === 7 || glyphCode === 10
  readonly property bool hasS3: glyphCode === 3 || glyphCode === 10
  readonly property color s7Color: Color.accent
  readonly property color s3Color: hueShift(Color.accent, 0.22)
  readonly property color iconColor: !present ? dim : (hasS3 && !hasS7 ? tint(s3Color, service.control) : tint(s7Color, service.control))
  readonly property string heroTitle: service.selectedTitle || "YoloCam S7"
  readonly property bool showPicker: service.bothCameras === true
  readonly property var tabList: {
    var all = Model.TABS
    var out = []
    for (var i = 0; i < all.length; i++) {
      if (all[i].id === "zoom" && !service.paramOk("zoom")) continue
      if (all[i].id === "audio" && String(service.selectedModel) === "s3") continue
      out.push({ value: all[i].id, label: all[i].label })
    }
    return out
  }
  onTabListChanged: {
    var ok = false
    for (var i = 0; i < tabList.length; i++) if (tabList[i].value === tab) ok = true
    if (!ok) tab = "exposure"
  }

  function hueShift(c, turns) {
    var h = (c.hslHue + turns) % 1
    if (h < 0) h += 1
    // Urgent-red band reads as an error; skip it.
    if (h < 0.08 || h > 0.95) h = (h + 0.22) % 1
    var s = Math.max(c.hslSaturation, 0.5)
    var l = Math.min(Math.max(c.hslLightness, 0.48), 0.68)
    return Qt.hsla(h, s, l, 1)
  }

  function tint(c, live) {
    return live ? c : Qt.rgba(c.r, c.g, c.b, 0.55)
  }

  function modelControl(id) {
    var list = service.cameras || []
    for (var i = 0; i < list.length; i++) {
      if (String(list[i].model) === id) return list[i].control === true
    }
    return service.control === true
  }
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
        glyph: root.glyphCode,
        selected: service.selectedModel,
        cameras: service.cameras,
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
    iconComponent: camIcon
    active: service.control && present
    activeColor: Color.accent
    useActiveColor: true
    dimmed: false
    tooltipText: {
      var a = root.modelStatusLine("s7", "YoloCam S7")
      var b = root.modelStatusLine("s3", "YoloCam S3")
      var n = Math.max(a.length, b.length)
      function pad(s) {
        while (s.length < n) s += " "
        return s
      }
      return pad(a) + "\n" + pad(b)
    }
    onPressed: function(b) {
      if (b === Qt.LeftButton) root.toggle()
    }
  }

  Component {
    id: camIcon
    CamMark {
      anchors.fill: parent
      pixelSize: Style.bar.iconFont
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

            CamMark {
              id: heroIcon
              pixelSize: Style.font.display
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              width: implicitWidth
              height: implicitHeight
            }

            Column {
              id: heroLabels
              anchors.left: heroIcon.right
              anchors.leftMargin: Style.space(14)
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(3)

              Text {
                text: root.heroTitle
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
              }

              Row {
                visible: root.showPicker
                spacing: Style.space(6)
                PickerChip { modelId: "s7"; label: "S7" }
                PickerChip { modelId: "s3"; label: "S3" }
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
                  spacing: Style.space(6)
                  clip: false
                  Repeater {
                    model: root.heroReadouts
                    Text {
                      required property int index
                      required property var modelData
                      text: (index > 0 ? "·  " : "") + String(modelData.label || "").toUpperCase()
                      color: root.dim
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.bodySmall
                      verticalAlignment: Text.AlignVCenter
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
            Layout.preferredHeight: root.portraitPreview
              ? Math.round(Math.min(keyCatcher.width * 0.78, 420))
              : Math.round(keyCatcher.width * 9 / 16)
            width: parent.width
            height: root.portraitPreview
              ? Math.round(Math.min(width * 0.78, 420))
              : Math.round(width * 9 / 16)
            radius: Style.cornerRadius
            color: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.06)
            readonly property bool live: root.previewReady && service.previewOn
            border.width: live ? Math.max(2, Style.normalBorderWidth) : Style.normalBorderWidth
            border.color: live ? Color.accent : Qt.rgba(foreground.r, foreground.g, foreground.b, 0.12)
            clip: true

            Item {
              id: viewBox
              anchors.horizontalCenter: parent.horizontalCenter
              anchors.verticalCenter: parent.verticalCenter
              width: root.portraitPreview ? Math.round(parent.height * 9 / 16) : parent.width
              height: parent.height
              clip: true

            Image {
              id: previewA
              anchors.centerIn: parent
              width: root.portraitPreview ? parent.height : parent.width
              height: root.portraitPreview ? parent.width : parent.height
              rotation: root.portraitPreview ? 90 : 0
              transform: Scale { origin.x: previewA.width / 2; origin.y: previewA.height / 2; xScale: 1; yScale: root.previewFlipV ? -1 : 1 }
              fillMode: Image.PreserveAspectCrop
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
              anchors.centerIn: parent
              width: root.portraitPreview ? parent.height : parent.width
              height: root.portraitPreview ? parent.width : parent.height
              rotation: root.portraitPreview ? 90 : 0
              transform: Scale { origin.x: previewB.width / 2; origin.y: previewB.height / 2; xScale: 1; yScale: root.previewFlipV ? -1 : 1 }
              fillMode: Image.PreserveAspectCrop
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
                      : (service.lastError ? String(service.lastError) : "Click for live preview")
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
              }
            }

            Repeater {
              model: root.afOverlay
              Rectangle {
                readonly property var d: root.camToDisplay(modelData)
                x: viewBox.width * Number(d && d.x)
                y: viewBox.height * Number(d && d.y)
                width: viewBox.width * Number(d && d.w)
                height: viewBox.height * Number(d && d.h)
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
              z: 8
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
                var n = Model.previewNorm(viewBox.width, viewBox.height, mouse.x, mouse.y)
                var c = root.displayToCam(n.x, n.y)
                return { x: c.x, y: c.y, inside: n.inside }
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
                void mouse
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
                }
              }
              onClicked: function(mouse) {
                void mouse
                if (root.afDragging || root.afMoving) return
                if (service.previewOn && root.tab === "focus") return
                service.setPreview(!service.previewOn)
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
            options: root.tabList
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
              width: Math.max(0, scrollArea.availableWidth - Style.space(22))
              spacing: Style.space(10)
              topPadding: Style.space(28)

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

  function camToDisplay(r) {
    if (!r) return r
    var x = Number(r.x), y = Number(r.y), w = Number(r.w), h = Number(r.h)
    if (root.portraitPreview) {
      var nx = 1 - y - h
      var ny = x
      var nw = h
      var nh = w
      x = nx; y = ny; w = nw; h = nh
    }
    if (root.previewFlipV) y = 1 - y - h
    return { x: x, y: y, w: w, h: h }
  }

  function displayToCam(dx, dy) {
    if (root.previewFlipV) dy = 1 - dy
    if (!root.portraitPreview) return { x: dx, y: dy }
    return { x: dy, y: 1 - dx }
  }

  function modelStatusLine(id, label) {
    var list = service.cameras || []
    var cam = null
    for (var i = 0; i < list.length; i++) {
      if (String(list[i].model) === id) cam = list[i]
    }
    if (!cam) return label + " · unplugged"
    if (service.previewBusy && String(service.selectedModel) === id)
      return label + " · in use by " + (service.previewHolder || "another app")
    if (cam.control) return label + " · connected"
    return label + " · USB only, no control"
  }

  function hasVal(name) {
    var v = service.values ? service.values[name] : undefined
    return v !== undefined && v !== null && v !== ""
  }

  function heroStatus() {
    if (service.applyStatus !== "") return service.applyStatus
    if (service.lensCovered) return "Lens looks covered"
    if (!present) return service.lastError !== "" ? service.lastError : "Unplugged"
    if (service.previewBusy) return "In use by " + (service.previewHolder || "another app")
    if (!service.control) return service.lastError !== "" ? service.lastError : "USB only — no control"
    if (service.lastError !== "") return service.lastError
    return "Reading camera…"
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
    var rows = []
    if (hasVal("iso"))
      rows.push({ label: "ISO " + val("iso", "—"), tip: "Sensor gain" })
    if (hasVal("shutter-us"))
      rows.push({ label: Model.shutterLabel(val("shutter-us", 0)), tip: "Shutter speed." })
    if (service.paramOk("zoom") && hasVal("zoom"))
      rows.push({ label: Model.zoomLabel(val("zoom", 50)), tip: "Optical zoom." })
    if (hasVal("focus-mode"))
      rows.push({ label: focusModeName(val("focus-mode", 4)), tip: "Focus mode." })
    return rows
  }

  component CamMark: Item {
    property real pixelSize: Style.bar.iconFont
    readonly property bool stacked: root.hasS3 && root.hasS7
    readonly property bool s3Front: String(service.selectedModel) === "s3"
    readonly property real shift: stacked ? Math.max(2, Math.round(pixelSize * 0.2)) : 0
    implicitWidth: pixelSize + shift
    implicitHeight: pixelSize + shift

    Text {
      visible: stacked
      x: 0
      y: 0
      z: 0
      text: "󰄀"
      color: root.tint(parent.s3Front ? root.s7Color : root.s3Color, root.modelControl(parent.s3Front ? "s7" : "s3"))
      font.family: root.fontFamily
      font.pixelSize: parent.pixelSize
    }
    Text {
      x: parent.shift
      y: parent.shift
      z: 1
      text: root.present ? "󰄀" : "󰵝"
      color: !root.present
        ? root.dim
        : stacked
          ? root.tint(parent.s3Front ? root.s3Color : root.s7Color, root.modelControl(parent.s3Front ? "s3" : "s7"))
          : root.hasS3
            ? root.tint(root.s3Color, root.modelControl("s3"))
            : root.tint(root.s7Color, root.modelControl("s7"))
      font.family: root.fontFamily
      font.pixelSize: parent.pixelSize
    }
  }

  component PickerChip: Rectangle {
    property string modelId
    property string label
    readonly property bool on: String(service.selectedModel) === modelId
    readonly property color mark: modelId === "s3" ? root.s3Color : root.s7Color
    width: chipLabel.implicitWidth + Style.space(12)
    height: chipLabel.implicitHeight + Style.space(6)
    radius: height / 2
    color: on ? mark : Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    Text {
      id: chipLabel
      anchors.centerIn: parent
      text: parent.label
      color: parent.on ? Color.background : root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: parent.on
    }
    MouseArea {
      anchors.fill: parent
      cursorShape: Qt.PointingHandCursor
      onClicked: {
        service.selectModel(parent.modelId)
        if (typeof service.send === "function") service.send({ cmd: "tab", name: root.tab })
      }
    }
  }

  component EnumRow: Column {
    property string title
    property string name
    property var options: []
    property var fallback: options.length ? options[0].value : ""
    visible: service.paramOk(name)
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

  component WbOffsetRow: Column {
    id: wbOff
    property string title
    property string channel
    readonly property int cur: {
      var o = root.val("wb-offset", { r: 10, b: 10 })
      return Number(channel === "r" ? (o && o.r) : (o && o.b)) || 10
    }
    width: parent ? parent.width : 0
    spacing: Style.space(4)
    Item {
      width: parent.width
      height: ol.implicitHeight
      Text {
        id: ol
        text: title
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
      Text {
        anchors.right: parent.right
        text: String(wbOff.cur)
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
    Item {
      width: parent.width
      height: wbSl.height
      PanelSlider {
        id: wbSl
        width: parent.width
        bar: root.bar
        minimum: 0
        maximum: 20
        step: 1
        integer: true
        value: parent.parent.cur
        onReleased: function(v) {
          var o = root.val("wb-offset", { r: 10, b: 10 })
          var r = Number(o && o.r) || 10
          var b = Number(o && o.b) || 10
          if (channel === "r") r = v
          else b = v
          service.setWbOffset(r, b)
        }
      }
      MouseArea {
        anchors.fill: wbSl
        acceptedButtons: Qt.NoButton
        onWheel: function(wheel) { wheel.accepted = true }
      }
    }
  }

  component SliderRow: Column {
    id: srow
    property string title
    property string name
    property real minimum: 0
    property real maximum: 100
    property real step: 1
    property bool integer: true
    property string readout: formatLive(Number(root.val(name, minimum)))
    property var formatLive: function(v) {
      if (srow.integer) return String(Math.round(v))
      return Number(v).toFixed(1)
    }
    property var parseLive: function(s) {
      var n = Number(String(s).replace("+", ""))
      return isNaN(n) ? null : n
    }
    visible: service.paramOk(name)
    width: parent ? parent.width : 0
    spacing: Style.space(4)

    Text {
      text: title
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
    }

    Row {
      width: parent.width
      spacing: Style.space(8)
      height: Math.max(sl.height, stepper.height)

      Item {
        width: parent.width - stepper.width - parent.spacing
        height: sl.height
        PanelSlider {
          id: sl
          width: parent.width
          bar: root.bar
          minimum: srow.minimum
          maximum: srow.maximum
          step: srow.step
          integer: srow.integer
          value: Number(root.val(name, srow.minimum))
          onMoved: function(v) {
            if (!box.activeFocus) box.text = srow.formatLive(v)
          }
          onReleased: function(v) { root.setNum(name, v) }
        }
        MouseArea {
          anchors.fill: sl
          acceptedButtons: Qt.NoButton
          onWheel: function(wheel) { wheel.accepted = true }
        }
        Rectangle {
          visible: sl.dragging
          z: 20
          width: tipLab.implicitWidth + Style.space(10)
          height: tipLab.implicitHeight + Style.space(6)
          radius: Style.cornerRadius
          color: Color.tooltip.background
          border.color: Color.tooltip.border
          border.width: 1
          x: Math.max(0, Math.min(parent.width - width, sl.width * sl.progress - width / 2))
          y: {
            var gy = sl.mapToItem(scrollArea, 0, 0).y
            if (gy < height + Style.space(8)) return sl.height + Style.space(4)
            return -height - Style.space(4)
          }
          Text {
            id: tipLab
            anchors.centerIn: parent
            text: srow.formatLive(sl.liveValue)
            color: Color.tooltip.text
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }

      Rectangle {
        id: stepper
        width: Style.space(78)
        height: Style.space(28)
        radius: Style.cornerRadius
        color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
        border.width: 1
        border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.16)

        TextInput {
          id: box
          anchors.left: parent.left
          anchors.right: arrows.left
          anchors.verticalCenter: parent.verticalCenter
          anchors.leftMargin: Style.space(6)
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          selectByMouse: true
          clip: true
          text: srow.formatLive(Number(root.val(name, srow.minimum)))
          onEditingFinished: {
            var n = srow.parseLive(text)
            if (n === null || n === undefined || isNaN(n)) {
              text = srow.formatLive(Number(root.val(name, srow.minimum)))
              return
            }
            n = Math.max(srow.minimum, Math.min(srow.maximum, n))
            if (srow.integer) n = Math.round(n)
            root.setNum(name, n)
            text = srow.formatLive(n)
          }
        }

        Column {
          id: arrows
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          anchors.rightMargin: Style.space(4)
          spacing: 0
          Text {
            text: "▴"
            color: root.dim
            font.pixelSize: Style.font.caption
            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.PointingHandCursor
              onClicked: {
                var n = Number(root.val(name, srow.minimum)) + srow.step
                n = Math.min(srow.maximum, srow.integer ? Math.round(n) : n)
                root.setNum(name, n)
                box.text = srow.formatLive(n)
              }
            }
          }
          Text {
            text: "▾"
            color: root.dim
            font.pixelSize: Style.font.caption
            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.PointingHandCursor
              onClicked: {
                var n = Number(root.val(name, srow.minimum)) - srow.step
                n = Math.max(srow.minimum, srow.integer ? Math.round(n) : n)
                root.setNum(name, n)
                box.text = srow.formatLive(n)
              }
            }
          }
        }
      }
    }

    Connections {
      target: service
      function onValuesChanged() {
        if (sl.dragging || box.activeFocus) return
        box.text = srow.formatLive(Number(root.val(name, srow.minimum)))
      }
    }
  }

  Component {
    id: videoTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      Column {
        width: parent.width
        spacing: Style.space(4)
        Text {
          text: "Orientation"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
        ButtonGroup {
          width: parent.width
          options: [{ value: "16:9", label: "16:9" }, { value: "9:16", label: "9:16" }]
          value: root.previewOrientation
          foreground: root.foreground
          background: Color.popups.background
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onChanged: function(v) { root.previewOrientation = v }
        }
      }
      EnumRow { title: "Frame rate"; name: "frame-rate"; options: [{value:"25",label:"25"},{value:"30",label:"30"},{value:"50",label:"50"},{value:"60",label:"60"}] }
      EnumRow { title: "HDMI format"; name: "hdmi-format"; options: [{value:"0",label:"Auto"},{value:"1",label:"4K"},{value:"2",label:"1080p"},{value:"3",label:"720p"}] }
      EnumRow { title: "HDMI color"; name: "hdmi-color"; options: [{value:"0",label:"Auto"},{value:"1",label:"RGB"},{value:"2",label:"YUV422"}] }
      Column {
        width: parent.width
        spacing: Style.space(4)
        visible: String(service.selectedModel) === "s7"
        Text {
          text: "Vertical flip"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
        ButtonGroup {
          width: parent.width
          options: [{ value: "0", label: "Off" }, { value: "1", label: "On" }]
          value: root.previewFlipV ? "1" : "0"
          foreground: root.foreground
          background: Color.popups.background
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onChanged: function(v) { root.previewFlipV = v === "1" }
        }
        Text {
          width: parent.width
          wrapMode: Text.Wrap
          text: "Mirrors the preview top-to-bottom. Does not change the USB or HDMI feed."
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
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
      SliderRow { title: "Dynamic range"; name: "dynamic-range" }
      SliderRow { title: "Hue"; name: "hue" }
      SliderRow { title: "Gamma"; name: "gamma" }
      EnumRow { title: "Vignette"; name: "shading"; fallback: "1"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
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
            { value: "2", label: "Manual" },
            { value: "3", label: "AF-C" },
            { value: "4", label: "Face" }
          ]
          value: String(root.val("focus-mode", 4))
          foreground: root.foreground
          background: Color.popups.background
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onHovered: function(index, on) {
            var tips = [root.focusModeTip(1), root.focusModeTip(2), root.focusModeTip(3), root.focusModeTip(4)]
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
      SliderRow {
        title: "Manual focus"
        name: "manu-focus"
        visible: root.focusMode === 2 && service.paramOk("manu-focus")
      }
      EnumRow { title: "Lens correction"; name: "distortion"; fallback: "1"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      EnumRow { title: "Focus speed"; name: "focus-speed"; options: [{value:"5",label:"Slow"},{value:"3",label:"Medium"},{value:"1",label:"Fast"}] }
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
        Text {
          text: "Zoom"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
        Item {
          width: parent.width
          height: zsl.height
          PanelSlider {
            id: zsl
            width: parent.width - Style.space(86)
            bar: root.bar
            minimum: 1
            maximum: 4
            step: 0.1
            integer: false
            value: Model.zoomMult(root.val("zoom", 50))
            onMoved: function(v) { zbox.text = Number(v).toFixed(1) }
            onReleased: function(v) { root.service.setParam("zoom", Model.zoomRaw(v)) }
          }
          MouseArea {
            anchors.fill: zsl
            acceptedButtons: Qt.NoButton
            onWheel: function(wheel) { wheel.accepted = true }
          }
          Rectangle {
            visible: zsl.dragging
            z: 20
            width: ztip.implicitWidth + Style.space(10)
            height: ztip.implicitHeight + Style.space(6)
            radius: Style.cornerRadius
            color: Color.tooltip.background
            border.color: Color.tooltip.border
            border.width: 1
            x: Math.max(0, Math.min(zsl.width - width, zsl.width * zsl.progress - width / 2))
            y: {
              var gy = zsl.mapToItem(scrollArea, 0, 0).y
              if (gy < height + Style.space(12)) return zsl.height + Style.space(4)
              return -height - Style.space(4)
            }
            Text {
              id: ztip
              anchors.centerIn: parent
              text: Number(zsl.liveValue).toFixed(1) + "×"
              color: Color.tooltip.text
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
          Rectangle {
            anchors.right: parent.right
            width: Style.space(78)
            height: Style.space(28)
            radius: Style.cornerRadius
            color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
            border.width: 1
            border.color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.16)
            TextInput {
              id: zbox
              anchors.fill: parent
              anchors.margins: Style.space(6)
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              text: Model.zoomMult(root.val("zoom", 50)).toFixed(1)
              onEditingFinished: {
                var n = Number(text)
                if (isNaN(n)) { text = Model.zoomMult(root.val("zoom", 50)).toFixed(1); return }
                n = Math.max(1, Math.min(4, n))
                root.service.setParam("zoom", Model.zoomRaw(n))
                text = Number(n).toFixed(1)
              }
            }
          }
        }
        Item {
          width: parent.width
          height: wMark.implicitHeight
          Text {
            id: wMark
            text: "W"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }
          Text {
            anchors.right: parent.right
            text: "T"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }
        }
      }

    }
  }

  Component {
    id: exposureTab
    Column {
      width: parent.width
      spacing: Style.space(10)
      EnumRow { title: "Exposure"; name: "exposure-mode"; options: [{value:"0",label:"Auto"},{value:"1",label:"Manual"}] }
      SliderRow {
        title: "EV"
        name: "ev"
        maximum: 16
        visible: Number(root.val("exposure-mode", 0)) === 0 && service.paramOk("ev")
        formatLive: function(v) { return Model.evLabel(v) }
        parseLive: function(s) {
          var ev = Number(String(s).replace("+", ""))
          if (isNaN(ev)) return null
          return Math.round(ev / 0.5 + 8)
        }
      }
      EnumRow { title: "Anti-flicker"; name: "anti-flicker"; options: [{value:"0",label:"Off"},{value:"1",label:"50 Hz"},{value:"2",label:"60 Hz"}] }
      SliderRow {
        title: "ISO"
        name: "iso"
        minimum: 100
        maximum: 9600
        visible: Number(root.val("exposure-mode", 0)) === 1 && service.paramOk("iso")
      }
      SliderRow {
        title: "Shutter"
        name: "shutter-us"
        minimum: 125
        maximum: 40000
        visible: Number(root.val("exposure-mode", 0)) === 1 && service.paramOk("shutter-us")
        formatLive: function(v) { return Model.shutterLabel(v) }
        parseLive: function(s) {
          s = String(s).trim()
          if (s.indexOf("1/") === 0) {
            var d = Number(s.substring(2))
            if (d > 0) return Math.round(1000000 / d)
          }
          var n = Number(s)
          return isNaN(n) ? null : n
        }
      }
      SliderRow {
        title: "Aperture"
        name: "aperture"
        maximum: 160
        visible: {
          if (!service.paramOk("aperture")) return false
          var r = root.val("aperture-range", null)
          if (r && r.max !== undefined) return Number(r.max) > Number(r.min)
          return true
        }
      }
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
      Column {
        width: parent.width
        spacing: Style.space(10)
        visible: Number(root.val("wb-mode", 1)) === 0
        SliderRow {
          title: "WB"
          name: "wb-temp"
          minimum: 2000
          maximum: 10000
          step: 50
          formatLive: function(v) { return String(Math.round(v)) }
        }
        SliderRow { title: "Tone"; name: "wb-tone"; maximum: 128 }
        Item {
          width: parent.width
          height: wbPad.height
          Canvas {
            id: wbPad
            width: Math.min(parent.width, Style.space(280))
            height: Style.space(160)
            anchors.horizontalCenter: parent.horizontalCenter
            onPaint: {
              var ctx = getContext("2d")
              var w = width, h = height
              for (var y = 0; y < h; y += 2) {
                for (var x = 0; x < w; x += 2) {
                  var t = x / Math.max(1, w - 1)
                  var u = y / Math.max(1, h - 1)
                  var hue = Math.round(30 + t * 200)
                  var sat = Math.round(35 + u * 30)
                  ctx.fillStyle = "hsl(" + hue + "," + sat + "%,55%)"
                  ctx.fillRect(x, y, 2, 2)
                }
              }
              var k = Number(root.val("wb-temp", 5500))
              var tone = Number(root.val("wb-tone", 64))
              var mx = ((k - 2000) / 8000) * w
              var my = (1 - tone / 128) * h
              ctx.strokeStyle = "#ffffff"
              ctx.lineWidth = 2
              ctx.beginPath()
              ctx.arc(mx, my, 6, 0, 6.283)
              ctx.stroke()
            }
            MouseArea {
              anchors.fill: parent
              cursorShape: Qt.CrossCursor
              onClicked: function(mouse) {
                var k = Math.round(2000 + (mouse.x / Math.max(1, parent.width)) * 8000)
                var t = Math.round(128 - (mouse.y / Math.max(1, parent.height)) * 128)
                root.service.setParam("wb-temp", Math.max(2000, Math.min(10000, k)))
                root.service.setParam("wb-tone", Math.max(0, Math.min(128, t)))
                wbPad.requestPaint()
              }
            }
          }
        }
        Connections {
          target: service
          function onValuesChanged() { if (wbPad.visible) wbPad.requestPaint() }
        }
        WbOffsetRow { title: "Red offset"; channel: "r" }
        WbOffsetRow { title: "Blue offset"; channel: "b" }
      }
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
        text: "Model     " + (service.selectedModel === "s3" ? "S3" : "S7") + "\n"
            + "Firmware  " + (service.firmware || "—") + "\n"
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
        onClicked: {
          if (typeof service.send === "function") service.send({ cmd: "tab", name: "status" })
          service.refresh()
        }
      }
    }
  }
}
