#!/usr/bin/env node
// Regression for untrusted helper/camera strings (Omarchy plugin UI safety).
// Mirrors Model.js clip / plain / parseLine / previewFileUrl.

const MAX_EVENT = 8192
const MAX_LABEL = 120

function clip(s, n) {
  s = String(s || "")
  if (s.length > n) return s.substring(0, n)
  return s
}

function plain(s, n) {
  s = clip(s, n === undefined || n === null ? MAX_LABEL : n)
  s = s.replace(/[<>]/g, "")
  s = s.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)?/g, "")
  s = s.replace(/\x1b[PX^_][^\x07\x1b]*(?:\x07|\x1b\\)?/g, "")
  s = s.replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, "")
  s = s.replace(/\x1b./g, "")
  var out = ""
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i)
    if (c < 32 || c === 127 || (c >= 0x80 && c <= 0x9f)) continue
    if (c === 0x061c || c === 0x200e || c === 0x200f || c === 0xfeff) continue
    if (c >= 0x200b && c <= 0x200d) continue
    if (c >= 0x202a && c <= 0x202e) continue
    if (c >= 0x2066 && c <= 0x2069) continue
    out += s.charAt(i)
  }
  return out
}

function parseLine(text) {
  var raw = String(text || "").trim()
  if (!raw) return null
  if (raw.length > MAX_EVENT) return { event: "error", error: "event too large" }
  try { return JSON.parse(raw) } catch (e) { return { event: "error", error: "helper output was not JSON" } }
}

function previewFileUrl(path, seq) {
  path = String(path || "")
  if (path.charAt(0) !== "/") return ""
  if (path.indexOf("..") !== -1) return ""
  if (path.indexOf("://") !== -1) return ""
  if (path.indexOf("?") !== -1 || path.indexOf("#") !== -1) return ""
  if (path.indexOf("/tmp/") === 0 || path === "/tmp") return ""
  if (path.indexOf("/var/tmp/") === 0 || path.indexOf("/dev/shm/") === 0) return ""
  if (!/\/preview-[ab]\.jpg$/.test(path)) return ""
  if (path.indexOf("/camy/") === -1) return ""
  return "file://" + path + "?" + seq
}

function safeIp(s) {
  s = plain(s, 64)
  return /^\d{1,3}(\.\d{1,3}){3}$/.test(s) ? s : ""
}

function safeVideo(s) {
  s = plain(s, 64)
  return /^\/dev\/video[0-9]+$/.test(s) ? s : ""
}

const markup = '<img src="https://example.invalid/tracker">'
if (plain(markup, 200).indexOf("<") !== -1) throw new Error("plain left markup")
if (/[<>]/.test('<img src="x">') === false) throw new Error("angle check")
if (plain(markup, 200).indexOf("img src") === -1) throw new Error("plain stripped too much")
const parsed = parseLine(markup)
if (parsed.error !== "helper output was not JSON") throw new Error("parseLine echoed raw")
if (parseLine("x".repeat(MAX_EVENT + 1)).error !== "event too large") throw new Error("no size cap")
if (previewFileUrl("/etc/passwd", 1) !== "") throw new Error("passwd path")
if (previewFileUrl("/run/user/1000/camy/Y-CAM/preview-a.jpg", 3) !== "file:///run/user/1000/camy/Y-CAM/preview-a.jpg?3") throw new Error("good path")
if (previewFileUrl("/run/user/1000/camy/../preview-a.jpg", 1) !== "") throw new Error("dotdot")
if (previewFileUrl("https://evil/preview-a.jpg", 1) !== "") throw new Error("remote")
if (previewFileUrl("/tmp/camy-1000/cam/preview-b.jpg", 2) !== "") throw new Error("tmp fallback")
if (previewFileUrl("/var/tmp/camy/cam/preview-a.jpg", 1) !== "") throw new Error("var tmp")
if (plain("\x1b[31mred\x1b[0m", 200) !== "red") throw new Error("ansi")
if (plain("\x1b]8;;https://evil.example/\x07text", 200) !== "text") throw new Error("osc")
if (plain("a\u202eb", 200) !== "ab") throw new Error("bidi")
if (plain("ok\x07bell", 200) !== "okbell") throw new Error("c0")
if (plain("a\u200b b", 200) !== "a b") throw new Error("zwsp")
if (safeIp("192.168.129.10") !== "192.168.129.10") throw new Error("ip")
if (safeIp("192.168.129.10;id") !== "") throw new Error("ip junk")
if (safeVideo("/dev/video34") !== "/dev/video34") throw new Error("v4l")
if (safeVideo("/dev/sda") !== "") throw new Error("sda")
console.log("ok")
