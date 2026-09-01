use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::params;
use crate::pb::{self, Message, MsgType};
use crate::wire;

#[derive(Default)]
struct WriteBuf {
    buf: Vec<u8>,
    off: usize,
}

impl WriteBuf {
    fn pending(&self) -> bool {
        self.off < self.buf.len()
    }

    fn queue(&mut self, frame: Vec<u8>) -> Result<()> {
        if self.pending() {
            return Err(Error::Busy);
        }
        self.buf = frame;
        self.off = 0;
        Ok(())
    }

    fn flush_to(&mut self, w: &mut impl Write) -> Result<()> {
        while self.off < self.buf.len() {
            match w.write(&self.buf[self.off..]) {
                Ok(0) => return Err(Error::Disconnected("camera closed the socket")),
                Ok(n) => self.off += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                    return Ok(());
                }
                Err(e) => return Err(Error::io_ctx(e, "write")),
            }
        }
        self.buf.clear();
        self.off = 0;
        Ok(())
    }
}

pub struct Client {
    stream: TcpStream,
    buf: Vec<u8>,
    out: WriteBuf,
    seq: u32,
    last_rpc: Instant,
}

impl Client {
    pub fn connect(addr: &str, timeout: Duration) -> Result<Self> {
        let stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| Error::Protocol(format!("addr: {e}")))?,
            timeout,
        )
        .map_err(|e| Error::io_ctx(e, format!("connect {addr}")))?;
        stream.set_nodelay(true).ok();
        stream
            .set_read_timeout(Some(Duration::from_millis(250)))
            .ok();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .ok();
        Ok(Self {
            stream,
            buf: Vec::new(),
            out: WriteBuf::default(),
            seq: 1,
            last_rpc: Instant::now(),
        })
    }

    fn next_seq(&mut self) -> u32 {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1).max(1);
        s
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.stream.as_fd()
    }

    pub fn set_nonblocking(&self, on: bool) -> Result<()> {
        self.stream
            .set_nonblocking(on)
            .map_err(|e| Error::io_ctx(e, "nonblock"))
    }

    pub fn pace_ready(&self) -> bool {
        !self.write_pending() && self.last_rpc.elapsed() >= Duration::from_millis(160)
    }

    pub fn write_pending(&self) -> bool {
        self.out.pending()
    }

    pub fn flush_write(&mut self) -> Result<()> {
        let pending = self.out.pending();
        self.out.flush_to(&mut self.stream)?;
        if pending && !self.out.pending() {
            self.last_rpc = Instant::now();
        }
        Ok(())
    }

    fn queue_frame(&mut self, msg: &Message) -> Result<()> {
        self.out.queue(wire::wrap_message(msg))?;
        self.flush_write()
    }

    pub fn begin_get(&mut self, cid: u32) -> Result<u32> {
        if self.write_pending() {
            return Err(Error::Busy);
        }
        let seq = self.next_seq();
        self.queue_frame(&pb::request(seq, cid))?;
        Ok(seq)
    }

    pub fn queue_heartbeat(&mut self) -> Result<()> {
        if self.write_pending() {
            return Err(Error::Busy);
        }
        let seq = self.next_seq();
        self.queue_frame(&pb::set_int(seq, params::HEARTBEAT_CID, 1))
    }

    pub fn begin_set(&mut self, cid: u32, value: u32) -> Result<u32> {
        if self.write_pending() {
            return Err(Error::Busy);
        }
        let seq = self.next_seq();
        self.queue_frame(&pb::set_int(seq, cid, value))?;
        Ok(seq)
    }

    pub fn begin_set_rect(&mut self, cid: u32, rect: pb::RectF) -> Result<u32> {
        if self.write_pending() {
            return Err(Error::Busy);
        }
        let seq = self.next_seq();
        self.queue_frame(&pb::set_rect(seq, cid, rect))?;
        Ok(seq)
    }

    pub fn try_read(&mut self) -> Result<Vec<Message>> {
        let mut tmp = [0u8; 8192];
        match self.stream.read(&mut tmp) {
            Ok(0) => Err(Error::Disconnected("camera closed the socket")),
            Ok(n) => {
                if self.buf.len().saturating_add(n) > wire::MAX_FRAME.saturating_add(16) {
                    return Err(Error::Protocol("frame too large".into()));
                }
                self.buf.extend_from_slice(&tmp[..n]);
                let payloads = wire::drain(&mut self.buf)?;
                payloads.into_iter().map(|p| pb::decode_message(&p)).collect()
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                Ok(Vec::new())
            }
            Err(e) => Err(Error::io_ctx(e, "read")),
        }
    }
}

/// Blocking CLI path. Watch uses `Client` and must not call these.
pub struct BlockingClient {
    inner: Client,
}

impl BlockingClient {
    pub fn connect(addr: &str, timeout: Duration) -> Result<Self> {
        Ok(Self {
            inner: Client::connect(addr, timeout)?,
        })
    }

    fn write_msg(&mut self, msg: &Message) -> Result<()> {
        const PACE: Duration = Duration::from_millis(160);
        let elapsed = self.inner.last_rpc.elapsed();
        if elapsed < PACE {
            std::thread::sleep(PACE.saturating_sub(elapsed));
        }
        let frame = wire::wrap_message(msg);
        self.inner
            .stream
            .write_all(&frame)
            .map_err(|e| Error::io_ctx(e, "write"))?;
        self.inner.last_rpc = Instant::now();
        Ok(())
    }

    fn pump(&mut self, wait: Duration) -> Result<Vec<Message>> {
        let deadline = Instant::now() + wait;
        let mut out = Vec::new();
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let slice = remaining.min(Duration::from_millis(50));
            self.inner.stream.set_read_timeout(Some(slice)).ok();
            let mut msgs = self.inner.try_read()?;
            out.append(&mut msgs);
            if !out.is_empty() && Instant::now() + Duration::from_millis(20) >= deadline {
                break;
            }
        }
        Ok(out)
    }

    pub fn heartbeat(&mut self) -> Result<()> {
        let seq = self.inner.next_seq();
        self.write_msg(&pb::set_int(seq, params::HEARTBEAT_CID, 1))
    }

    pub fn get(&mut self, cid: u32) -> Result<Message> {
        self.get_within(cid, Duration::from_millis(800))
    }

    pub fn get_within(&mut self, cid: u32, timeout: Duration) -> Result<Message> {
        let seq = self.inner.next_seq();
        self.write_msg(&pb::request(seq, cid))?;
        let deadline = Instant::now() + timeout;
        loop {
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                return Err(Error::Timeout { cid: Some(cid) });
            }
            for msg in self.pump(wait.min(Duration::from_millis(120)))? {
                if msg.is_reply_to(seq, cid) {
                    return Ok(msg);
                }
            }
        }
    }

    pub fn set(&mut self, cid: u32, value: u32) -> Result<Message> {
        self.send_set(pb::set_int(0, cid, value), pb::Body::Int(value))
    }

    pub fn set_rect(&mut self, cid: u32, rect: pb::RectF) -> Result<Message> {
        self.send_set(pb::set_rect(0, cid, rect), pb::Body::Rects(vec![rect]))
    }

    fn send_set(&mut self, mut msg: Message, fallback: pb::Body) -> Result<Message> {
        let seq = self.inner.next_seq();
        msg.seq = seq;
        let cid = msg.id;
        self.write_msg(&msg)?;
        let deadline = Instant::now() + Duration::from_millis(800);
        loop {
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                return Ok(Message {
                    ty: Some(MsgType::Response),
                    seq,
                    resp_code: 0,
                    id: cid,
                    body: fallback,
                    ..Default::default()
                });
            }
            for reply in self.pump(wait.min(Duration::from_millis(200)))? {
                if reply.is_reply_to(seq, cid) {
                    return Ok(reply);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WriteBuf;
    use std::io::{self, Write};

    struct Limited<'a> {
        inner: &'a mut Vec<u8>,
        max: usize,
    }

    impl Write for Limited<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.max == 0 {
                return Err(io::Error::from(io::ErrorKind::WouldBlock));
            }
            let n = buf.len().min(self.max);
            self.max = 0;
            self.inner.extend_from_slice(&buf[..n]);
            Ok(n)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn queue_rejects_second_frame() {
        let mut out = WriteBuf::default();
        out.queue(vec![1, 2, 3, 4]).unwrap();
        assert!(out.queue(vec![5]).is_err());
    }

    #[test]
    fn flush_two_chunks() {
        let mut out = WriteBuf::default();
        out.queue(vec![1, 2, 3, 4, 5]).unwrap();
        let mut got = Vec::new();
        {
            let mut w = Limited {
                inner: &mut got,
                max: 2,
            };
            out.flush_to(&mut w).unwrap();
        }
        assert!(out.pending());
        assert_eq!(got, vec![1, 2]);
        {
            let mut w = Limited {
                inner: &mut got,
                max: 8,
            };
            out.flush_to(&mut w).unwrap();
        }
        assert!(!out.pending());
        assert_eq!(got, vec![1, 2, 3, 4, 5]);
        out.queue(vec![9]).unwrap();
    }
}
