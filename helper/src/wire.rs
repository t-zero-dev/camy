//! Dual-envelope Yunxi wire format around a protobuf payload.

use crate::error::{Error, Result};
use crate::pb;

/// Largest inner envelope we will buffer. Real Yunxi payloads are hundreds of bytes.
pub const MAX_FRAME: usize = 64 * 1024;

pub fn xor_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |a, b| a ^ b)
}

pub fn wrap(payload: &[u8], client_type: u8) -> Vec<u8> {
    let proto_size = payload.len() as u32;
    let inner_size = proto_size + 8;
    let mut frame = Vec::with_capacity(16 + payload.len());
    frame.extend_from_slice(&[0x08, client_type, 0x00, 0x00]);
    frame.extend_from_slice(&inner_size.to_le_bytes());
    let mut inner_hdr = Vec::with_capacity(5);
    inner_hdr.push(0xA5);
    inner_hdr.extend_from_slice(&proto_size.to_be_bytes());
    let hdr_xor = xor_checksum(&inner_hdr);
    frame.extend_from_slice(&inner_hdr);
    frame.push(hdr_xor);
    frame.extend_from_slice(payload);
    frame.push(xor_checksum(payload));
    frame.push(0x5A);
    frame
}

pub fn wrap_message(msg: &pb::Message) -> Vec<u8> {
    wrap(&pb::encode_message(msg), 0x01)
}

/// Pull complete frames from `buf`. Oversized length fields fail immediately
/// instead of waiting for a giant allocation.
pub fn drain(buf: &mut Vec<u8>) -> Result<Vec<Vec<u8>>> {
    let mut payloads = Vec::new();
    let mut i = 0;
    while i + 8 <= buf.len() {
        if buf[i] != 0x08 {
            i += 1;
            continue;
        }
        let Ok(inner_bytes) = <[u8; 4]>::try_from(&buf[i + 4..i + 8]) else {
            break;
        };
        let inner_size = u32::from_le_bytes(inner_bytes) as usize;
        if inner_size > MAX_FRAME {
            return Err(Error::Protocol("frame too large".into()));
        }
        let Some(total) = 8usize.checked_add(inner_size) else {
            return Err(Error::Protocol("frame too large".into()));
        };
        if i + total > buf.len() {
            break;
        }
        if total < 16 {
            i += 1;
            continue;
        }
        let chunk = &buf[i..i + total];
        if chunk[8] != 0xA5 || chunk[total - 1] != 0x5A {
            i += 1;
            continue;
        }
        let Ok(proto_bytes) = <[u8; 4]>::try_from(&chunk[9..13]) else {
            i += 1;
            continue;
        };
        let proto_size = u32::from_be_bytes(proto_bytes) as usize;
        if proto_size > MAX_FRAME {
            return Err(Error::Protocol("frame too large".into()));
        }
        let Some(need) = 14usize
            .checked_add(proto_size)
            .and_then(|n| n.checked_add(2))
        else {
            return Err(Error::Protocol("frame too large".into()));
        };
        if need > total {
            i += 1;
            continue;
        }
        if chunk[13] != xor_checksum(&chunk[8..13])
            || chunk[14 + proto_size] != xor_checksum(&chunk[14..14 + proto_size])
        {
            i += total;
            continue;
        }
        payloads.push(chunk[14..14 + proto_size].to_vec());
        i += total;
    }
    if i > 0 {
        buf.drain(..i);
    }
    Ok(payloads)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn wrap_matches_captured_get_version() {
        let msg = pb::request(1, 1);
        let frame = wrap_message(&msg);
        assert_eq!(
            frame,
            hex("0801000011000000a500000009ac0801100118b8172801865a")
        );
    }

    #[test]
    fn drain_roundtrip() {
        let frame = wrap_message(&pb::request(1, 7));
        let mut buf = frame.clone();
        buf.extend_from_slice(&[0x00]);
        let payloads = drain(&mut buf).unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(buf, vec![0x00]);
        let msg = pb::decode_message(&payloads[0]).unwrap();
        assert_eq!(msg.id, 7);
    }

    #[test]
    fn drain_rejects_huge_inner_size() {
        let mut buf = vec![0x08, 0x01, 0x00, 0x00];
        buf.extend_from_slice(&1_000_000_000u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 16]);
        let before = buf.len();
        let err = drain(&mut buf).unwrap_err();
        assert_eq!(err.to_string(), "frame too large");
        assert!(before < 64);
        assert!(buf.len() < 64);
    }

    #[test]
    fn drain_drops_bad_checksum() {
        let mut frame = wrap_message(&pb::request(1, 7));
        let last_payload = frame.len() - 2;
        frame[last_payload] ^= 0xff;
        let mut buf = frame;
        let payloads = drain(&mut buf).unwrap();
        assert_eq!(payloads.len(), 0);
    }

    #[test]
    fn drain_waits_for_incomplete_legal_frame() {
        let frame = wrap_message(&pb::request(1, 7));
        let mut buf = frame[..12].to_vec();
        let payloads = drain(&mut buf).unwrap();
        assert_eq!(payloads.len(), 0);
        assert_eq!(buf.len(), 12);
    }
}
