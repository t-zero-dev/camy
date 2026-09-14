//! Named camera parameters mapped onto Yunxi command IDs.

use crate::error::{Error, Result};
use crate::pb::{Body, Message};

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub cid: u32,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: Kind,
    pub set_ok: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Int { min: i32, max: i32 },
    Enum(&'static [(&'static str, u32)]),
    Info,
}

pub const PARAMS: &[Param] = &[
    Param { name: "firmware", cid: 1, label: "Firmware", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "frame-rate", cid: 5, label: "Frame rate", group: "video", kind: Kind::Enum(&[("25", 25), ("30", 30), ("50", 50), ("60", 60)]), set_ok: true },
    Param { name: "resolution", cid: 6, label: "Resolution", group: "video", kind: Kind::Enum(&[("auto", 0), ("4k", 1), ("1080p", 2), ("720p", 3)]), set_ok: true },
    Param { name: "brightness", cid: 7, label: "Brightness", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "dynamic-range", cid: 102, label: "Dynamic range", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "sharpness", cid: 8, label: "Sharpness", group: "image", kind: Kind::Int { min: -10, max: 30 }, set_ok: true },
    Param { name: "contrast", cid: 9, label: "Contrast", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "saturation", cid: 10, label: "Saturation", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "hue", cid: 11, label: "Hue", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "gamma", cid: 12, label: "Gamma", group: "image", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "distortion", cid: 26, label: "Distortion correction", group: "image", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "shading", cid: 27, label: "Vignette correction", group: "image", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "zoom", cid: 13, label: "Zoom", group: "zoom", kind: Kind::Int { min: 50, max: 98 }, set_ok: true },
    Param { name: "exposure-mode", cid: 14, label: "Exposure", group: "exposure", kind: Kind::Enum(&[("auto", 0), ("manual", 1)]), set_ok: true },
    Param { name: "ev", cid: 15, label: "EV", group: "exposure", kind: Kind::Int { min: 0, max: 16 }, set_ok: true },
    Param { name: "anti-flicker", cid: 16, label: "Anti-flicker", group: "exposure", kind: Kind::Enum(&[("off", 0), ("50hz", 1), ("60hz", 2)]), set_ok: true },
    Param { name: "iso", cid: 17, label: "ISO", group: "exposure", kind: Kind::Int { min: 100, max: 12800 }, set_ok: true },
    Param { name: "shutter-us", cid: 18, label: "Shutter (µs)", group: "exposure", kind: Kind::Int { min: 125, max: 40000 }, set_ok: true },
    Param { name: "aperture", cid: 20, label: "Aperture", group: "exposure", kind: Kind::Int { min: 0, max: 160 }, set_ok: true },
    Param { name: "wb-mode", cid: 21, label: "White balance", group: "wb", kind: Kind::Enum(&[("manual", 0), ("auto", 1)]), set_ok: true },
    Param { name: "wb-temp", cid: 22, label: "Color temp", group: "wb", kind: Kind::Int { min: 2000, max: 10000 }, set_ok: true },
    Param { name: "wb-tone", cid: 105, label: "Tone", group: "wb", kind: Kind::Int { min: 0, max: 128 }, set_ok: true },
    Param { name: "focus-mode", cid: 23, label: "Focus", group: "focus", kind: Kind::Enum(&[("afs", 1), ("manual", 2), ("afc", 3), ("face", 4)]), set_ok: true },
    Param { name: "manu-focus", cid: 111, label: "Manual focus", group: "focus", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "aperture-range", cid: 31, label: "Aperture range", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "shutter-range", cid: 32, label: "Shutter range", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "wb-offset", cid: 33, label: "WB offset", group: "wb", kind: Kind::Info, set_ok: false },
    Param { name: "focus-box", cid: 38, label: "Focus box", group: "focus", kind: Kind::Enum(&[("large", 0), ("medium", 1), ("small", 2)]), set_ok: true },
    Param { name: "caf", cid: 42, label: "Continuous AF", group: "focus", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "focus-speed", cid: 47, label: "Focus speed", group: "focus", kind: Kind::Enum(&[("fast", 1), ("medium", 3), ("slow", 5)]), set_ok: true },
    Param { name: "lens-status", cid: 48, label: "Lens", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "usb-version", cid: 60, label: "USB", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "hdmi-format", cid: 61, label: "HDMI format", group: "video", kind: Kind::Enum(&[("auto", 0), ("4k", 1), ("1080p", 2), ("720p", 3)]), set_ok: true },
    Param { name: "lens-detail", cid: 63, label: "Lens detail", group: "status", kind: Kind::Info, set_ok: false },
    Param { name: "hdmi-color", cid: 65, label: "HDMI color", group: "video", kind: Kind::Enum(&[("auto", 0), ("rgb", 1), ("yuv422", 2)]), set_ok: true },
    Param { name: "audio-volume", cid: 73, label: "Audio volume", group: "audio", kind: Kind::Int { min: 0, max: 100 }, set_ok: true },
    Param { name: "audio-denoise", cid: 75, label: "Denoise", group: "audio", kind: Kind::Enum(&[("off", 0), ("weak", 1), ("middle", 2), ("strong", 3)]), set_ok: true },
    Param { name: "audio-reverb", cid: 76, label: "Reverb", group: "audio", kind: Kind::Enum(&[("off", 0), ("weak", 1), ("middle", 2), ("strong", 3)]), set_ok: true },
    Param { name: "audio-delay", cid: 77, label: "Audio delay ms", group: "audio", kind: Kind::Int { min: 0, max: 400 }, set_ok: true },
    Param { name: "audio-agc", cid: 78, label: "Auto gain", group: "audio", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "mic-type", cid: 79, label: "Mic type", group: "audio", kind: Kind::Enum(&[("line", 0), ("mic", 1)]), set_ok: true },
    Param { name: "clut", cid: 94, label: "Color LUT", group: "image", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "clut-mix", cid: 96, label: "LUT mix", group: "image", kind: Kind::Int { min: 0, max: 255 }, set_ok: true },
    Param { name: "hdr", cid: 98, label: "HDR", group: "exposure", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "wb-lock", cid: 104, label: "WB lock", group: "wb", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "exposure-lock", cid: 106, label: "Exposure lock", group: "exposure", kind: Kind::Enum(&[("off", 0), ("on", 1)]), set_ok: true },
    Param { name: "zoom-speed", cid: 112, label: "Zoom speed", group: "zoom", kind: Kind::Int { min: 1, max: 5 }, set_ok: true },
];

/// Enough for the first paint. Stay well under the ~25-request crash threshold.
pub const WARM: &[&str] = &[
    "iso",
    "shutter-us",
    "zoom",
    "focus-mode",
    "firmware",
    "focus-box",
    "exposure-mode",
    "ev",
    "wb-mode",
    "wb-temp",
    "wb-tone",
    "brightness",
    "dynamic-range",
    "manu-focus",
    "contrast",
    "distortion",
    "shading",
];

pub fn is_warm(name: &str) -> bool {
    WARM.contains(&name)
}

pub fn tab_group(tab: &str) -> &'static str {
    match tab {
        "video" => "video",
        "image" => "image",
        "focus" => "focus",
        "zoom" => "zoom",
        "wb" => "wb",
        "audio" => "audio",
        "status" => "status",
        _ => "exposure",
    }
}

/// Indices into `PARAMS`: hero/WARM first, then `group`, then the rest.
pub fn fill_order(group: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut seen = vec![false; PARAMS.len()];
    for name in WARM {
        if let Some(i) = PARAMS.iter().position(|p| p.name == *name) {
            if !seen[i] {
                seen[i] = true;
                out.push(i);
            }
        }
    }
    for (i, p) in PARAMS.iter().enumerate() {
        if p.group == group && !seen[i] {
            seen[i] = true;
            out.push(i);
        }
    }
    for (i, _) in PARAMS.iter().enumerate() {
        if !seen[i] {
            out.push(i);
        }
    }
    out
}

/// Panel Refresh: current tab only. Skip bulky Info (`lens-detail`).
pub fn refresh_order(group: &str) -> Vec<usize> {
    PARAMS
        .iter()
        .enumerate()
        .filter(|(_, p)| p.group == group)
        .filter(|(_, p)| p.name != "lens-detail")
        .filter(|(_, p)| group == "status" || !matches!(p.kind, Kind::Info))
        .map(|(i, _)| i)
        .collect()
}

pub fn prefer_group(order: &mut Vec<usize>, filled: usize, group: &str) {
    if filled >= order.len() {
        return;
    }
    let rest: Vec<usize> = order.drain(filled..).collect();
    let mut want = Vec::new();
    let mut other = Vec::new();
    for i in rest {
        if PARAMS.get(i).map(|p| p.group) == Some(group) {
            want.push(i);
        } else {
            other.push(i);
        }
    }
    order.extend(want);
    order.extend(other);
}

pub const HEARTBEAT_CID: u32 = 215;
pub const FACE_BOX_CID: u32 = 59;
pub const FOCUS_TARGET_CID: u32 = 24;
pub const FOCUS_AREA_CID: u32 = 34;

/// Keep the most face-like box (drops a raised hand next to a face).
pub fn pick_primary_face(rects: &[crate::pb::RectF]) -> Vec<crate::pb::RectF> {
    if rects.len() <= 1 {
        return rects.to_vec();
    }
    let best = rects
        .iter()
        .max_by(|a, b| face_score(a).partial_cmp(&face_score(b)).unwrap_or(std::cmp::Ordering::Equal));
    best.copied().into_iter().collect()
}

fn face_score(r: &crate::pb::RectF) -> f32 {
    let area = r.w * r.h;
    if area <= 0.0 {
        return 0.0;
    }
    let aspect = r.w / r.h.max(0.001);
    let aspect_term = 1.0 - (aspect - 0.8).abs().min(1.0);
    let cy = r.y + r.h * 0.5;
    let vertical = 1.15 - cy;
    area * (0.35 + 0.65 * aspect_term.max(0.0)) * vertical.max(0.25)
}

/// AF-S point (~4–10%) or AF-C area (~50–92%) as a fraction of the frame.
pub fn af_size(mode: u32, level: u32) -> Option<f32> {
    match mode {
        1 => Some(match level {
            1 => 0.07,
            2 => 0.045,
            _ => 0.10,
        }),
        3 => Some(match level {
            1 => 0.72,
            2 => 0.50,
            _ => 0.92,
        }),
        _ => None,
    }
}

/// Center AF region for AF-S (small point) and AF-C (wide area).
pub fn af_region(mode: u32, level: u32) -> Option<crate::pb::RectF> {
    let size = af_size(mode, level)?;
    Some(box_at(0.5, 0.5, size))
}

/// Box of `size`×`size` centered on `(cx, cy)`, shifted to stay in frame.
pub fn box_at(cx: f32, cy: f32, size: f32) -> crate::pb::RectF {
    let size = size.clamp(0.03, 1.0);
    let x = (cx - size * 0.5).clamp(0.0, 1.0 - size);
    let y = (cy - size * 0.5).clamp(0.0, 1.0 - size);
    crate::pb::RectF { x, y, w: size, h: size }
}

/// Keep a user rect inside the unit square. Degenerate sizes expand to 0.03.
#[allow(clippy::many_single_char_names)]
pub fn clamp_norm_rect(r: crate::pb::RectF) -> crate::pb::RectF {
    if !r.x.is_finite() || !r.y.is_finite() || !r.w.is_finite() || !r.h.is_finite() {
        return crate::pb::RectF::default();
    }
    let mut x = r.x;
    let mut y = r.y;
    let mut w = r.w.abs().max(0.03);
    let mut h = r.h.abs().max(0.03);
    if x < 0.0 {
        w += x;
        x = 0.0;
    }
    if y < 0.0 {
        h += y;
        y = 0.0;
    }
    w = w.min(1.0 - x).max(0.03);
    h = h.min(1.0 - y).max(0.03);
    if x + w > 1.0 {
        x = (1.0 - w).max(0.0);
    }
    if y + h > 1.0 {
        y = (1.0 - h).max(0.0);
    }
    crate::pb::RectF { x, y, w, h }
}

pub fn find(name: &str) -> Option<&'static Param> {
    PARAMS.iter().find(|p| p.name == name)
}

pub fn find_cid(cid: u32) -> Option<&'static Param> {
    PARAMS.iter().find(|p| p.cid == cid)
}

/// Sharpness UI -10..30 is sent as 0-30 or 61-70 on the wire.
pub fn encode_value(param: &Param, value: i32) -> Result<u32> {
    match param.kind {
        Kind::Int { min, max } => {
            if value < min || value > max {
                return Err(Error::Invalid(format!(
                    "{} out of range {min}..{max}",
                    param.name
                )));
            }
            if param.name == "sharpness" && value < 0 {
                return Ok((61 + (-1 - value)) as u32); // -1 => 61, -10 => 70
            }
            Ok(value as u32)
        }
        Kind::Enum(opts) => {
            if let Some((_, v)) = opts.iter().find(|(l, n)| *l == value.to_string() || *n == value as u32) {
                Ok(*v)
            } else if opts.iter().any(|(_, n)| *n == value as u32) {
                Ok(value as u32)
            } else {
                Err(Error::Invalid(format!("invalid {} value {value}", param.name)))
            }
        }
        Kind::Info => Err(Error::Invalid(format!("{} is read-only", param.name))),
    }
}

pub fn encode_named(param: &Param, raw: &str) -> Result<u32> {
    if let Kind::Enum(opts) = param.kind {
        if let Some((_, v)) = opts.iter().find(|(l, _)| l.eq_ignore_ascii_case(raw)) {
            return Ok(*v);
        }
    }
    if param.name == "zoom" {
        if let Ok(m) = raw.parse::<f64>() {
            if (1.0..=4.0).contains(&m) && m < 50.0 {
                return Ok(zoom_raw(m));
            }
        }
    }
    let n: i32 = raw
        .parse()
        .map_err(|_| Error::Invalid(format!("not a number: {raw}")))?;
    encode_value(param, n)
}

pub fn decode_int(param: &Param, wire: u32) -> i32 {
    if param.name == "sharpness" && (61..=70).contains(&wire) {
        return -1 - (wire as i32 - 61); // 61 => -1, 70 => -10
    }
    wire as i32
}

pub fn body_json(msg: &Message) -> serde_json::Value {
    match &msg.body {
        Body::Empty => serde_json::Value::Null,
        Body::Int(v) => {
            if let Some(p) = find_cid(msg.id) {
                serde_json::json!(decode_int(p, *v))
            } else {
                serde_json::json!(v)
            }
        }
        Body::Str(s) => serde_json::json!(s),
        Body::Version(v) => serde_json::json!({
            "firmware": v.firmware,
            "uniqueId": v.unique_id,
            "serial": v.serial,
            "sysCode": v.sys_code
        }),
        Body::Range(r) => serde_json::json!({"min": r.min, "max": r.max}),
        Body::WbOffset(w) => serde_json::json!({"r": w.r, "b": w.b}),
        Body::Lens(l) => {
            let map: serde_json::Map<String, serde_json::Value> = l
                .raw
                .iter()
                .map(|(f, b)| (f.to_string(), serde_json::json!(String::from_utf8_lossy(b).trim_end_matches('\0'))))
                .collect();
            serde_json::Value::Object(map)
        }
        Body::Rects(rs) => serde_json::json!(rs
            .iter()
            .map(|r| serde_json::json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h}))
            .collect::<Vec<_>>()),
        Body::Bytes { field, data } => serde_json::json!({"field": field, "hex": hex(data)}),
    }
}

fn hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub const SHUTTER_TABLE: &[(u32, u32)] = &[
    (8000, 125), (6400, 156), (5000, 200), (4000, 250), (3200, 312), (2500, 400),
    (2000, 500), (1600, 625), (1250, 800), (1000, 1000), (800, 1250), (640, 1562),
    (500, 2000), (400, 2500), (320, 3125), (250, 4000), (200, 5000), (160, 6250),
    (125, 8000), (100, 10000), (80, 12500), (60, 16666), (50, 20000), (30, 33333),
    (25, 40000),
];

pub fn shutter_from_denom(denom: u32) -> Option<u32> {
    SHUTTER_TABLE.iter().find(|(d, _)| *d == denom).map(|(_, us)| *us)
}

pub fn denom_from_us(us: u32) -> u32 {
    SHUTTER_TABLE
        .iter()
        .min_by_key(|(_, u)| u.abs_diff(us))
        .map_or(60, |(d, _)| *d)
}

pub fn zoom_mult(raw: u32) -> f64 {
    0.0625 * f64::from(raw) - 2.125
}

pub fn zoom_raw(mult: f64) -> u32 {
    ((mult + 2.125) / 0.0625).round().clamp(50.0, 98.0) as u32
}

#[cfg(test)]
mod tests {
    use super::pick_primary_face;
    use crate::pb::RectF;

    #[test]
    fn fill_order_puts_warm_first() {
        let order = super::fill_order("audio");
        assert_eq!(super::PARAMS[order[0]].name, "iso");
        assert_eq!(super::PARAMS[order[1]].name, "shutter-us");
        let iso = order.iter().position(|&i| super::PARAMS[i].name == "iso").unwrap();
        let zoom = order.iter().position(|&i| super::PARAMS[i].name == "zoom").unwrap();
        let focus = order.iter().position(|&i| super::PARAMS[i].name == "focus-mode").unwrap();
        let vol = order.iter().position(|&i| super::PARAMS[i].name == "audio-volume").unwrap();
        assert!(iso < vol && zoom < vol && focus < vol);
        assert!(order.iter().any(|&i| super::PARAMS[i].group == "audio"));
    }

    #[test]
    fn prefer_group_reorders_rest() {
        let mut order: Vec<usize> = (0..super::PARAMS.len()).collect();
        super::prefer_group(&mut order, 0, "audio");
        assert_eq!(super::PARAMS[order[0]].group, "audio");
    }

    #[test]
    fn refresh_order_is_tab_only() {
        let image = super::refresh_order("image");
        assert!(!image.is_empty());
        assert!(image.iter().all(|&i| super::PARAMS[i].group == "image"));
        assert!(image
            .iter()
            .all(|&i| !matches!(super::PARAMS[i].kind, super::Kind::Info)));
        assert!(image.len() < super::fill_order("image").len());

        let status = super::refresh_order("status");
        assert!(status.iter().any(|&i| super::PARAMS[i].name == "firmware"));
        assert!(status.iter().all(|&i| super::PARAMS[i].group == "status"));
        assert!(status.iter().all(|&i| super::PARAMS[i].name != "lens-detail"));

        let wb = super::refresh_order("wb");
        assert!(wb.iter().all(|&i| super::PARAMS[i].name != "wb-offset"));
    }

    #[test]
    fn prefers_face_over_hand() {
        let face = RectF {
            x: 0.35,
            y: 0.18,
            w: 0.28,
            h: 0.36,
        };
        let hand = RectF {
            x: 0.62,
            y: 0.48,
            w: 0.22,
            h: 0.14,
        };
        let picked = pick_primary_face(&[hand, face]);
        assert_eq!(picked.len(), 1);
        assert!((picked[0].x - face.x).abs() < f32::EPSILON);
    }

    #[test]
    fn afc_sizes_are_not_full_frame() {
        let large = super::af_size(3, 0).unwrap();
        let medium = super::af_size(3, 1).unwrap();
        let small = super::af_size(3, 2).unwrap();
        assert!(large > medium && medium > small);
        assert!(small < 0.6);
    }

    #[test]
    fn box_at_stays_in_frame() {
        let r = super::box_at(0.02, 0.98, 0.10);
        assert!(r.x >= 0.0 && r.y >= 0.0);
        assert!(r.x + r.w <= 1.0001);
        assert!(r.y + r.h <= 1.0001);
        assert!((r.w - 0.10).abs() < 1e-5);
    }

    #[test]
    fn clamp_norm_rect_clips() {
        let r = super::clamp_norm_rect(crate::pb::RectF {
            x: -0.2,
            y: 0.2,
            w: 1.5,
            h: 0.3,
        });
        assert!((r.x - 0.0).abs() < 1e-5);
        assert!(r.x + r.w <= 1.0001);
        assert!(r.w >= 0.03);
    }
}
