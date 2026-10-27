//! Named camera parameters mapped onto Yunxi command IDs.
//!
//! `ParamId` is the only way to name a SET. Unknown CIDs are `Kind::GetOnly`.

use crate::error::{Error, Result};
use crate::pb::{Body, Message};

macro_rules! define_params {
    ($(
        $variant:ident => $name:literal, $cid:expr, $label:literal, $group:literal, $kind:expr
    ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ParamId {
            $($variant,)+
        }

        impl ParamId {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        pub const PARAMS: &[Param] = &[
            $(Param {
                id: ParamId::$variant,
                name: $name,
                cid: $cid,
                label: $label,
                group: $group,
                kind: $kind,
            },)+
        ];
    };
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub id: ParamId,
    pub name: &'static str,
    pub cid: u32,
    pub label: &'static str,
    pub group: &'static str,
    pub kind: Kind,
}

/// GET body; SET must use the same shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int { min: i32, max: i32 },
    Enum(&'static [(&'static str, u32)]),
    GetOnly,
    WbOffset,
}

impl Kind {
    pub fn settable(self) -> bool {
        !matches!(self, Self::GetOnly)
    }
}

impl ParamId {
    pub fn spec(self) -> &'static Param {
        PARAMS
            .iter()
            .find(|p| p.id == self)
            .expect("ParamId and PARAMS are generated together")
    }
}

impl std::fmt::Display for ParamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ParamId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s).ok_or_else(|| Error::Invalid(format!("unknown param {s}")))
    }
}

define_params! {
    Firmware => "firmware", 1, "Firmware", "status", Kind::GetOnly,
    ImageVersion => "image-version", 4, "Image version", "image", Kind::Enum(&[("v0", 0), ("v1", 1)]),
    FrameRate => "frame-rate", 5, "Frame rate", "video", Kind::Enum(&[("25", 25), ("30", 30), ("50", 50), ("60", 60)]),
    Resolution => "resolution", 6, "Resolution", "video", Kind::Enum(&[("auto", 0), ("4k", 1), ("1080p", 2), ("720p", 3)]),
    Brightness => "brightness", 7, "Brightness", "image", Kind::Int { min: 0, max: 100 },
    DynamicRange => "dynamic-range", 102, "Dynamic range", "image", Kind::Int { min: 0, max: 100 },
    Sharpness => "sharpness", 8, "Sharpness", "image", Kind::Int { min: -10, max: 30 },
    Contrast => "contrast", 9, "Contrast", "image", Kind::Int { min: 0, max: 100 },
    Saturation => "saturation", 10, "Saturation", "image", Kind::Int { min: 0, max: 100 },
    Hue => "hue", 11, "Hue", "image", Kind::Int { min: 0, max: 100 },
    Gamma => "gamma", 12, "Gamma", "image", Kind::Int { min: 0, max: 100 },
    Distortion => "distortion", 26, "Distortion correction", "image", Kind::Enum(&[("off", 0), ("on", 1)]),
    Shading => "shading", 27, "Vignette correction", "image", Kind::Enum(&[("off", 0), ("on", 1)]),
    Zoom => "zoom", 13, "Zoom", "zoom", Kind::Int { min: 50, max: 98 },
    ExposureMode => "exposure-mode", 14, "Exposure", "exposure", Kind::Enum(&[("auto", 0), ("manual", 1)]),
    Ev => "ev", 15, "EV", "exposure", Kind::Int { min: 0, max: 16 },
    AntiFlicker => "anti-flicker", 16, "Anti-flicker", "exposure", Kind::Enum(&[("off", 0), ("50hz", 1), ("60hz", 2)]),
    Iso => "iso", 17, "ISO", "exposure", Kind::Int { min: 100, max: 12800 },
    ShutterUs => "shutter-us", 18, "Shutter (µs)", "exposure", Kind::Int { min: 125, max: 40000 },
    Aperture => "aperture", 20, "Aperture", "exposure", Kind::Int { min: 0, max: 160 },
    WbMode => "wb-mode", 21, "White balance", "wb", Kind::Enum(&[("manual", 0), ("auto", 1)]),
    WbTemp => "wb-temp", 22, "Color temp", "wb", Kind::Int { min: 2000, max: 10000 },
    WbTone => "wb-tone", 105, "Tone", "wb", Kind::Int { min: 0, max: 128 },
    FocusMode => "focus-mode", 23, "Focus", "focus", Kind::Enum(&[("afs", 1), ("manual", 2), ("afc", 3), ("face", 4)]),
    ManuFocus => "manu-focus", 111, "Manual focus", "focus", Kind::Int { min: 0, max: 100 },
    ApertureRange => "aperture-range", 31, "Aperture range", "status", Kind::GetOnly,
    ShutterRange => "shutter-range", 32, "Shutter range", "status", Kind::GetOnly,
    WbOffset => "wb-offset", 33, "WB offset", "wb", Kind::WbOffset,
    FocusBox => "focus-box", 38, "Focus box", "focus", Kind::Enum(&[("large", 0), ("medium", 1), ("small", 2)]),
    Caf => "caf", 42, "Continuous AF", "focus", Kind::Enum(&[("off", 0), ("on", 1)]),
    FocusSpeed => "focus-speed", 47, "Focus speed", "focus", Kind::Enum(&[("fast", 1), ("medium", 3), ("slow", 5)]),
    LensStatus => "lens-status", 48, "Lens", "status", Kind::GetOnly,
    UsbVersion => "usb-version", 60, "USB", "status", Kind::GetOnly,
    HdmiFormat => "hdmi-format", 61, "HDMI format", "video", Kind::Enum(&[("auto", 0), ("4k", 1), ("1080p", 2), ("720p", 3)]),
    LensDetail => "lens-detail", 63, "Lens detail", "status", Kind::GetOnly,
    HdmiColor => "hdmi-color", 65, "HDMI color", "video", Kind::Enum(&[("auto", 0), ("rgb", 1), ("yuv422", 2)]),
    AudioVolume => "audio-volume", 73, "Audio volume", "audio", Kind::Int { min: 0, max: 100 },
    AudioDenoise => "audio-denoise", 75, "Denoise", "audio", Kind::Enum(&[("off", 0), ("weak", 1), ("middle", 2), ("strong", 3)]),
    AudioReverb => "audio-reverb", 76, "Reverb", "audio", Kind::Enum(&[("off", 0), ("weak", 1), ("middle", 2), ("strong", 3)]),
    AudioDelay => "audio-delay", 77, "Audio delay ms", "audio", Kind::Int { min: 0, max: 400 },
    AudioAgc => "audio-agc", 78, "Auto gain", "audio", Kind::Enum(&[("off", 0), ("on", 1)]),
    MicType => "mic-type", 79, "Mic type", "audio", Kind::Enum(&[("line", 0), ("mic", 1)]),
    Clut => "clut", 94, "Color LUT", "image", Kind::Enum(&[("off", 0), ("on", 1)]),
    ClutMix => "clut-mix", 96, "LUT mix", "image", Kind::Int { min: 0, max: 255 },
    Hdr => "hdr", 98, "HDR", "exposure", Kind::Enum(&[("off", 0), ("on", 1)]),
    WbLock => "wb-lock", 104, "WB lock", "wb", Kind::Enum(&[("off", 0), ("on", 1)]),
    ExposureLock => "exposure-lock", 106, "Exposure lock", "exposure", Kind::Enum(&[("off", 0), ("on", 1)]),
    ZoomSpeed => "zoom-speed", 112, "Zoom speed", "zoom", Kind::Int { min: 1, max: 5 },
}

/// SET body accepted by `parse_set` for a named parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SetPayload {
    Int(u32),
    Wb { r: u32, b: u32 },
    Rect(crate::pb::RectF),
}

impl SetPayload {
    pub fn set_wire(self) -> Option<u32> {
        match self {
            Self::Int(w) => Some(w),
            Self::Rect(_) | Self::Wb { .. } => None,
        }
    }

    pub fn set_rect(self) -> Option<crate::pb::RectF> {
        match self {
            Self::Rect(r) => Some(r),
            Self::Int(_) | Self::Wb { .. } => None,
        }
    }

    pub fn set_wb(self) -> Option<(u32, u32)> {
        match self {
            Self::Wb { r, b } => Some((r, b)),
            Self::Int(_) | Self::Rect(_) => None,
        }
    }
}

pub const S3_UNSUPPORTED: &[ParamId] = &[
    ParamId::Hue,
    ParamId::Gamma,
    ParamId::LensDetail,
    ParamId::MicType,
    ParamId::AudioVolume,
    ParamId::AudioDenoise,
    ParamId::AudioReverb,
    ParamId::AudioDelay,
    ParamId::AudioAgc,
    ParamId::HdmiFormat,
    ParamId::HdmiColor,
    ParamId::Brightness,
    ParamId::Distortion,
    ParamId::Clut,
    ParamId::ClutMix,
];

/// Enough for the first paint. Stay well under the ~25-request crash threshold.
pub const WARM: &[ParamId] = &[
    ParamId::Iso,
    ParamId::ShutterUs,
    ParamId::Zoom,
    ParamId::FocusMode,
    ParamId::Firmware,
    ParamId::FocusBox,
    ParamId::ExposureMode,
    ParamId::Ev,
    ParamId::WbMode,
    ParamId::WbTemp,
    ParamId::WbTone,
    ParamId::Brightness,
    ParamId::DynamicRange,
    ParamId::ManuFocus,
    ParamId::Contrast,
    ParamId::Distortion,
    ParamId::Shading,
];

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
    // CID_SCENE is set-only; a GET can stall S3 fill.
    if let Some(i) = PARAMS.iter().position(|p| p.id == ParamId::ImageVersion) {
        seen[i] = true;
    }
    for &id in WARM {
        if let Some(i) = PARAMS.iter().position(|p| p.id == id) {
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

/// Panel Refresh: current tab only. Skip bulky GetOnly (`lens-detail`) and WbOffset.
pub fn refresh_order(group: &str) -> Vec<usize> {
    PARAMS
        .iter()
        .enumerate()
        .filter(|(_, p)| p.group == group)
        .filter(|(_, p)| p.id != ParamId::LensDetail)
        .filter(|(_, p)| p.id != ParamId::ImageVersion)
        .filter(|(_, p)| !matches!(p.kind, Kind::WbOffset))
        .filter(|(_, p)| group == "status" || !matches!(p.kind, Kind::GetOnly))
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
    let best = rects.iter().max_by(|a, b| {
        face_score(a)
            .partial_cmp(&face_score(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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
    crate::pb::RectF {
        x,
        y,
        w: size,
        h: size,
    }
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
    ParamId::parse(name).map(ParamId::spec)
}

pub fn find_cid(cid: u32) -> Option<&'static Param> {
    PARAMS.iter().find(|p| p.cid == cid)
}

/// Unknown CIDs cannot be SET. Named params use their `Kind`.
pub fn kind_for_cid(cid: u32) -> Kind {
    find_cid(cid).map_or(Kind::GetOnly, |p| p.kind)
}

/// Parse a JSON `set` object into the body this param's GET kind accepts.
pub fn parse_set(param: &Param, v: &serde_json::Value) -> Result<SetPayload> {
    match param.kind {
        Kind::GetOnly => Err(Error::Invalid(format!("{} is read-only", param.name))),
        Kind::WbOffset => parse_wb_offset(v),
        Kind::Int { .. } | Kind::Enum(_) => {
            let raw = v
                .get("value")
                .map(|x| match x {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .ok_or_else(|| Error::Invalid("value".into()))?;
            Ok(SetPayload::Int(encode_named(param, raw.trim_matches('"'))?))
        }
    }
}

fn parse_wb_offset(v: &serde_json::Value) -> Result<SetPayload> {
    if let Some((r, b)) = wb_pair(v) {
        return Ok(SetPayload::Wb { r, b });
    }
    if let Some(val) = v.get("value") {
        if let Some((r, b)) = wb_pair(val) {
            return Ok(SetPayload::Wb { r, b });
        }
        return Err(Error::Invalid("wb-offset needs r and b, not an int".into()));
    }
    Err(Error::Invalid("wb-offset needs r and b".into()))
}

fn wb_pair(v: &serde_json::Value) -> Option<(u32, u32)> {
    let r = json_u32(v.get("r"))?;
    let b = json_u32(v.get("b"))?;
    if r > 20 || b > 20 {
        return None;
    }
    Some((r, b))
}

fn json_u32(v: Option<&serde_json::Value>) -> Option<u32> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return u32::try_from(n).ok();
    }
    v.as_str()?.parse().ok()
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
            if param.id == ParamId::Sharpness && value < 0 {
                return Ok((61 + (-1 - value)) as u32); // -1 => 61, -10 => 70
            }
            Ok(value as u32)
        }
        Kind::Enum(opts) => {
            if let Some((_, v)) = opts
                .iter()
                .find(|(l, n)| *l == value.to_string() || *n == value as u32)
            {
                Ok(*v)
            } else if opts.iter().any(|(_, n)| *n == value as u32) {
                Ok(value as u32)
            } else {
                Err(Error::Invalid(format!(
                    "invalid {} value {value}",
                    param.name
                )))
            }
        }
        Kind::GetOnly => Err(Error::Invalid(format!("{} is read-only", param.name))),
        Kind::WbOffset => Err(Error::Invalid(format!(
            "{} needs r and b, not an int",
            param.name
        ))),
    }
}

pub fn encode_named(param: &Param, raw: &str) -> Result<u32> {
    match param.kind {
        Kind::GetOnly => return Err(Error::Invalid(format!("{} is read-only", param.name))),
        Kind::WbOffset => {
            return Err(Error::Invalid(format!(
                "{} needs r and b, not an int",
                param.name
            )));
        }
        Kind::Int { .. } | Kind::Enum(_) => {}
    }
    if let Kind::Enum(opts) = param.kind {
        if let Some((_, v)) = opts.iter().find(|(l, _)| l.eq_ignore_ascii_case(raw)) {
            return Ok(*v);
        }
    }
    if param.id == ParamId::Zoom {
        if let Ok(m) = raw.parse::<f64>() {
            if (1.0..=4.0).contains(&m) && m < 50.0 {
                return Ok(zoom_raw(m));
            }
        }
    }
    let n: i32 = raw
        .parse()
        .map_err(|_| Error::Invalid("not a number".into()))?;
    encode_value(param, n)
}

pub fn decode_int(param: &Param, wire: u32) -> i32 {
    if param.id == ParamId::Sharpness && (61..=70).contains(&wire) {
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
                .map(|(f, b)| {
                    (
                        f.to_string(),
                        serde_json::json!(String::from_utf8_lossy(b).trim_end_matches('\0')),
                    )
                })
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
    (8000, 125),
    (6400, 156),
    (5000, 200),
    (4000, 250),
    (3200, 312),
    (2500, 400),
    (2000, 500),
    (1600, 625),
    (1250, 800),
    (1000, 1000),
    (800, 1250),
    (640, 1562),
    (500, 2000),
    (400, 2500),
    (320, 3125),
    (250, 4000),
    (200, 5000),
    (160, 6250),
    (125, 8000),
    (100, 10000),
    (80, 12500),
    (60, 16666),
    (50, 20000),
    (30, 33333),
    (25, 40000),
];

pub fn shutter_from_denom(denom: u32) -> Option<u32> {
    SHUTTER_TABLE
        .iter()
        .find(|(d, _)| *d == denom)
        .map(|(_, us)| *us)
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

/// S3 ePTZ crop: centred square of size `1/multiplier` in the unit frame.
pub fn zoom_rect(raw: u32) -> crate::pb::RectF {
    let m = zoom_mult(raw).clamp(1.0, 4.0);
    let size = (1.0 / m) as f32;
    box_at(0.5, 0.5, size)
}

/// S7 optical zoom is CID 13 Int. S3 digital zoom is a centred CID 13 Rect.
pub fn zoom_set_payload(s3: bool, wire: u32) -> SetPayload {
    if s3 {
        SetPayload::Rect(zoom_rect(wire))
    } else {
        SetPayload::Int(wire)
    }
}

pub fn zoom_raw_from_rect(r: crate::pb::RectF) -> u32 {
    let size = r.w.min(r.h).clamp(0.25, 1.0);
    zoom_raw(1.0 / f64::from(size))
}

#[cfg(test)]
mod tests {
    use super::{kind_for_cid, parse_set, pick_primary_face, Kind, ParamId, SetPayload};
    use crate::pb::RectF;
    use serde_json::json;

    #[test]
    fn param_id_roundtrip() {
        assert_eq!(ParamId::ALL.len(), super::PARAMS.len());
        for (i, p) in super::PARAMS.iter().enumerate() {
            assert_eq!(ParamId::ALL[i], p.id);
            assert_eq!(ParamId::parse(p.name), Some(p.id));
            assert_eq!(p.id.as_str(), p.name);
            assert_eq!(p.id.spec().cid, p.cid);
            assert_eq!(p.id.spec().name, p.name);
        }
        assert!(ParamId::parse("color-style").is_none());
        assert!(ParamId::parse("focus-enhance").is_none());
        assert_eq!(ParamId::parse("image-version"), Some(ParamId::ImageVersion));
        assert_eq!(ParamId::ImageVersion.spec().cid, 4);
    }

    #[test]
    fn parse_set_getonly_fails() {
        let p = ParamId::Firmware.spec();
        assert!(parse_set(p, &json!({"name": "firmware", "value": "1"})).is_err());
        assert!(super::encode_named(p, "1").is_err());
        assert!(!p.kind.settable());
    }

    #[test]
    fn parse_set_wb_rejects_int() {
        let p = ParamId::WbOffset.spec();
        assert!(parse_set(p, &json!({"name": "wb-offset", "value": "10"})).is_err());
        assert!(parse_set(p, &json!({"name": "wb-offset", "value": 10})).is_err());
        assert!(super::encode_named(p, "10").is_err());
        assert!(super::encode_value(p, 10).is_err());
        assert!(parse_set(p, &json!({"name": "wb-offset", "r": 99, "b": 12})).is_err());
        let ok = parse_set(p, &json!({"name": "wb-offset", "r": 8, "b": 12})).unwrap();
        assert_eq!(ok, SetPayload::Wb { r: 8, b: 12 });
        let nested =
            parse_set(p, &json!({"name": "wb-offset", "value": {"r": 1, "b": 2}})).unwrap();
        assert_eq!(nested, SetPayload::Wb { r: 1, b: 2 });
        assert!(p.kind.settable());
    }

    #[test]
    fn parse_set_image_version() {
        let p = ParamId::ImageVersion.spec();
        assert_eq!(
            parse_set(p, &json!({"name": "image-version", "value": "v0"})).unwrap(),
            SetPayload::Int(0)
        );
        assert_eq!(
            parse_set(p, &json!({"name": "image-version", "value": "1"})).unwrap(),
            SetPayload::Int(1)
        );
        assert_eq!(super::encode_named(p, "v1").unwrap(), 1);
    }

    #[test]
    fn parse_set_int_ok() {
        let p = ParamId::Iso.spec();
        assert_eq!(
            parse_set(p, &json!({"name": "iso", "value": "400"})).unwrap(),
            SetPayload::Int(400)
        );
        assert_eq!(
            parse_set(p, &json!({"cmd": "set", "name": "iso", "value": 200})).unwrap(),
            SetPayload::Int(200)
        );
    }

    #[test]
    fn encode_named_does_not_echo_raw() {
        let p = ParamId::Iso.spec();
        let err = super::encode_named(p, r#"<img src="https://example.invalid/t">"#).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("not a number"));
        assert!(!s.contains('<'));
        assert!(!s.contains("img"));
        assert!(!s.contains("example.invalid"));
    }

    #[test]
    fn unknown_cid_is_getonly() {
        assert!(matches!(kind_for_cid(55), Kind::GetOnly));
        assert!(matches!(kind_for_cid(58), Kind::GetOnly));
        assert!(matches!(kind_for_cid(29), Kind::GetOnly));
        assert!(!kind_for_cid(55).settable());
        assert!(matches!(kind_for_cid(17), Kind::Int { .. }));
        assert!(matches!(kind_for_cid(33), Kind::WbOffset));
    }

    #[test]
    fn fill_order_puts_warm_first() {
        let order = super::fill_order("audio");
        assert_eq!(super::PARAMS[order[0]].id, ParamId::Iso);
        assert_eq!(super::PARAMS[order[1]].id, ParamId::ShutterUs);
        let iso = order
            .iter()
            .position(|&i| super::PARAMS[i].id == ParamId::Iso)
            .unwrap();
        let zoom = order
            .iter()
            .position(|&i| super::PARAMS[i].id == ParamId::Zoom)
            .unwrap();
        let focus = order
            .iter()
            .position(|&i| super::PARAMS[i].id == ParamId::FocusMode)
            .unwrap();
        let vol = order
            .iter()
            .position(|&i| super::PARAMS[i].id == ParamId::AudioVolume)
            .unwrap();
        assert!(iso < vol && zoom < vol && focus < vol);
        assert!(order.iter().any(|&i| super::PARAMS[i].group == "audio"));
        assert!(order
            .iter()
            .all(|&i| super::PARAMS[i].id != ParamId::ImageVersion));
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
        assert_ne!(image.len(), 0);
        assert!(image.iter().all(|&i| super::PARAMS[i].group == "image"));
        assert!(image.iter().all(|&i| !matches!(
            super::PARAMS[i].kind,
            super::Kind::GetOnly | super::Kind::WbOffset
        )));
        assert!(image.len() < super::fill_order("image").len());

        let status = super::refresh_order("status");
        assert!(status
            .iter()
            .any(|&i| super::PARAMS[i].id == ParamId::Firmware));
        assert!(status.iter().all(|&i| super::PARAMS[i].group == "status"));
        assert!(status
            .iter()
            .all(|&i| super::PARAMS[i].id != ParamId::LensDetail));

        let wb = super::refresh_order("wb");
        assert!(wb.iter().all(|&i| super::PARAMS[i].id != ParamId::WbOffset));
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
    fn zoom_rect_is_centred_crop() {
        let wide = super::zoom_rect(50);
        assert!((wide.w - 1.0).abs() < 1e-5);
        assert!((wide.h - 1.0).abs() < 1e-5);
        assert!((wide.x - 0.0).abs() < 1e-5);
        let tele = super::zoom_rect(98);
        assert!((tele.w - 0.25).abs() < 1e-4);
        assert!((tele.x - 0.375).abs() < 1e-4);
        assert_eq!(super::zoom_raw_from_rect(tele), 98);
        assert_eq!(super::zoom_raw_from_rect(wide), 50);
    }

    #[test]
    fn zoom_set_payload_s7_stays_int_s3_is_rect() {
        assert_eq!(super::zoom_set_payload(false, 80), SetPayload::Int(80));
        assert_eq!(super::zoom_set_payload(false, 50), SetPayload::Int(50));
        let s3 = super::zoom_set_payload(true, 80);
        let rect = s3.set_rect().expect("s3 zoom is a crop rect");
        assert_eq!(super::zoom_raw_from_rect(rect), 80);
        assert!(s3.set_wire().is_none());
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
