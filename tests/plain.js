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
  return s.replace(/[<>]/g, "")
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
  if (!/\/preview-[ab]\.jpg$/.test(path)) return ""
  if (path.indexOf("/yolocam/") === -1 && !/\/yolocam-[0-9]+\//.test(path)) return ""
  return "file://" + path + "?" + seq
}

const markup = '<img src="https://example.invalid/tracker">'
if (plain(markup, 200).indexOf("<") !== -1) throw new Error("plain left markup")
if (/[<>]/.test('<img src="x">') === false) throw new Error("angle check")
if (plain(markup, 200).indexOf("img src") === -1) throw new Error("plain stripped too much")
const parsed = parseLine(markup)
if (parsed.error !== "helper output was not JSON") throw new Error("parseLine echoed raw")
if (parseLine("x".repeat(MAX_EVENT + 1)).error !== "event too large") throw new Error("no size cap")
if (previewFileUrl("/etc/passwd", 1) !== "") throw new Error("passwd path")
if (previewFileUrl("/run/user/1000/yolocam/Y-CAM/preview-a.jpg", 3) !== "file:///run/user/1000/yolocam/Y-CAM/preview-a.jpg?3") throw new Error("good path")
if (previewFileUrl("/run/user/1000/yolocam/../preview-a.jpg", 1) !== "") throw new Error("dotdot")
if (previewFileUrl("https://evil/preview-a.jpg", 1) !== "") throw new Error("remote")
if (previewFileUrl("/tmp/yolocam-1000/cam/preview-b.jpg", 2).indexOf("file://") !== 0) throw new Error("tmp fallback")
console.log("ok")
