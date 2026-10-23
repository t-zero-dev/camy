import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

Item {
  id: root

  property var shell: null
  property var manifest: null
  property var settings: ({})
  property var service: localService
  property var bar: null
  property bool opened: false
  property bool closingFromHost: false

  property string tab: "exposure"
  property string previewOrientation: "16:9"
  property bool previewFlipV: false
  readonly property bool portraitPreview: previewOrientation === "9:16"
  property bool cursorActive: false
  property bool cameraSwitchPending: false
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
  readonly property bool present: service.usb === true
  readonly property int glyphCode: Number(service.glyphCode || 0)
  readonly property bool hasS7: glyphCode === 7 || glyphCode === 10
  readonly property bool hasS3: glyphCode === 3 || glyphCode === 10
  readonly property color s7Color: Color.accent
  readonly property color s3Color: hueShift(Color.accent, 0.22)
  readonly property color iconColor: !present ? dim : (hasS3 && !hasS7 ? tint(s3Color, service.control) : tint(s7Color, service.control))
  readonly property string heroTitle: present
    ? (service.selectedTitle || "YoloCam S7")
    : "No camera connected"
  property string connectHint: ""
  readonly property bool showPicker: service.bothCameras === true
  readonly property var tabList: {
    var all = Model.TABS
    var out = []
    for (var i = 0; i < all.length; i++) {
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

  onTabChanged: {
    if (service && typeof service.send === "function") service.send({ cmd: "tab", name: tab })
    resetScroll()
    Qt.callLater(function() {
      var g = currentGrid()
      if (g) {
        g.selectedIndex = 0
        if (typeof g.applySelection === "function") g.applySelection()
      }
    })
  }

  function open(payloadJson) {
    closingFromHost = false
    opened = true
    Qt.callLater(function() {
      if (root.opened && keyCatcher) keyCatcher.forceActiveFocus()
    })
  }

  function close() {
    closingFromHost = true
    opened = false
    closingFromHost = false
  }

  function toggle() {
    if (opened) requestClose()
    else open("{}")
  }

  function requestClose() {
    if (shell && typeof shell.hide === "function") shell.hide("io.github.t-zero-dev.camy")
    else close()
  }

  function switchPanel(_direction) {}

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
      var url = Model.previewFileUrl(path, service.previewSeq)
      if (url === "") return
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
    function onSelectedModelChanged() { root.queueCameraRelayout() }
    function onUnsupportedChanged() {
      if (root.cameraSwitchPending) root.queueCameraRelayout()
      else root.scheduleGridReflow()
    }
  }

  Timer {
    id: cameraRelayout
    interval: 200
    repeat: false
    onTriggered: {
      root.cameraSwitchPending = false
      root.rebuildCurrentTab()
    }
  }

  function val(name, fallback) { return service.valueOf(name, fallback) }

  function flashConnectHint() {
    connectHint = "Connect a camera to make changes"
    connectHintClear.restart()
  }

  function guardEdit() {
    if (present) return true
    flashConnectHint()
    return false
  }

  function setParam(name, value) {
    if (!guardEdit()) return false
    service.setParam(name, value)
    return true
  }

  function setNum(name, value) {
    return setParam(name, Math.round(value))
  }

  function setWbOffset(r, b) {
    if (!guardEdit()) return
    service.setWbOffset(r, b)
  }

  Timer {
    id: connectHintClear
    interval: 1800
    repeat: false
    onTriggered: root.connectHint = ""
  }

  property var editingBox: null

  function cameraIds() {
    var ids = []
    var list = service.cameras || []
    for (var i = 0; i < list.length; i++) {
      var m = String(list[i].model || "")
      if (m && ids.indexOf(m) < 0) ids.push(m)
    }
    if (!ids.length) {
      if (root.hasS7) ids.push("s7")
      if (root.hasS3) ids.push("s3")
    }
    return ids
  }

  function cycleCamera(dir) {
    if (!guardEdit()) return
    var ids = cameraIds()
    if (ids.length < 2) return
    var cur = String(service.selectedModel || "")
    var i = ids.indexOf(cur)
    if (i < 0) i = 0
    var n = (i + dir + ids.length) % ids.length
    service.selectModel(ids[n])
    if (typeof service.send === "function") service.send({ cmd: "tab", name: root.tab })
  }

  function jumpCamera(n) {
    if (!guardEdit()) return
    var ids = cameraIds()
    if (n < 1 || n > ids.length) return
    service.selectModel(ids[n - 1])
    if (typeof service.send === "function") service.send({ cmd: "tab", name: root.tab })
  }

  function cycleTab(dir) {
    var tabs = root.tabList
    if (!tabs.length) return
    var i = 0
    for (var t = 0; t < tabs.length; t++) if (tabs[t].value === root.tab) i = t
    root.tab = tabs[(i + dir + tabs.length) % tabs.length].value
  }

  function jumpTab(n) {
    var tabs = root.tabList
    if (n < 1 || n > tabs.length) return
    root.tab = tabs[n - 1].value
  }

  function currentGrid() {
    return tabLoader && tabLoader.item ? tabLoader.item : null
  }

  function moveCard(delta) {
    var g = currentGrid()
    if (g && typeof g.moveSelect === "function") g.moveSelect(delta)
  }

  function flickableOf(scroll) {
    if (!scroll) return null
    var item = scroll.contentItem
    if (item && item.contentY !== undefined) return item
    return null
  }

  function resetScroll() {
    var flick = flickableOf(scrollArea)
    if (flick) flick.contentY = 0
  }

  function ensureCardInView() {
    var g = currentGrid()
    var card = g && typeof g.currentItem === "function" ? g.currentItem() : null
    if (!card || !scrollArea || !panelColumn) return
    var y0 = card.mapToItem(panelColumn, 0, 0).y
    var y1 = y0 + Math.max(1, card.height || card.implicitHeight)
    var margin = Style.space(8)
    var flick = flickableOf(scrollArea)
    if (flick) {
      var viewH = flick.height
      var maxY = Math.max(0, (flick.contentHeight || panelColumn.implicitHeight) - viewH)
      if (y0 < flick.contentY + margin)
        flick.contentY = Math.max(0, Math.min(maxY, y0 - margin))
      else if (y1 > flick.contentY + viewH - margin)
        flick.contentY = Math.max(0, Math.min(maxY, y1 - viewH + margin))
      return
    }
    var bar = scrollArea.ScrollBar.vertical
    if (!bar || bar.size >= 1) return
    var contentH = Math.max(1, panelColumn.implicitHeight)
    var viewH = bar.size * contentH
    var pos = bar.position * contentH
    if (y0 < pos + margin)
      bar.position = Math.max(0, (y0 - margin) / contentH)
    else if (y1 > pos + viewH - margin)
      bar.position = Math.max(0, Math.min(1 - bar.size, (y1 - viewH + margin) / contentH))
  }

  function scheduleGridReflow() {
    Qt.callLater(function() {
      var g = currentGrid()
      if (g && typeof g.reflow === "function") g.reflow()
    })
  }

  function queueCameraRelayout() {
    cameraSwitchPending = true
    cameraRelayout.restart()
  }

  // Same effect as cycling tabs: GridLayout keeps stale rows after
  // selectModel clears unsupported and caps hides cards again.
  function rebuildCurrentTab() {
    if (!tabLoader) return
    tabLoader.active = false
    Qt.callLater(function() {
      if (!tabLoader) return
      tabLoader.active = true
      Qt.callLater(function() {
        var g = currentGrid()
        if (!g) return
        g.selectedIndex = 0
        if (typeof g.reflow === "function") g.reflow()
        else if (typeof g.applySelection === "function") g.applySelection()
        root.resetScroll()
      })
    })
  }

  function selectedCard() {
    var g = currentGrid()
    return g && typeof g.currentItem === "function" ? g.currentItem() : null
  }

  function nudgeCard(dir, big) {
    if (!guardEdit()) return
    var c = selectedCard()
    if (c && typeof c.nudge === "function") c.nudge(dir, !!big)
  }

  function activateCard() {
    if (!guardEdit()) return
    var c = selectedCard()
    if (c && typeof c.activateCard === "function") c.activateCard()
  }

  function typeCardDigit(t) {
    if (!guardEdit()) return
    var c = selectedCard()
    if (c && typeof c.typeDigit === "function") c.typeDigit(t)
  }

  function beginEdit(box) {
    if (!guardEdit()) return
    if (!box) return
    root.editingBox = box
    box.forceActiveFocus()
    box.selectAll()
  }

  function leaveField() {
    if (root.editingBox) {
      if (typeof root.editingBox.editingFinished === "function")
        root.editingBox.editingFinished()
      root.editingBox.focus = false
      root.editingBox = null
    }
    if (keyCatcher) keyCatcher.forceActiveFocus()
  }

  function refreshStatus() {
    if (!guardEdit()) return
    if (root.tab !== "status") return
    if (typeof service.send === "function") service.send({ cmd: "tab", name: "status" })
    service.refresh()
  }

  function togglePreview() {
    if (!guardEdit()) return
    service.setPreview(!service.previewOn)
  }

  Service {
    id: localService
    active: false
  }

  FloatingWindow {
    id: window
    title: "CamY"
    color: Color.popups.background
    implicitWidth: 720
    implicitHeight: 1220
    minimumSize: Qt.size(720, 720)
    visible: root.opened

    onVisibleChanged: {
      if (visible) {
        root.opened = true
        return
      }
      root.opened = false
      if (!root.closingFromHost && root.shell && typeof root.shell.hide === "function")
        root.shell.hide("io.github.t-zero-dev.camy")
    }

    Item {
      id: keyCatcher
      anchors.fill: parent
      focus: true
      Keys.priority: Keys.BeforeItem
      readonly property bool blocked: root.editingBox !== null

      Keys.onPressed: function(event) {
        var ctrl = !!(event.modifiers & Qt.ControlModifier)
        var alt = !!(event.modifiers & Qt.AltModifier)
        var shift = !!(event.modifiers & Qt.ShiftModifier)

        if (blocked) {
          if (event.key === Qt.Key_Escape) {
            root.leaveField()
            event.accepted = true
          }
          return
        }

        if (event.key === Qt.Key_Escape) {
          root.requestClose()
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Space) {
          root.togglePreview()
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
          var back = shift || event.key === Qt.Key_Backtab
          if (ctrl) root.cycleTab(back ? -1 : 1)
          else root.moveCard(back ? -1 : 1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_F5 || (ctrl && event.key === Qt.Key_R)) {
          root.refreshStatus()
          event.accepted = true
          return
        }
        if (ctrl && event.key >= Qt.Key_1 && event.key <= Qt.Key_8) {
          root.jumpTab(event.key - Qt.Key_1 + 1)
          event.accepted = true
          return
        }
        if (alt && event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
          root.jumpCamera(event.key - Qt.Key_1 + 1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_BracketLeft || event.text === "[") {
          root.cycleCamera(-1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_BracketRight || event.text === "]") {
          root.cycleCamera(1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Down || event.key === Qt.Key_J || event.text === "j") {
          root.moveCard(1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Up || event.key === Qt.Key_K || event.text === "k") {
          root.moveCard(-1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Right || event.key === Qt.Key_L || event.text === "l") {
          root.nudgeCard(1, shift)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Left || event.key === Qt.Key_H || event.text === "h") {
          root.nudgeCard(-1, shift)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
          root.activateCard()
          event.accepted = true
          return
        }
        if (!ctrl && !alt && event.text && event.text.length === 1 && event.text >= "0" && event.text <= "9") {
          root.typeCardDigit(event.text)
          event.accepted = true
        }
      }

      ColumnLayout {
        anchors.fill: parent
        anchors.margins: Style.space(16)
        spacing: Style.space(12)

          Column {
            Layout.fillWidth: true
            spacing: Style.space(4)

            CamMark {
              id: heroIcon
              pixelSize: Style.font.display
              anchors.horizontalCenter: parent.horizontalCenter
              width: implicitWidth
              height: implicitHeight
            }

            Text {
              anchors.horizontalCenter: parent.horizontalCenter
              textFormat: Text.PlainText
              text: root.heroTitle
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.weight: Font.DemiBold
            }

            Row {
              anchors.horizontalCenter: parent.horizontalCenter
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
                textFormat: Text.PlainText
                text: "X"
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
              }

              Text {
                anchors.left: parent.left
                anchors.right: parent.right
                visible: root.heroReadouts.length === 0
                textFormat: Text.PlainText
                text: heroStatus().toUpperCase()
                horizontalAlignment: Text.AlignHCenter
                color: root.heroAlarm ? Color.urgent : root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
              }

              Row {
                id: heroStats
                anchors.horizontalCenter: parent.horizontalCenter
                visible: root.heroReadouts.length > 0
                spacing: Style.space(6)
                clip: false
                Repeater {
                  model: root.heroReadouts
                  Text {
                    required property int index
                    required property var modelData
                    textFormat: Text.PlainText
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
              textFormat: Text.PlainText
              text: root.heroReadouts.length > 0 && root.heroHoverTip !== "" ? root.heroHoverTip : " "
              horizontalAlignment: Text.AlignHCenter
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

          Rectangle {
            id: previewFrame
            Layout.fillWidth: true
            Layout.preferredHeight: {
              var w = Math.max(1, window.width - Style.space(32))
              if (root.portraitPreview)
                return Math.round(Math.min(w * 0.78, 480))
              return Math.round(w * 9 / 16)
            }
            Layout.minimumHeight: 160
            width: parent.width
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
              // Bind zoom to values/pending the same way heroReadouts does, or
              // the pane keeps 1× while the slider and 2.9× readout move.
              readonly property real zoom: {
                var _vals = service.values
                var _pend = service.pending
                if (String(service.selectedModel) !== "s3") return 1
                var raw = 50
                if (_pend && _pend["zoom"] !== undefined && _pend["zoom"] !== null)
                  raw = _pend["zoom"]
                else if (_vals && _vals["zoom"] !== undefined && _vals["zoom"] !== null)
                  raw = _vals["zoom"]
                var m = Model.zoomMult(raw)
                return Math.max(1, Math.min(4, m))
              }
              readonly property real frameW: root.portraitPreview ? height : width
              readonly property real frameH: root.portraitPreview ? width : height

            Image {
              id: previewA
              anchors.centerIn: parent
              width: viewBox.frameW * viewBox.zoom
              height: viewBox.frameH * viewBox.zoom
              rotation: root.portraitPreview ? 90 : 0
              transform: Scale { origin.x: previewA.width / 2; origin.y: previewA.height / 2; yScale: root.previewFlipV ? -1 : 1 }
              fillMode: Image.PreserveAspectFit
              asynchronous: true
              cache: false
              smooth: true
              visible: root.previewShowA && status === Image.Ready
              sourceSize.width: viewBox.frameW
              sourceSize.height: viewBox.frameH
              onStatusChanged: if (status === Image.Ready) { root.previewShowA = true; root.previewReady = true }
            }

            Image {
              id: previewB
              anchors.centerIn: parent
              width: viewBox.frameW * viewBox.zoom
              height: viewBox.frameH * viewBox.zoom
              rotation: root.portraitPreview ? 90 : 0
              transform: Scale { origin.x: previewB.width / 2; origin.y: previewB.height / 2; yScale: root.previewFlipV ? -1 : 1 }
              fillMode: Image.PreserveAspectFit
              asynchronous: true
              cache: false
              smooth: true
              visible: !root.previewShowA && status === Image.Ready
              sourceSize.width: viewBox.frameW
              sourceSize.height: viewBox.frameH
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
                textFormat: Text.PlainText
                text: !present ? "No camera connected"
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
                textFormat: Text.PlainText
                text: "Lens looks covered"
                color: Color.urgent
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                font.bold: true
              }
            }

            MouseArea {
              id: previewClick
              anchors.fill: parent
              enabled: root.opened
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
                root.togglePreview()
              }
            }
            }
          }

          Text {
            Layout.fillWidth: true
            visible: service.previewOn && root.previewReady && root.tab !== "focus"
            wrapMode: Text.Wrap
            horizontalAlignment: Text.AlignHCenter
            textFormat: Text.PlainText
            text: "Click the picture to stop the live view"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Item {
            Layout.fillWidth: true
            Layout.preferredHeight: tabGroup.implicitHeight
            ButtonGroup {
              id: tabGroup
              anchors.horizontalCenter: parent.horizontalCenter
              options: root.tabList
              value: root.tab
              foreground: root.foreground
              background: Color.popups.background
              accent: Color.accent
              fontFamily: root.fontFamily
              fontSize: Style.font.caption
              onChanged: function(v) { root.tab = v }
            }
          }

          ScrollView {
            id: scrollArea
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            padding: 0
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            ScrollBar.vertical.policy: ScrollBar.AsNeeded

            Column {
              id: panelColumn
              width: previewFrame.width
              spacing: Style.spacing.panelGap
              topPadding: Style.space(8)

              Loader {
                id: tabLoader
                width: parent.width
                height: item ? item.implicitHeight : 0
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

      Rectangle {
        z: 50
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Style.space(16)
        visible: root.connectHint !== ""
        height: hintLab.implicitHeight + Style.space(12)
        radius: Style.cornerRadius
        color: Color.popups.background
        border.width: Math.max(1, Style.normalBorderWidth)
        border.color: Color.accent
        Text {
          id: hintLab
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: root.connectHint
          color: Color.accent
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          font.weight: Font.DemiBold
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
    if (!guardEdit()) return
    var size = Model.afSize(1, 0)
    var box = Model.boxAt(nx, ny, size)
    root.localAf = box
    root.afHint = ""
    service.setAfTarget(box)
  }

  function placeAfC(nx, ny) {
    if (!guardEdit()) return
    var size = Model.afSize(3, val("focus-box", 0))
    var box = Model.boxAt(nx, ny, size)
    root.localAf = box
    root.afHint = ""
    service.setAfArea(box)
  }

  function setAfcShape(v) {
    if (!guardEdit()) return
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
    var s = heroStatus()
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
    if (!present) return ""
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
    if (!present) return []
    if (service.previewBusy) return []
    if (!service.control) return []
    var rows = []
    if (hasVal("iso"))
      rows.push({ label: "ISO " + val("iso", "—"), tip: "Sensor gain" })
    if (hasVal("shutter-us"))
      rows.push({ label: Model.shutterLabel(val("shutter-us", 0)), tip: "Shutter speed." })
    if (service.paramOk("zoom") && hasVal("zoom"))
      rows.push({ label: Model.zoomLabel(val("zoom", 50)), tip: String(service.selectedModel) === "s3" ? "Digital zoom." : "Optical zoom." })
    if (hasVal("focus-mode"))
      rows.push({ label: focusModeName(val("focus-mode", 4)), tip: "Focus mode." })
    return rows
  }

  component CamMark: Item {
    property real pixelSize: Style.bar.iconFont
    readonly property bool stacked: root.present && root.hasS3 && root.hasS7
    readonly property bool s3Front: String(service.selectedModel) === "s3"
    readonly property real shift: stacked ? Math.max(2, Math.round(pixelSize * 0.2)) : 0
    readonly property real slashPad: root.present ? 0 : Math.round(pixelSize * 0.55)
    readonly property real slashAngle: -40
    readonly property real slashNudge: root.present ? 0 : pixelSize * 0.17
    readonly property real slashDx: -slashNudge * Math.cos(Math.abs(slashAngle) * Math.PI / 180)
    readonly property real slashDy: slashNudge * Math.sin(Math.abs(slashAngle) * Math.PI / 180)
    clip: false
    implicitWidth: pixelSize + shift + slashPad * 2
    implicitHeight: pixelSize + shift + slashPad * 2

    Text {
      visible: stacked
      x: parent.slashPad
      y: parent.slashPad
      z: 0
      textFormat: Text.PlainText
      text: "󰄀"
      color: root.tint(parent.s3Front ? root.s7Color : root.s3Color, root.modelControl(parent.s3Front ? "s7" : "s3"))
      font.family: root.fontFamily
      font.pixelSize: parent.pixelSize
    }
    Text {
      x: parent.slashPad + parent.shift
      y: parent.slashPad + parent.shift
      z: 1
      textFormat: Text.PlainText
      text: "󰄀"
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
    Rectangle {
      visible: !root.present
      z: 2
      width: parent.pixelSize * 1.95
      height: Math.max(4, Math.round(parent.pixelSize * 0.28))
      radius: height / 2
      color: Color.popups.background
      rotation: parent.slashAngle
      antialiasing: true
      anchors.centerIn: parent
      anchors.horizontalCenterOffset: parent.slashDx
      anchors.verticalCenterOffset: parent.slashDy
    }
    Rectangle {
      visible: !root.present
      z: 3
      width: parent.pixelSize * 1.95
      height: Math.max(2, Math.round(parent.pixelSize * 0.11))
      radius: height / 2
      color: root.dim
      rotation: parent.slashAngle
      antialiasing: true
      anchors.centerIn: parent
      anchors.horizontalCenterOffset: parent.slashDx
      anchors.verticalCenterOffset: parent.slashDy
    }
  }

  component CardGrid: GridLayout {
    id: grid
    columns: 2
    uniformCellWidths: true
    columnSpacing: Style.spacing.panelGap
    rowSpacing: Style.spacing.panelGap
    width: parent ? parent.width : 0
    Layout.fillWidth: true

    function reflow() {
      var vis = []
      var hid = []
      for (var i = 0; i < children.length; i++) {
        var c = children[i]
        if (!c) continue
        if (c.visible) vis.push(c)
        else hid.push(c)
      }
      var row = 0
      var col = 0
      for (var j = 0; j < vis.length; j++) {
        var item = vis[j]
        var force = Number(item.cardSpan || 0)
        var leftover = (col === 0 && (vis.length - j) === 1)
        var span = (force >= 2 || leftover) ? 2 : 1
        if (span === 2 && col === 1) {
          row += 1
          col = 0
        }
        item.Layout.row = row
        item.Layout.column = col
        item.Layout.columnSpan = span
        item.Layout.fillWidth = true
        item.Layout.fillHeight = true
        item.Layout.preferredWidth = span
        item.Layout.minimumWidth = 0
        item.Layout.minimumHeight = 0
        item.Layout.maximumWidth = 100000
        item.Layout.maximumHeight = 100000
        item.Layout.preferredHeight = item.implicitHeight
        if (span === 2 || col === 1) {
          row += 1
          col = 0
        } else {
          col = 1
        }
      }
      for (var h = 0; h < hid.length; h++) {
        hid[h].Layout.row = 99
        hid[h].Layout.column = 0
        hid[h].Layout.columnSpan = 1
        hid[h].Layout.preferredHeight = 0
        hid[h].Layout.maximumHeight = 0
        hid[h].Layout.minimumHeight = 0
        hid[h].Layout.preferredWidth = 0
        hid[h].Layout.maximumWidth = 0
      }
      if (typeof grid.forceLayout === "function") grid.forceLayout()
      applySelection()
    }

    function scheduleReflow() {
      Qt.callLater(function() { grid.reflow() })
    }

    onChildrenChanged: scheduleReflow()
    Component.onCompleted: scheduleReflow()

    property int selectedIndex: 0

    function visibleItems() {
      var vis = []
      for (var i = 0; i < children.length; i++) {
        if (children[i] && children[i].visible) vis.push(children[i])
      }
      return vis
    }

    function selectableItems() {
      var vis = []
      for (var i = 0; i < children.length; i++) {
        var c = children[i]
        if (c && c.visible && c.cardSelectable !== false) vis.push(c)
      }
      return vis
    }

    function applySelection() {
      var vis = selectableItems()
      if (selectedIndex >= vis.length) selectedIndex = Math.max(0, vis.length - 1)
      if (selectedIndex < 0) selectedIndex = 0
      for (var i = 0; i < children.length; i++) {
        var c = children[i]
        if (c && c.cardSelected !== undefined) c.cardSelected = false
      }
      if (vis.length && vis[selectedIndex].cardSelected !== undefined)
        vis[selectedIndex].cardSelected = true
      Qt.callLater(function() { root.ensureCardInView() })
    }

    function moveSelect(delta) {
      var vis = selectableItems()
      if (!vis.length) return
      selectedIndex = (selectedIndex + delta + vis.length) % vis.length
      applySelection()
    }

    function currentItem() {
      var vis = selectableItems()
      if (!vis.length) return null
      if (selectedIndex < 0 || selectedIndex >= vis.length) return vis[0]
      return vis[selectedIndex]
    }
  }

  component SettingCard: Rectangle {
    id: card
    default property alias extra: extraColumn.data
    property alias header: headerSlot.data
    property string title: ""
    property string icon: title !== "" ? Model.cardIcon(title) : ""
    property string valueText: ""
    property string hint: ""
    property bool hintAlarm: false
    property string hoverTip: ""
    property bool showHoverSlot: false
    property int cardSpan: 0
    property bool cardSelected: false
    property bool cardSelectable: true

    function findGroup() {
      function walk(node) {
        if (!node) return null
        if (typeof node.selectedOptionIndex === "function" && node.options !== undefined)
          return node
        var kids = node.children
        if (!kids) return null
        for (var i = 0; i < kids.length; i++) {
          var g = walk(kids[i])
          if (g) return g
        }
        return null
      }
      return walk(extraColumn)
    }

    function nudge(dir, big) {
      if (typeof sliderNudge === "function") {
        sliderNudge(dir, big)
        return
      }
      var g = findGroup()
      if (!g || !g.options || !g.options.length) return
      var i = g.selectedOptionIndex()
      if (i < 0) i = 0
      var n = i + dir
      if (n < 0) n = g.options.length - 1
      if (n >= g.options.length) n = 0
      g.changed(g.optionValue(g.options[n]))
      g.cursorIndex = n
    }

    function activateCard() {
      if (typeof sliderActivate === "function") {
        sliderActivate()
        return
      }
      var g = findGroup()
      if (g && g.options && g.options.length === 2) nudge(1, false)
    }

    function focusField() {
      if (typeof sliderFocusField === "function") sliderFocusField()
    }

    function typeDigit(t) {
      if (typeof sliderTypeDigit === "function") sliderTypeDigit(t)
    }

    onCardSelectedChanged: {
      var g = findGroup()
      if (g) g.cursorIndex = cardSelected ? g.selectedOptionIndex() : -1
    }

    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.preferredWidth: 1
    Layout.minimumWidth: 0
    implicitWidth: 160
    onVisibleChanged: if (parent && typeof parent.scheduleReflow === "function") parent.scheduleReflow()
    Component.onCompleted: if (parent && typeof parent.scheduleReflow === "function") parent.scheduleReflow()

    Binding {
      when: card.parent && card.parent.columns === undefined
      target: card
      property: "width"
      value: card.parent ? card.parent.width : card.implicitWidth
    }
    radius: Style.cornerRadius
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    border.width: cardSelected ? Math.max(2, Style.normalBorderWidth) : Style.normalBorderWidth
    border.color: cardSelected ? Color.accent : Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    implicitHeight: inner.implicitHeight + Style.space(15) * 2

    Column {
      id: inner
      x: Style.space(15)
      y: Style.space(15)
      width: Math.max(0, card.width - Style.space(15) * 2)
      spacing: Style.space(6)

      Item {
        id: titleRow
        width: parent.width
        visible: card.title !== "" || headerSlot.width > 0
        height: Math.max(titleCluster.implicitHeight, headerSlot.height)
        implicitHeight: height

        Row {
          id: titleCluster
          spacing: Style.space(6)
          anchors.verticalCenter: parent.verticalCenter
          x: headerSlot.width > 0 ? 0 : Math.max(0, (titleRow.width - implicitWidth) / 2)
          width: headerSlot.width > 0
            ? Math.max(0, titleRow.width - headerSlot.width - Style.space(8))
            : implicitWidth

          Text {
            id: glyphLab
            visible: card.icon !== ""
            textFormat: Text.PlainText
            text: card.icon
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.icon
            anchors.verticalCenter: parent.verticalCenter
          }
          Text {
            id: titleLab
            width: headerSlot.width > 0
              ? Math.max(0, parent.width - (glyphLab.visible ? glyphLab.implicitWidth + parent.spacing : 0))
              : implicitWidth
            textFormat: Text.PlainText
            text: card.title
            elide: Text.ElideRight
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.subtitle
            font.weight: Font.DemiBold
            anchors.verticalCenter: parent.verticalCenter
          }
        }
        Item {
          id: headerSlot
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          width: childrenRect.width
          height: childrenRect.height
          opacity: root.present ? 1 : 0.45
        }
      }
      Text {
        width: parent.width
        visible: card.valueText !== ""
        textFormat: Text.PlainText
        text: card.valueText
        horizontalAlignment: Text.AlignHCenter
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
      Column {
        id: extraColumn
        width: parent.width
        spacing: Style.space(6)
        opacity: root.present ? 1 : 0.45
      }
      Text {
        width: parent.width
        visible: card.showHoverSlot || card.hint !== ""
        textFormat: Text.PlainText
        text: card.hoverTip !== "" ? card.hoverTip : (card.hint !== "" ? card.hint : " ")
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        opacity: (card.hoverTip !== "" || card.hint !== "") ? 1 : 0
        color: card.hintAlarm ? Color.urgent : (card.hoverTip !== "" ? root.foreground : root.dim)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        Behavior on opacity { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
      }
    }

    MouseArea {
      anchors.fill: parent
      enabled: !root.present
      z: 20
      hoverEnabled: true
      onPressed: function(mouse) {
        root.flashConnectHint()
        mouse.accepted = true
      }
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
      textFormat: Text.PlainText
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
        if (!root.guardEdit()) return
        service.selectModel(parent.modelId)
        if (typeof service.send === "function") service.send({ cmd: "tab", name: root.tab })
      }
    }
  }

  component EnumRow: SettingCard {
    id: erow
    property string name
    property var options: []
    property var fallback: options.length ? options[0].value : ""
    visible: service.paramOk(name)

    Item {
      width: parent.width
      height: egroup.implicitHeight
      ButtonGroup {
        id: egroup
        anchors.horizontalCenter: parent.horizontalCenter
        options: erow.options
        value: String(root.val(erow.name, erow.fallback))
        foreground: root.foreground
        background: Color.popups.background
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        focusable: false
        onChanged: function(v) { root.setParam(erow.name, v) }
      }
    }
  }

  component WbOffsetRow: SettingCard {
    id: wbOff
    property string channel
    readonly property int cur: {
      var o = root.val("wb-offset", { r: 10, b: 10 })
      return Number(channel === "r" ? (o && o.r) : (o && o.b)) || 10
    }

    function sliderNudge(dir, big) {
      var step = big ? 5 : 1
      var n = wbOff.cur + dir * step
      n = Math.max(0, Math.min(20, n))
      var o = root.val("wb-offset", { r: 10, b: 10 })
      var r = Number(o && o.r) || 10
      var b = Number(o && o.b) || 10
      if (channel === "r") r = n
      else b = n
      root.setWbOffset(r, b)
    }

    header: Text {
      textFormat: Text.PlainText
      text: String(wbOff.cur)
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
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
        value: wbOff.cur
        onReleased: function(v) {
          var o = root.val("wb-offset", { r: 10, b: 10 })
          var r = Number(o && o.r) || 10
          var b = Number(o && o.b) || 10
          if (channel === "r") r = v
          else b = v
          root.setWbOffset(r, b)
        }
      }
      MouseArea {
        anchors.fill: wbSl
        acceptedButtons: Qt.NoButton
        onWheel: function(wheel) { wheel.accepted = true }
      }
    }
  }

  component SliderRow: SettingCard {
    id: srow
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

    function sliderNudge(dir, big) {
      var span = srow.maximum - srow.minimum
      var step = big ? Math.max(srow.step * 10, span / 20) : srow.step
      var n = Number(root.val(srow.name, srow.minimum)) + dir * step
      n = Math.max(srow.minimum, Math.min(srow.maximum, n))
      if (srow.integer) n = Math.round(n)
      else n = Math.round(n / srow.step) * srow.step
      root.setNum(srow.name, n)
      box.text = srow.formatLive(n)
    }

    function sliderActivate() { root.beginEdit(box) }
    function sliderFocusField() { root.beginEdit(box) }
    function sliderTypeDigit(t) {
      root.editingBox = box
      box.forceActiveFocus()
      box.text = t
      box.cursorPosition = String(t).length
    }

    header: Rectangle {
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
        horizontalAlignment: TextInput.AlignHCenter
        selectByMouse: true
        clip: true
        maximumLength: 16
        text: srow.formatLive(Number(root.val(srow.name, srow.minimum)))
        onEditingFinished: {
          var n = srow.parseLive(text)
          if (n === null || n === undefined || isNaN(n)) {
            text = srow.formatLive(Number(root.val(srow.name, srow.minimum)))
            return
          }
          n = Math.max(srow.minimum, Math.min(srow.maximum, n))
          if (srow.integer) n = Math.round(n)
          root.setNum(srow.name, n)
          text = srow.formatLive(n)
          root.editingBox = null
          if (keyCatcher) keyCatcher.forceActiveFocus()
        }
      }

      Column {
        id: arrows
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.rightMargin: Style.space(4)
        spacing: 0
        Text {
          textFormat: Text.PlainText
          text: "▴"
          color: root.dim
          font.pixelSize: Style.font.caption
          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              var n = Number(root.val(srow.name, srow.minimum)) + srow.step
              n = Math.min(srow.maximum, srow.integer ? Math.round(n) : n)
              root.setNum(srow.name, n)
              box.text = srow.formatLive(n)
            }
          }
        }
        Text {
          textFormat: Text.PlainText
          text: "▾"
          color: root.dim
          font.pixelSize: Style.font.caption
          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              var n = Number(root.val(srow.name, srow.minimum)) - srow.step
              n = Math.max(srow.minimum, srow.integer ? Math.round(n) : n)
              root.setNum(srow.name, n)
              box.text = srow.formatLive(n)
            }
          }
        }
      }
    }

    Item {
      width: parent.width
      height: sl.height
      PanelSlider {
        id: sl
        width: parent.width
        bar: root.bar
        minimum: srow.minimum
        maximum: srow.maximum
        step: srow.step
        integer: srow.integer
        value: Number(root.val(srow.name, srow.minimum))
        onMoved: function(v) {
          if (!box.activeFocus) box.text = srow.formatLive(v)
        }
        onReleased: function(v) {
          if (!root.present) {
            root.flashConnectHint()
            box.text = srow.formatLive(Number(root.val(srow.name, srow.minimum)))
            return
          }
          root.setNum(srow.name, v)
        }
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
          textFormat: Text.PlainText
          text: srow.formatLive(sl.liveValue)
          color: Color.tooltip.text
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
      Connections {
        target: service
        function onValuesChanged() {
          if (sl.dragging || box.activeFocus) return
          box.text = srow.formatLive(Number(root.val(srow.name, srow.minimum)))
        }
      }
    }
  }

  Component {
    id: videoTab
    CardGrid {
      SettingCard {
        title: "Orientation"
        Item {
          width: parent.width
          height: orientGroup.implicitHeight
          ButtonGroup {
            id: orientGroup
            anchors.horizontalCenter: parent.horizontalCenter
            options: [{ value: "16:9", label: "16:9" }, { value: "9:16", label: "9:16" }]
            value: root.previewOrientation
            foreground: root.foreground
            background: Color.popups.background
            fontFamily: root.fontFamily
            fontSize: Style.font.caption
            onChanged: function(v) { if (root.guardEdit()) root.previewOrientation = v }
          }
        }
      }
      EnumRow { title: "Frame rate"; name: "frame-rate"; options: [{value:"25",label:"25"},{value:"30",label:"30"},{value:"50",label:"50"},{value:"60",label:"60"}] }
      EnumRow { title: "Resolution"; name: "resolution"; options: [{value:"0",label:"Auto"},{value:"1",label:"4K"},{value:"2",label:"1080p"},{value:"3",label:"720p"}] }
      SettingCard {
        visible: String(service.selectedModel) === "s7"
        title: "Vertical flip"
        Item {
          width: parent.width
          height: flipGroup.implicitHeight
          ButtonGroup {
            id: flipGroup
            anchors.horizontalCenter: parent.horizontalCenter
            options: [{ value: "0", label: "Off" }, { value: "1", label: "On" }]
            value: root.previewFlipV ? "1" : "0"
            foreground: root.foreground
            background: Color.popups.background
            fontFamily: root.fontFamily
            fontSize: Style.font.caption
            onChanged: function(v) { if (root.guardEdit()) root.previewFlipV = v === "1" }
          }
        }
      }
      EnumRow { title: "HDMI color"; name: "hdmi-color"; options: [{value:"0",label:"Auto"},{value:"1",label:"RGB"},{value:"2",label:"YUV422"}] }
      EnumRow { title: "HDMI output"; name: "hdmi-format"; options: [{value:"0",label:"Auto"},{value:"1",label:"4K"},{value:"2",label:"1080p"},{value:"3",label:"720p"}] }
    }
  }

  Component {
    id: imageTab
    CardGrid {
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
    SettingCard {
      title: "AF area"
      Item {
        width: parent.width
        height: afcGroup.implicitHeight
        ButtonGroup {
          id: afcGroup
          anchors.horizontalCenter: parent.horizontalCenter
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
  }

  Component {
    id: focusTab
    CardGrid {
      SettingCard {
        title: "Focus mode"
        showHoverSlot: true
        hoverTip: root.focusHoverTip
        hint: {
          if (!(root.focusMode === 1 || root.focusMode === 3)) return ""
          if (root.afHint !== "") return root.afHint
          if (root.focusMode === 1) return "Click the preview to set the focus point."
          if (root.afcCustom) return "Drag on the preview to draw the AF box. Drag the box to move it."
          return "Click the preview to place the AF area."
        }
        hintAlarm: root.afHint.indexOf("Stay") === 0
        Item {
          width: parent.width
          height: focusGroup.implicitHeight
          ButtonGroup {
            id: focusGroup
            anchors.horizontalCenter: parent.horizontalCenter
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
              if (!root.setParam("focus-mode", v)) return
              var mode = Number(v)
              if (mode === 3 && !root.afcCustom) {
                var box = Model.afRegion(3, root.val("focus-box", 0))
                if (box) {
                  root.localAf = box
                  service.setAfArea(box)
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
        }
      }
      EnumRow { title: "Focus speed"; name: "focus-speed"; options: [{value:"5",label:"Slow"},{value:"3",label:"Medium"},{value:"1",label:"Fast"}] }
      Loader {
        property int cardSpan: 2
        property bool cardSelected: false
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredWidth: 1
        Layout.minimumWidth: 0
        Layout.preferredHeight: item ? item.implicitHeight : 0
        active: root.focusMode === 3
        visible: active
        sourceComponent: afcAreaControls
        onCardSelectedChanged: if (item && item.cardSelected !== undefined) item.cardSelected = cardSelected
        onItemChanged: if (item && item.cardSelected !== undefined) item.cardSelected = cardSelected
        function nudge(dir, big) { if (item && typeof item.nudge === "function") item.nudge(dir, big) }
        function activateCard() { if (item && typeof item.activateCard === "function") item.activateCard() }
        onVisibleChanged: if (parent && typeof parent.scheduleReflow === "function") parent.scheduleReflow()
        onActiveChanged: if (parent && typeof parent.scheduleReflow === "function") parent.scheduleReflow()
      }
      SliderRow {
        title: "Manual focus"
        name: "manu-focus"
        cardSpan: 2
        visible: root.focusMode === 2 && service.paramOk("manu-focus")
      }
      EnumRow { title: "Lens correction"; name: "distortion"; fallback: "1"; cardSpan: 2; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
    }
  }

  Component {
    id: zoomTab
    CardGrid {
      SettingCard {
        Layout.columnSpan: 2
        cardSpan: 2
        title: "Zoom"

        function sliderNudge(dir, big) {
          var step = big ? 0.5 : 0.1
          var n = Model.zoomMult(root.val("zoom", 50)) + dir * step
          n = Math.max(1, Math.min(4, n))
          n = Math.round(n * 10) / 10
          root.setParam("zoom", Model.zoomRaw(n))
          zbox.text = Number(n).toFixed(1)
        }
        function sliderActivate() { root.beginEdit(zbox) }
        function sliderFocusField() { root.beginEdit(zbox) }
        function sliderTypeDigit(t) {
          root.editingBox = zbox
          zbox.forceActiveFocus()
          zbox.text = t
          zbox.cursorPosition = String(t).length
        }

        header: Rectangle {
          id: zstep
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
            horizontalAlignment: TextInput.AlignHCenter
            maximumLength: 16
            text: Model.zoomMult(root.val("zoom", 50)).toFixed(1)
            onEditingFinished: {
              var n = Number(text)
              if (isNaN(n)) { text = Model.zoomMult(root.val("zoom", 50)).toFixed(1); return }
              n = Math.max(1, Math.min(4, n))
              root.setParam("zoom", Model.zoomRaw(n))
              text = Number(n).toFixed(1)
              root.editingBox = null
              if (keyCatcher) keyCatcher.forceActiveFocus()
            }
          }
        }
        Item {
          width: parent.width
          height: zsl.height
          PanelSlider {
            id: zsl
            anchors.left: wMark.right
            anchors.right: tMark.left
            anchors.leftMargin: Style.space(6)
            anchors.rightMargin: Style.space(6)
            bar: root.bar
            minimum: 1
            maximum: 4
            step: 0.1
            integer: false
            value: Model.zoomMult(root.val("zoom", 50))
            onMoved: function(v) { zbox.text = Number(v).toFixed(1) }
            onReleased: function(v) {
              if (!root.present) {
                root.flashConnectHint()
                zbox.text = Model.zoomMult(root.val("zoom", 50)).toFixed(1)
                return
              }
              root.setParam("zoom", Model.zoomRaw(v))
            }
          }
          Text {
            id: wMark
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: "W"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.weight: Font.DemiBold
          }
          Text {
            id: tMark
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: "T"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.weight: Font.DemiBold
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
            x: Math.max(0, Math.min(parent.width - width, zsl.x + zsl.width * zsl.progress - width / 2))
            y: {
              var gy = zsl.mapToItem(scrollArea, 0, 0).y
              if (gy < height + Style.space(12)) return zsl.height + Style.space(4)
              return -height - Style.space(4)
            }
            Text {
              id: ztip
              anchors.centerIn: parent
              textFormat: Text.PlainText
              text: Number(zsl.liveValue).toFixed(1) + "×"
              color: Color.tooltip.text
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
      }
    }
  }

  Component {
    id: exposureTab
    CardGrid {
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
      EnumRow { title: "HDR"; name: "hdr"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      EnumRow { title: "Lock"; name: "exposure-lock"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
    }
  }

  Component {
    id: wbTab
    CardGrid {
      EnumRow { title: "White balance"; name: "wb-mode"; options: [{value:"1",label:"Auto"},{value:"0",label:"Manual"}] }
      SliderRow {
        title: "WB"
        name: "wb-temp"
        minimum: 2000
        maximum: 10000
        step: 50
        visible: Number(root.val("wb-mode", 1)) === 0
        formatLive: function(v) { return String(Math.round(v)) }
      }
      SliderRow {
        title: "Tone"
        name: "wb-tone"
        maximum: 128
        visible: Number(root.val("wb-mode", 1)) === 0
      }
      SettingCard {
        title: "Picker"
        visible: Number(root.val("wb-mode", 1)) === 0
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
                root.setParam("wb-temp", Math.max(2000, Math.min(10000, k)))
                root.setParam("wb-tone", Math.max(0, Math.min(128, t)))
                wbPad.requestPaint()
              }
            }
            Connections {
              target: service
              function onValuesChanged() { if (wbPad.visible) wbPad.requestPaint() }
            }
          }
        }
      }
      WbOffsetRow {
        title: "Red offset"
        channel: "r"
        visible: Number(root.val("wb-mode", 1)) === 0
      }
      WbOffsetRow {
        title: "Blue offset"
        channel: "b"
        visible: Number(root.val("wb-mode", 1)) === 0
      }
      EnumRow { title: "WB lock"; name: "wb-lock"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
    }
  }

  Component {
    id: audioTab
    CardGrid {
      SliderRow { title: "Volume"; name: "audio-volume" }
      SliderRow { title: "Delay (ms)"; name: "audio-delay"; maximum: 400 }
      EnumRow { title: "Input"; name: "mic-type"; options: [{value:"0",label:"Line"},{value:"1",label:"Mic"}] }
      EnumRow { title: "Auto gain"; name: "audio-agc"; options: [{value:"0",label:"Off"},{value:"1",label:"On"}] }
      EnumRow { title: "Noise cancellation"; name: "audio-denoise"; cardSpan: 2; options: [{value:"0",label:"Off"},{value:"1",label:"Weak"},{value:"2",label:"Middle"},{value:"3",label:"Strong"}] }
    }
  }

  Component {
    id: statusTab
    CardGrid {
      SettingCard {
        title: "Model"
        cardSelectable: false
        valueText: !root.present ? "—" : (service.selectedModel === "s3" ? "S3" : "S7")
      }
      SettingCard {
        title: "Firmware"
        cardSelectable: false
        valueText: service.firmware || "—"
      }
      SettingCard {
        title: "Serial"
        cardSelectable: false
        valueText: service.serial || "—"
      }
      SettingCard {
        title: "Control"
        cardSelectable: false
        valueText: service.cameraIp || "—"
      }
      SettingCard {
        title: "UVC"
        cardSelectable: false
        valueText: service.videoDevice || "—"
      }
      SettingCard {
        title: "Lens"
        cardSelectable: false
        valueText: root.val("lens-status", "—") === 1 || root.val("lens-status", "—") === "1" ? "mounted" : String(root.val("lens-status", "—"))
      }
      Item {
        property bool cardSelected: false
        property bool cardSelectable: true
        function activateCard() { root.refreshStatus() }
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.columnSpan: 2
        Layout.preferredWidth: 1
        implicitHeight: refreshBtn.height
        height: refreshBtn.height
        Rectangle {
          anchors.fill: parent
          visible: parent.cardSelected
          color: "transparent"
          border.width: Math.max(2, Style.normalBorderWidth)
          border.color: Color.accent
          radius: Style.cornerRadius
        }
        PanelActionButton {
          id: refreshBtn
          anchors.horizontalCenter: parent.horizontalCenter
          iconText: "󰑐"
          tooltipText: "Refresh from camera"
          foreground: root.foreground
          fontFamily: root.fontFamily
          onClicked: root.refreshStatus()
        }
      }
    }
  }
}
