//! Minimal protobuf codec for `tv.yunxi.protobuf.Message`.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Request = 1,
    Set = 2,
    Post = 3,
    Response = 4,
    Notify = 5,
}

impl MsgType {
    pub fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            1 => Self::Request,
            2 => Self::Set,
            3 => Self::Post,
            4 => Self::Response,
            5 => Self::Notify,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    pub firmware: String,
    pub unique_id: String,
    pub serial: String,
    pub sys_code: String,
}

#[derive(Debug, Clone, Default)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Default)]
pub struct WbOffset {
    pub r: u32,
    pub b: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LensDetail {
    pub raw: Vec<(u32, Vec<u8>)>,
}

#[derive(Debug, Clone, Default)]
pub enum Body {
    #[default]
    Empty,
    Int(u32),
    Str(String),
    Version(VersionInfo),
    Range(Range),
    WbOffset(WbOffset),
    Lens(LensDetail),
    Rects(Vec<RectF>),
    Bytes {
        field: u32,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    pub ty: Option<MsgType>,
    pub seq: u32,
    pub timeout: u32,
    pub resp_code: u32,
    pub id: u32,
    pub body: Body,
}

impl Message {
    pub fn is_response_to(&self, seq: u32) -> bool {
        self.ty == Some(MsgType::Response) && self.seq == seq
    }

    /// Yunxi often echoes cid and omits seq. Prefer seq; accept cid so SET
    /// is consumed and does not fall through as an empty param overwrite.
    pub fn is_reply_to(&self, seq: u32, cid: u32) -> bool {
        self.ty == Some(MsgType::Response) && (self.seq == seq || self.id == cid)
    }

    pub fn is_notify(&self) -> bool {
        self.ty == Some(MsgType::Notify)
    }
}

pub fn encode_varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            b |= 0x80;
        }
        out.push(b);
        if n == 0 {
            break;
        }
    }
    out
}

pub fn decode_varint(data: &[u8], mut i: usize) -> Result<(u64, usize)> {
    let mut n = 0u64;
    let mut shift = 0;
    while i < data.len() {
        let b = data[i];
        i += 1;
        n |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok((n, i));
        }
        shift += 7;
        if shift > 63 {
            return Err(Error::Protocol("varint overflow".into()));
        }
    }
    Err(Error::Protocol("truncated varint".into()))
}

fn tag(field: u32, wt: u32) -> Vec<u8> {
    encode_varint(u64::from((field << 3) | wt))
}

fn field_varint(field: u32, value: u32) -> Vec<u8> {
    let mut o = tag(field, 0);
    o.extend(encode_varint(u64::from(value)));
    o
}

fn field_bytes(field: u32, value: &[u8]) -> Vec<u8> {
    let mut o = tag(field, 2);
    o.extend(encode_varint(value.len() as u64));
    o.extend_from_slice(value);
    o
}

fn field_float(field: u32, value: f32) -> Vec<u8> {
    let mut o = tag(field, 5);
    o.extend_from_slice(&value.to_bits().to_le_bytes());
    o
}

fn encode_rect_f(r: &RectF) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend(field_float(1, r.x));
    o.extend(field_float(2, r.y));
    o.extend(field_float(3, r.x + r.w));
    o.extend(field_float(4, r.y + r.h));
    o
}

fn encode_body(body: &Body) -> Vec<u8> {
    match body {
        Body::Int(v) => field_varint(1, *v),
        Body::Str(s) => field_bytes(2, s.as_bytes()),
        Body::Rects(rs) => rs
            .first()
            .map(|r| field_bytes(14, &encode_rect_f(r)))
            .unwrap_or_default(),
        Body::WbOffset(w) => {
            let mut inner = field_varint(1, w.r);
            inner.extend(field_varint(2, w.b));
            field_bytes(13, &inner)
        }
        Body::Empty | Body::Version(_) | Body::Range(_) | Body::Lens(_) => Vec::new(),
        Body::Bytes { field, data } => field_bytes(*field, data),
    }
}

pub fn encode_message(msg: &Message) -> Vec<u8> {
    let mut o = Vec::new();
    if let Some(ty) = msg.ty {
        o.extend(field_varint(1, ty as u32));
    }
    if msg.seq != 0 {
        o.extend(field_varint(2, msg.seq));
    }
    if msg.timeout != 0 {
        o.extend(field_varint(3, msg.timeout));
    }
    if msg.resp_code != 0 {
        o.extend(field_varint(4, msg.resp_code));
    }
    if msg.id != 0 {
        o.extend(field_varint(5, msg.id));
    }
    let body = encode_body(&msg.body);
    if !body.is_empty() {
        o.extend(field_bytes(6, &body));
    }
    o
}

pub fn request(seq: u32, cid: u32) -> Message {
    Message {
        ty: Some(MsgType::Request),
        seq,
        timeout: 3000,
        id: cid,
        ..Default::default()
    }
}

pub fn set_int(seq: u32, cid: u32, value: u32) -> Message {
    Message {
        ty: Some(MsgType::Set),
        seq,
        timeout: 3000,
        id: cid,
        body: Body::Int(value),
        ..Default::default()
    }
}

pub fn set_wb_offset(seq: u32, cid: u32, r: u32, b: u32) -> Message {
    Message {
        ty: Some(MsgType::Set),
        seq,
        timeout: 3000,
        id: cid,
        body: Body::WbOffset(WbOffset { r, b }),
        ..Default::default()
    }
}

pub fn set_rect(seq: u32, cid: u32, rect: RectF) -> Message {
    Message {
        ty: Some(MsgType::Set),
        seq,
        timeout: 3000,
        id: cid,
        body: Body::Rects(vec![rect]),
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
pub struct ProtoField {
    pub field: u32,
    pub value: ProtoValue,
}

#[derive(Debug, Clone)]
pub enum ProtoValue {
    Varint(u64),
    Bytes(Vec<u8>),
    Float(f32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Largest length-delimited protobuf field we will copy, including nested
/// messages. Matches the wire frame cap so a nested length cannot allocate
/// past what a legal frame could contain.
pub const MAX_FIELD_BYTES: usize = 64 * 1024;

pub fn decode_fields(data: &[u8]) -> Result<Vec<ProtoField>> {
    let mut i = 0;
    let mut fields = Vec::new();
    while i < data.len() {
        let (key, ni) = decode_varint(data, i)?;
        i = ni;
        let field = (key >> 3) as u32;
        let wt = (key & 7) as u32;
        match wt {
            0 => {
                let (v, ni) = decode_varint(data, i)?;
                i = ni;
                fields.push(ProtoField {
                    field,
                    value: ProtoValue::Varint(v),
                });
            }
            2 => {
                let (ln, ni) = decode_varint(data, i)?;
                i = ni;
                if ln > MAX_FIELD_BYTES as u64 {
                    return Err(Error::Protocol("bytes field too long".into()));
                }
                let ln = ln as usize;
                let end = i
                    .checked_add(ln)
                    .ok_or_else(|| Error::Protocol("bytes field too long".into()))?;
                if end > data.len() {
                    return Err(Error::Protocol("truncated bytes".into()));
                }
                fields.push(ProtoField {
                    field,
                    value: ProtoValue::Bytes(data[i..end].to_vec()),
                });
                i = end;
            }
            1 => {
                if i + 8 > data.len() {
                    return Err(Error::Protocol("truncated fixed64".into()));
                }
                i += 8;
            }
            5 => {
                let bits = read_u32_le(data, i)?;
                i += 4;
                fields.push(ProtoField {
                    field,
                    value: ProtoValue::Float(f32::from_bits(bits)),
                });
            }
            _ => return Err(Error::Protocol(format!("unsupported wire type {wt}"))),
        }
    }
    Ok(fields)
}

fn read_u32_le(data: &[u8], i: usize) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(i..i + 4)
        .ok_or_else(|| Error::Protocol("truncated fixed32".into()))?
        .try_into()
        .map_err(|_| Error::Protocol("truncated fixed32".into()))?;
    Ok(u32::from_le_bytes(bytes))
}

const MAX_TEXT: usize = 512;

fn utf8(bytes: &[u8]) -> String {
    let n = bytes.len().min(MAX_TEXT);
    String::from_utf8_lossy(&bytes[..n]).into_owned()
}

fn decode_rect(data: &[u8]) -> RectF {
    let mut r = RectF::default();
    let Ok(fields) = decode_fields(data) else {
        return r;
    };
    let mut nums = [0f32; 4];
    for f in fields {
        if let ProtoValue::Float(v) = f.value {
            if (1..=4).contains(&f.field) {
                nums[(f.field - 1) as usize] = v;
            }
        }
    }
    r.x = nums[0];
    r.y = nums[1];
    // field 3/4 may be width/height or right/bottom
    if nums[2] > nums[0] && nums[3] > nums[1] && nums[2] <= 1.5 && nums[3] <= 1.5 {
        r.w = nums[2] - nums[0];
        r.h = nums[3] - nums[1];
    } else {
        r.w = nums[2];
        r.h = nums[3];
    }
    normalize_rect(&mut r);
    r
}

fn normalize_rect(r: &mut RectF) {
    if !r.x.is_finite() || !r.y.is_finite() || !r.w.is_finite() || !r.h.is_finite() {
        *r = RectF::default();
        return;
    }
    if r.x > 1.5 || r.y > 1.5 || r.w > 1.5 || r.h > 1.5 {
        r.x /= 1280.0;
        r.y /= 720.0;
        r.w /= 1280.0;
        r.h /= 720.0;
    }
    r.x = r.x.clamp(0.0, 1.0);
    r.y = r.y.clamp(0.0, 1.0);
    r.w = r.w.clamp(0.0, 1.0 - r.x);
    r.h = r.h.clamp(0.0, 1.0 - r.y);
}

fn decode_rect_list(data: &[u8]) -> Vec<RectF> {
    let Ok(fields) = decode_fields(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in fields {
        match f.value {
            ProtoValue::Bytes(b) if f.field == 1 => out.push(decode_rect(&b)),
            _ => {}
        }
    }
    if out.is_empty() {
        let one = decode_rect(data);
        if one.w > 0.0 || one.h > 0.0 {
            out.push(one);
        }
    }
    out
}

fn decode_version(data: &[u8]) -> VersionInfo {
    let mut v = VersionInfo::default();
    if let Ok(fields) = decode_fields(data) {
        for f in fields {
            if let ProtoValue::Bytes(b) = f.value {
                match f.field {
                    1 => v.firmware = utf8(&b),
                    2 => v.unique_id = utf8(&b),
                    3 => v.serial = utf8(&b),
                    4 => v.sys_code = utf8(&b),
                    _ => {}
                }
            }
        }
    }
    v
}

fn decode_range(data: &[u8]) -> Range {
    let mut r = Range::default();
    if let Ok(fields) = decode_fields(data) {
        for f in fields {
            if let ProtoValue::Varint(n) = f.value {
                match f.field {
                    1 => r.min = n as u32,
                    2 => r.max = n as u32,
                    _ => {}
                }
            }
        }
    }
    r
}

fn decode_wb(data: &[u8]) -> WbOffset {
    let mut w = WbOffset::default();
    if let Ok(fields) = decode_fields(data) {
        for f in fields {
            if let ProtoValue::Varint(n) = f.value {
                match f.field {
                    1 => w.r = n as u32,
                    2 => w.b = n as u32,
                    _ => {}
                }
            }
        }
    }
    w
}

fn decode_body(data: &[u8]) -> Body {
    let Ok(fields) = decode_fields(data) else {
        return Body::Bytes {
            field: 0,
            data: data.to_vec(),
        };
    };
    if fields.is_empty() {
        return Body::Empty;
    }
    for f in &fields {
        match (&f.value, f.field) {
            (ProtoValue::Bytes(b), 14) => return Body::Rects(vec![decode_rect(b)]),
            (ProtoValue::Bytes(b), 19) => return Body::Rects(decode_rect_list(b)),
            _ => {}
        }
    }
    if fields.len() == 1 {
        let f = &fields[0];
        match (&f.value, f.field) {
            (ProtoValue::Varint(n), 1) => return Body::Int(*n as u32),
            (ProtoValue::Bytes(b), 2) => return Body::Str(utf8(b)),
            (ProtoValue::Bytes(b), 10) => return Body::Version(decode_version(b)),
            (ProtoValue::Bytes(b), 7) => return Body::Range(decode_range(b)),
            (ProtoValue::Bytes(b), 13) => return Body::WbOffset(decode_wb(b)),
            (ProtoValue::Bytes(b), 20) => {
                return Body::Lens(LensDetail {
                    raw: decode_fields(b)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|x| match x.value {
                            ProtoValue::Bytes(d) => Some((x.field, d)),
                            _ => None,
                        })
                        .collect(),
                });
            }
            (ProtoValue::Bytes(b), field) => {
                return Body::Bytes {
                    field,
                    data: b.clone(),
                };
            }
            _ => {}
        }
    }
    Body::Bytes {
        field: 0,
        data: data.to_vec(),
    }
}

pub fn decode_message(data: &[u8]) -> Result<Message> {
    let fields = decode_fields(data)?;
    let mut msg = Message::default();
    for f in fields {
        match (f.field, f.value) {
            (1, ProtoValue::Varint(n)) => msg.ty = MsgType::from_u32(n as u32),
            (2, ProtoValue::Varint(n)) => msg.seq = n as u32,
            (3, ProtoValue::Varint(n)) => msg.timeout = n as u32,
            (4, ProtoValue::Varint(n)) => msg.resp_code = n as u32,
            (5, ProtoValue::Varint(n)) => msg.id = n as u32,
            (6, ProtoValue::Bytes(b)) => msg.body = decode_body(&b),
            _ => {}
        }
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_version_payload() {
        let msg = request(1, 1);
        let bytes = encode_message(&msg);
        assert_eq!(bytes, hex("0801100118b8172801"));
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn decode_version_response() {
        let payload = hex("0804100120c80128013260525e0a1f59756e786943616d6572612d7a6869746f6e672d63683433312d302e302e3912273078383234343239343432333631306130386561663966346437363864313761633632326333301a0c303234383235333630303035220431303031");
        let msg = decode_message(&payload).unwrap();
        assert_eq!(msg.ty, Some(MsgType::Response));
        assert_eq!(msg.seq, 1);
        assert_eq!(msg.resp_code, 200);
        assert_eq!(msg.id, 1);
        match msg.body {
            Body::Version(v) => {
                assert_eq!(v.firmware, "YunxiCamera-zhitong-ch431-0.0.9");
                assert_eq!(v.serial, "024825360005");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn decode_normalized_rect() {
        // RectF is left/top/right/bottom. 0.25,0.25,0.75,0.75 → 0.5×0.5 box.
        let payload = hex("0d0000803e150000803e1d0000403f250000403f");
        let r = decode_rect(&payload);
        assert!((r.x - 0.25).abs() < 1e-6);
        assert!((r.y - 0.25).abs() < 1e-6);
        assert!((r.w - 0.5).abs() < 1e-6);
        assert!((r.h - 0.5).abs() < 1e-6);
    }

    #[test]
    fn nan_rect_is_empty() {
        let mut r = RectF {
            x: f32::NAN,
            y: 0.1,
            w: 0.2,
            h: 0.3,
        };
        normalize_rect(&mut r);
        assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn inf_rect_is_empty() {
        let mut r = RectF {
            x: 0.0,
            y: 0.0,
            w: f32::INFINITY,
            h: 0.5,
        };
        normalize_rect(&mut r);
        assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn encode_rect_roundtrip() {
        let src = RectF {
            x: 0.1,
            y: 0.2,
            w: 0.3,
            h: 0.4,
        };
        let decoded = decode_rect(&encode_rect_f(&src));
        assert!((decoded.x - 0.1).abs() < 1e-6);
        assert!((decoded.y - 0.2).abs() < 1e-6);
        assert!((decoded.w - 0.3).abs() < 1e-6);
        assert!((decoded.h - 0.4).abs() < 1e-6);
    }

    #[test]
    fn rejects_oversized_bytes_field() {
        let mut data = encode_varint((1 << 3) | 2);
        data.extend(encode_varint((MAX_FIELD_BYTES as u64) + 1));
        data.extend_from_slice(&[0u8; 4]);
        let err = decode_fields(&data).unwrap_err();
        assert_eq!(err.to_string(), "bytes field too long");
    }

    #[test]
    fn rejects_truncated_legal_bytes_field() {
        let mut data = encode_varint((1 << 3) | 2);
        data.extend(encode_varint(10));
        data.extend_from_slice(&[1, 2, 3]);
        let err = decode_fields(&data).unwrap_err();
        assert_eq!(err.to_string(), "truncated bytes");
    }

    #[test]
    fn response_matches_seq_only() {
        let mut msg = request(3, 17);
        msg.ty = Some(MsgType::Response);
        msg.seq = 3;
        assert!(msg.is_response_to(3));
        assert!(!msg.is_response_to(4));
        msg.seq = 0;
        msg.id = 17;
        assert!(!msg.is_response_to(3));
        assert!(msg.is_reply_to(3, 17));
        assert!(!msg.is_reply_to(3, 99));
    }
}
