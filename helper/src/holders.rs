//! Who has a V4L2 node open.
//!
//! Naming is a heuristic: `/proc` comm/cmdline of the holder and its ancestors,
//! plus Hyprland window class/title for those pids. A browser capture utility
//! does not know which tab opened the camera; a background Meet tab therefore
//! names as Chromium unless a related window class/title says otherwise.

use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

const MAX_HYPR_JSON: usize = 1_048_576;
const MAX_COMM: usize = 64;
const MAX_CMDLINE: usize = 4096;
const MAX_TITLE: usize = 256;

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Holder {
    pub pid: u32,
    pub comm: String,
    pub name: String,
    pub kind: &'static str,
}

pub fn describe(device: &str) -> String {
    summarize(device).name
}

pub fn summarize(device: &str) -> Holder {
    let mut found = list(device);
    if found.is_empty() {
        return Holder {
            pid: 0,
            comm: String::new(),
            name: "another app".into(),
            kind: "other",
        };
    }
    found.sort_by_key(|h| kind_rank(h.kind));
    found.remove(0)
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "meet" | "zoom" | "obs" => 0,
        _ => 1,
    }
}

pub fn list(device: &str) -> Vec<Holder> {
    let want = match fs::metadata(device) {
        Ok(m) => m.rdev(),
        Err(_) => return Vec::new(),
    };
    let self_pid = std::process::id();
    let windows = hypr_windows();
    let mut out = Vec::new();
    let Ok(proc) = fs::read_dir("/proc") else {
        return out;
    };
    for ent in proc.flatten() {
        let name = ent.file_name();
        let pid: u32 = match name.to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        if pid == self_pid {
            continue;
        }
        if !holds_device(&ent.path().join("fd"), device, want) {
            continue;
        }
        let comm = proc_comm(pid);
        let cmdline = proc_cmdline(pid);
        if is_self_helper(&comm, &cmdline) {
            continue;
        }
        let related = related_text(pid, &windows);
        let (label, kind) = classify(&comm, &cmdline, &related);
        out.push(Holder {
            pid,
            comm,
            name: label,
            kind,
        });
    }
    out.sort_by_key(|h| h.pid);
    out.dedup_by(|a, b| a.name == b.name);
    out
}

fn holds_device(fd_dir: &Path, device: &str, want: u64) -> bool {
    let Ok(fds) = fs::read_dir(fd_dir) else {
        return false;
    };
    for fd in fds.flatten() {
        let Ok(link) = fs::read_link(fd.path()) else {
            continue;
        };
        if link == Path::new(device) {
            return true;
        }
        if let Ok(meta) = fs::metadata(fd.path()) {
            if meta.rdev() == want {
                return true;
            }
        }
    }
    false
}

fn proc_comm(pid: u32) -> String {
    let mut f = match fs::File::open(format!("/proc/{pid}/comm")) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut buf = [0u8; 64];
    let n = f.read(&mut buf).unwrap_or(0);
    clip(
        &String::from_utf8_lossy(&buf[..n]),
        MAX_COMM,
    )
}

fn proc_cmdline(pid: u32) -> String {
    fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|b| {
            let s = String::from_utf8_lossy(&b[..b.len().min(MAX_CMDLINE)]).replace('\0', " ");
            clip(&s, MAX_CMDLINE)
        })
        .unwrap_or_default()
}

fn proc_ppid(pid: u32) -> Option<u32> {
    let mut f = fs::File::open(format!("/proc/{pid}/status")).ok()?;
    let mut buf = [0u8; 4096];
    let n = f.read(&mut buf).ok()?;
    let status = String::from_utf8_lossy(&buf[..n]);
    status.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|rest| rest.trim().parse().ok())
    })
}

fn is_session_root(comm: &str) -> bool {
    matches!(
        comm,
        "systemd" | "init" | "hyprland" | "Hyprland" | "quickshell" | "omarchy-shell"
    )
}

fn ancestor_pids(start: u32) -> Vec<u32> {
    let mut pid = start;
    let mut out = vec![pid];
    for _ in 0..12 {
        let Some(parent) = proc_ppid(pid) else {
            break;
        };
        if parent <= 1 || out.contains(&parent) {
            break;
        }
        if is_session_root(&proc_comm(parent)) {
            break;
        }
        out.push(parent);
        pid = parent;
    }
    out
}

struct HyprWin {
    pid: u32,
    text: String,
}

fn related_text(pid: u32, windows: &[HyprWin]) -> String {
    let tree = ancestor_pids(pid);
    let mut blob = String::new();
    for p in &tree {
        blob.push(' ');
        blob.push_str(&proc_comm(*p));
        blob.push(' ');
        blob.push_str(&proc_cmdline(*p));
    }
    for w in windows {
        if tree.contains(&w.pid) {
            blob.push(' ');
            blob.push_str(&w.text);
        }
    }
    blob
}

fn hypr_windows() -> Vec<HyprWin> {
    let mut child = match Command::new("/usr/bin/hyprctl")
        .args(["clients", "-j"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut buf = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = Read::take(&mut stdout, MAX_HYPR_JSON as u64 + 1).read_to_end(&mut buf);
    }
    let _ = child.kill();
    let _ = child.wait();
    if buf.len() > MAX_HYPR_JSON {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|c| {
            let pid = c.get("pid").and_then(serde_json::Value::as_u64)? as u32;
            let mut text = String::new();
            for key in ["title", "class", "initialTitle", "initialClass"] {
                if let Some(s) = c.get(key).and_then(serde_json::Value::as_str) {
                    text.push(' ');
                    text.push_str(&clip(s, MAX_TITLE));
                }
            }
            Some(HyprWin { pid, text })
        })
        .collect()
}

fn is_self_helper(comm: &str, cmdline: &str) -> bool {
    let hay = format!("{comm} {cmdline}").to_ascii_lowercase();
    hay.contains("yolocam")
}

fn is_meet(hay: &str) -> bool {
    hay.contains("meet.google.com")
        || hay.contains("chrome-meet.google")
        || hay.contains("google meet")
        || hay.contains("meet \u{2013}")
        || hay.contains("meet \u{2014}")
        || hay.contains("meet - ")
}

pub(crate) fn classify(comm: &str, cmdline: &str, extra: &str) -> (String, &'static str) {
    if is_self_helper(comm, cmdline) {
        return ("YoloCam helper".into(), "other");
    }
    let hay = format!("{comm} {cmdline} {extra}").to_ascii_lowercase();
    if hay.contains("obs-studio")
        || comm == "obs"
        || hay.contains("/obs ")
        || hay.contains("class:obs")
    {
        return ("OBS Studio".into(), "obs");
    }
    if hay.contains("zoom.us")
        || hay.contains("zoom meeting")
        || comm == "zoom"
        || hay.contains("us.zoom.zoom")
        || hay.contains("zoom-client")
    {
        return ("Zoom".into(), "zoom");
    }
    if is_meet(&hay) {
        return ("Google Meet".into(), "meet");
    }
    if hay.contains("ms-teams")
        || hay.contains("teams.microsoft")
        || hay.contains("microsoft teams")
    {
        return ("Microsoft Teams".into(), "other");
    }
    if hay.contains("discord") {
        return ("Discord".into(), "other");
    }
    if hay.contains("slack") {
        return ("Slack".into(), "other");
    }
    if hay.contains("firefox") {
        return ("Firefox".into(), "other");
    }
    if hay.contains("microsoft-edge") || comm.contains("msedge") {
        return ("Microsoft Edge".into(), "other");
    }
    if hay.contains("brave") {
        return ("Brave".into(), "other");
    }
    if hay.contains("chromium") {
        return ("Chromium".into(), "other");
    }
    if hay.contains("google-chrome") || hay.contains("/chrome") || comm == "chrome" {
        return ("Google Chrome".into(), "other");
    }
    if hay.contains("signal-desktop") || comm == "signal-desktop" {
        return ("Signal".into(), "other");
    }
    if !comm.is_empty() {
        return (printable(&clip(comm, MAX_COMM)), "other");
    }
    ("another app".into(), "other")
}

fn printable(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let u = *c as u32;
            !c.is_control()
                && u != 0x7f
                && u != 0x061c
                && u != 0xfeff
                && !(0x200b..=0x200f).contains(&u)
                && !(0x202a..=0x202e).contains(&u)
                && !(0x2066..=0x2069).contains(&u)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn meet_from_hypr_class() {
        let (name, kind) = classify(
            "chrome",
            "/opt/google/chrome/chrome --type=utility",
            "chrome-meet.google.com__-Default Meet - call - Google Chrome",
        );
        assert_eq!(name, "Google Meet");
        assert_eq!(kind, "meet");
    }

    #[test]
    fn meet_from_chromium_en_dash_title() {
        let (name, kind) = classify(
            "chromium",
            "/usr/lib/chromium/chromium --type=utility --utility-sub-type=video_capture.mojom.VideoCaptureService",
            "Meet \u{2013} gsb-ygmi-tnq - Chromium",
        );
        assert_eq!(name, "Google Meet");
        assert_eq!(kind, "meet");
    }

    #[test]
    fn yolocam_in_other_window_does_not_hide_chromium() {
        let (name, kind) = classify(
            "chromium",
            "/usr/lib/chromium/chromium --type=utility",
            "Yolocam S7 Omarchy Quickshell Native Configurator - grok",
        );
        assert_eq!(name, "Chromium");
        assert_eq!(kind, "other");
    }

    #[test]
    fn zoom_from_title() {
        let (name, kind) = classify("zoom", "/usr/bin/zoom", "Zoom Meeting");
        assert_eq!(name, "Zoom");
        assert_eq!(kind, "zoom");
    }

    #[test]
    fn obs_from_comm() {
        let (name, kind) = classify("obs", "/usr/bin/obs", "");
        assert_eq!(name, "OBS Studio");
        assert_eq!(kind, "obs");
    }

    #[test]
    fn unknown_comm_drops_bidi() {
        let (name, kind) = classify("cam\u{202e}evil", "", "");
        assert_eq!(name, "camevil");
        assert_eq!(kind, "other");
    }

    #[test]
    fn chrome_without_meet_stays_chrome() {
        let (name, kind) = classify(
            "chrome",
            "/opt/google/chrome/chrome --type=utility --utility-sub-type=video_capture",
            "Gmail - Google Chrome",
        );
        assert_eq!(name, "Google Chrome");
        assert_eq!(kind, "other");
    }

    #[test]
    fn capture_service_without_meet_title_is_chromium() {
        let (name, kind) = classify(
            "chromium",
            "/usr/lib/chromium/chromium --type=utility --utility-sub-type=video_capture.mojom.VideoCaptureService",
            "GitHub - t4t5/omarchy-caldir-widget - Chromium chrome-www.blueletterbible.org__-Default",
        );
        assert_eq!(name, "Chromium");
        assert_eq!(kind, "other");
    }

    #[test]
    fn live_holder_is_named_when_camera_is_open() {
        let path = "/dev/video34";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let listed = super::list(path);
        if listed.is_empty() {
            return;
        }
        let h = super::summarize(path);
        assert_ne!(h.name, "another app", "{h:?}");
        assert!(!h.name.is_empty());
    }
}
