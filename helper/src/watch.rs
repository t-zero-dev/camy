//! One `poll` over camera TCP, ctl socket, stdin, and USB inotify.
//! Idle GET is a pending RPC, not a blocking walk.

use std::io::{self, BufRead};
use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use yolocam::client::Client;
use yolocam::discover::{self, Discovery};
use yolocam::params::{self, Param};
use yolocam::pb;
use yolocam::preview::Preview;
use yolocam::usbwatch::PlugWatch;
use yolocam::{Error, Result};

use super::{connect_watch, ctl_path, emit};

#[derive(Clone, Copy, Debug)]
enum SetBody {
    Int(u32),
    Rect(pb::RectF),
}

impl SetBody {
    fn set_wire(self) -> Option<u32> {
        match self {
            Self::Int(w) => Some(w),
            Self::Rect(_) => None,
        }
    }

    fn set_rect(self) -> Option<pb::RectF> {
        match self {
            Self::Rect(r) => Some(r),
            Self::Int(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RpcBody {
    Get,
    Set(SetBody),
}

impl RpcBody {
    fn timeout_is_error(self) -> bool {
        matches!(self, Self::Set(SetBody::Int(_)))
    }

    fn set_wire(self) -> Option<u32> {
        match self {
            Self::Set(s) => s.set_wire(),
            Self::Get => None,
        }
    }

    fn set_rect(self) -> Option<pb::RectF> {
        match self {
            Self::Set(s) => s.set_rect(),
            Self::Get => None,
        }
    }
}

struct Pending {
    seq: u32,
    name: &'static str,
    cid: u32,
    started: Instant,
    body: RpcBody,
}

#[derive(Clone, Copy, Debug)]
struct QueuedSet {
    name: &'static str,
    cid: u32,
    body: SetBody,
}

struct Rpc {
    pending: Option<Pending>,
    queued: Option<QueuedSet>,
}

impl Rpc {
    fn new() -> Self {
        Self {
            pending: None,
            queued: None,
        }
    }

    fn clear(&mut self) {
        self.pending = None;
        self.queued = None;
    }
}

#[derive(Debug)]
enum SendKind {
    Set(QueuedSet),
    Get(&'static Param),
}

fn take_send(rpc: &mut Rpc, fill: &mut Fill, last_fill: Instant) -> Option<SendKind> {
    if let Some(q) = rpc.queued.take() {
        return Some(SendKind::Set(q));
    }
    fill.peek_due(last_fill).map(SendKind::Get)
}

struct Watch {
    disc: Discovery,
    client: Option<Client>,
    rpc: Rpc,
    fill: Fill,
    preview: Option<Preview>,
    last_slot: String,
    focus: FocusEmit,
    last_beat: Instant,
    last_reconnect: Instant,
    last_usb_fallback: Instant,
    last_fill: Instant,
    plugs: Option<PlugWatch>,
    usb_fallback: Option<Duration>,
    listener: UnixListener,
    sock_path: PathBuf,
}

pub fn run() -> Result<()> {
    Watch::open()?.run_loop()
}

impl Watch {
    fn open() -> Result<Self> {
        let disc = discover::discover();
        emit(serde_json::json!({"event":"discover","ok": true, "usb": disc.usb, "video": disc.video, "cameraIp": disc.camera_ip, "iface": disc.iface, "serial": disc.serial, "product": disc.product}));
        if !disc.usb {
            emit(serde_json::json!({"event":"disconnected","error":"camera not on USB"}));
        }

        let _dir = yolocam::preview::ensure_runtime_dir()?;
        let sock_path = ctl_path();
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).map_err(|e| Error::io_ctx(e, "ctl.sock"))?;
        listener.set_nonblocking(true).ok();
        yolocam::preview::restrict_mode(&sock_path, 0o700)?;

        let client = open_client(&disc);
        let plugs = PlugWatch::new();
        // inotify is the plug signal. Only walk sysfs on a timer if that watch
        // could not be created.
        let usb_fallback = if plugs.is_some() {
            None
        } else {
            Some(Duration::from_millis(800))
        };
        Ok(Self {
            disc,
            client,
            rpc: Rpc::new(),
            fill: Fill::new("exposure"),
            preview: None,
            last_slot: String::new(),
            focus: FocusEmit::default(),
            last_beat: Instant::now(),
            last_reconnect: Instant::now(),
            last_usb_fallback: Instant::now(),
            last_fill: Instant::now(),
            plugs,
            usb_fallback,
            listener,
            sock_path,
        })
    }

    fn run_loop(mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        loop {
            if !self.poll_once(&mut input, &mut line) {
                break;
            }
        }
        let _ = std::fs::remove_file(&self.sock_path);
        Ok(())
    }

    fn poll_once(&mut self, input: &mut (impl BufRead + AsFd), line: &mut String) -> bool {
        self.tick_usb();
        self.tick_heartbeat();
        let (stdin_hit, ctl_hit, plug_hit, cam_hit) = self.poll_fds(input);
        if plug_hit && self.plugs.as_ref().is_some_and(PlugWatch::changed) {
            self.rediscover();
        }
        self.flush_camera();
        if cam_hit {
            self.on_camera();
        }
        self.expire_pending();
        self.kick_set();
        drain_preview(&mut self.preview, &mut self.last_slot, &self.disc);
        if ctl_hit {
            self.on_ctl();
        }
        if stdin_hit {
            line.clear();
            if input.read_line(line).unwrap_or(0) == 0 {
                return false;
            }
            let line = line.trim();
            if !line.is_empty() {
                self.on_stdin(line);
            }
        }
        if self.fill.take_refreshed(self.rpc.pending.is_none()) {
            emit(serde_json::json!({"event":"refreshed"}));
        }
        true
    }

    fn tick_usb(&mut self) {
        let fallback_due = self
            .usb_fallback
            .is_some_and(|gap| self.last_usb_fallback.elapsed() > gap);
        if self.plugs.as_ref().is_some_and(PlugWatch::changed) || fallback_due {
            self.last_usb_fallback = Instant::now();
            self.rediscover();
        }
        if self.client.is_none() && self.disc.usb && self.last_reconnect.elapsed() > Duration::from_secs(3)
        {
            self.last_reconnect = Instant::now();
            self.client = open_client(&self.disc);
            if self.client.is_some() {
                self.fill.reset();
                self.rpc.clear();
                self.last_fill = Instant::now();
            }
        }
    }

    fn tick_heartbeat(&mut self) {
        let Some(mut c) = self.client.take() else {
            return;
        };
        if self.last_beat.elapsed() > Duration::from_secs(25)
            && c.pace_ready()
            && self.rpc.queued.is_none()
        {
            match c.queue_heartbeat() {
                Ok(()) => self.last_beat = Instant::now(),
                Err(e) if e.is_transient_write() => {}
                Err(e) => {
                    drop(c);
                    self.disconnect(&e);
                    return;
                }
            }
        }
        self.client = Some(c);
    }

    fn poll_timeout(&self) -> PollTimeout {
        // Slot files are not in the poll set. Preview display is ~15 fps, so
        // drain at least that often while the viewfinder is on.
        let mut wait = if self.preview.is_some() {
            Duration::from_millis(40)
        } else {
            Duration::from_millis(250)
        };
        if let Some(p) = &self.rpc.pending {
            wait = wait.min(
                Duration::from_millis(400)
                    .saturating_sub(p.started.elapsed())
                    .max(Duration::from_millis(5)),
            );
        }
        if self.client.is_none() && self.disc.usb {
            wait = wait.min(
                Duration::from_secs(3)
                    .saturating_sub(self.last_reconnect.elapsed())
                    .max(Duration::from_millis(20)),
            );
        }
        if self.client.is_some() && self.preview.is_none() {
            wait = wait.min(
                Duration::from_secs(25)
                    .saturating_sub(self.last_beat.elapsed())
                    .max(Duration::from_millis(20)),
            );
        }
        PollTimeout::try_from(wait).unwrap_or(PollTimeout::ZERO)
    }

    fn poll_fds(&self, input: &impl AsFd) -> (bool, bool, bool, bool) {
        let timeout = self.poll_timeout();
        let mut fds: Vec<PollFd> = Vec::with_capacity(4);
        fds.push(PollFd::new(input.as_fd(), PollFlags::POLLIN));
        fds.push(PollFd::new(self.listener.as_fd(), PollFlags::POLLIN));
        if let Some(p) = self.plugs.as_ref() {
            fds.push(PollFd::new(p.fd(), PollFlags::POLLIN));
        }
        if let Some(c) = self.client.as_ref() {
            let mut flags = PollFlags::POLLIN;
            if c.write_pending() {
                flags |= PollFlags::POLLOUT;
            }
            fds.push(PollFd::new(c.fd(), flags));
        }
        let _ = poll(&mut fds, timeout);
        let stdin_hit = fds[0].any() == Some(true);
        let ctl_hit = fds[1].any() == Some(true);
        let mut idx = 2;
        let plug_hit = if self.plugs.is_some() {
            let hit = fds[idx].any() == Some(true);
            idx += 1;
            hit
        } else {
            false
        };
        let cam_hit = self.client.is_some() && fds.get(idx).is_some_and(|f| f.any() == Some(true));
        (stdin_hit, ctl_hit, plug_hit, cam_hit)
    }

    fn flush_camera(&mut self) {
        let Some(mut c) = self.client.take() else {
            return;
        };
        if c.write_pending() {
            match c.flush_write() {
                Ok(()) => {}
                Err(e) if e.is_transient_write() => {}
                Err(e) => {
                    drop(c);
                    self.disconnect(&e);
                    return;
                }
            }
        }
        self.client = Some(c);
    }

    fn on_camera(&mut self) {
        let Some(mut c) = self.client.take() else {
            return;
        };
        match c.try_read() {
            Ok(msgs) => {
                self.client = Some(c);
                dispatch(&mut self.focus, &mut self.rpc.pending, msgs);
            }
            Err(e) => self.disconnect(&e),
        }
    }

    fn expire_pending(&mut self) {
        let timed_out = self
            .rpc
            .pending
            .as_ref()
            .is_some_and(|p| p.started.elapsed() > Duration::from_millis(400));
        if timed_out {
            if let Some(p) = self.rpc.pending.take() {
                if let Some(ev) = set_timeout_event(&p) {
                    emit(ev);
                }
            }
        }
    }

    fn kick_set(&mut self) {
        if self.rpc.pending.is_some() {
            return;
        }
        let Some(mut c) = self.client.take() else {
            return;
        };
        if !c.pace_ready() {
            self.client = Some(c);
            return;
        }
        let send_err = match take_send(&mut self.rpc, &mut self.fill, self.last_fill) {
            Some(SendKind::Set(q)) => {
                let started = Instant::now();
                let send = match q.body {
                    SetBody::Int(wire) => c.begin_set(q.cid, wire),
                    SetBody::Rect(rect) => c.begin_set_rect(q.cid, rect),
                };
                match send {
                    Ok(seq) => {
                        if let Some(r) = q.body.set_rect() {
                            emit(serde_json::json!({"event":"param","name": q.name, "cid": q.cid, "resp": 0, "value": {"x": r.x, "y": r.y, "w": r.w, "h": r.h}}));
                            emit(serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}));
                        }
                        self.rpc.pending = Some(Pending {
                            seq,
                            name: q.name,
                            cid: q.cid,
                            started,
                            body: RpcBody::Set(q.body),
                        });
                        None
                    }
                    Err(e) if e.is_transient_write() => {
                        self.rpc.queued = Some(q);
                        None
                    }
                    Err(e) => Some(e),
                }
            }
            Some(SendKind::Get(p)) => match c.begin_get(p.cid) {
                Ok(seq) => {
                    self.fill.commit();
                    self.last_fill = Instant::now();
                    self.rpc.pending = Some(Pending {
                        seq,
                        name: p.name,
                        cid: p.cid,
                        started: Instant::now(),
                        body: RpcBody::Get,
                    });
                    None
                }
                Err(e) if e.is_transient_write() => None,
                Err(e) => Some(e),
            },
            None => None,
        };
        if let Some(e) = send_err {
            drop(c);
            self.disconnect(&e);
            return;
        }
        self.client = Some(c);
    }

    fn on_stdin(&mut self, line: &str) {
        match self.handle_cmd(line) {
            Ok(Some(v)) => emit(v),
            Ok(None) => {}
            Err(e) => emit(serde_json::json!({"event":"error","error": e.to_string()})),
        }
    }

    fn on_ctl(&mut self) {
        if let Ok((stream, _)) = self.listener.accept() {
            stream.set_nonblocking(false).ok();
            let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
            let mut buf = String::new();
            let mut reader = io::BufReader::new(&stream);
            if reader.read_line(&mut buf).unwrap_or(0) > 0 {
                let line = buf.trim();
                if !line.is_empty() {
                    self.on_stdin(line);
                }
            }
        }
    }

    fn disconnect(&mut self, e: &Error) {
        self.client = None;
        self.rpc.clear();
        emit(serde_json::json!({"event":"disconnected","error": e.to_string()}));
        // Power-off drops TCP first. Recheck USB so the bar can go outline
        // instead of sitting on "USB only".
        self.rediscover();
    }

    fn rediscover(&mut self) {
        let next = discover::discover();
        if next.usb == self.disc.usb && next.video == self.disc.video && next.iface == self.disc.iface {
            return;
        }
        emit(serde_json::json!({
            "event":"discover","ok": true, "usb": next.usb, "video": next.video,
            "cameraIp": next.camera_ip, "iface": next.iface, "serial": next.serial,
            "product": next.product
        }));
        if !next.usb {
            self.preview = None;
            self.client = None;
            self.rpc.clear();
            emit(serde_json::json!({"event":"disconnected","error":"unplugged"}));
        } else if self.preview.is_some() && next.video != self.disc.video {
            self.preview = None;
            emit(serde_json::json!({"event":"preview","on": false}));
        }
        self.disc = next;
    }

    #[allow(clippy::too_many_lines)]
    fn handle_cmd(&mut self, line: &str) -> Result<Option<serde_json::Value>> {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| Error::Protocol(format!("json: {e}")))?;
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        match cmd {
            "tab" => {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("exposure");
                self.fill.set_tab(name);
                Ok(None)
            }
            "get-all" => {
                self.fill.refresh();
                self.rpc.pending = None;
                Ok(None)
            }
            "set" => {
                let name = v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| Error::Invalid("name".into()))?;
                let raw = v
                    .get("value")
                    .map(|x| match x {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .ok_or_else(|| Error::Invalid("value".into()))?;
                let p = params::find(name).ok_or_else(|| Error::Invalid(format!("unknown {name}")))?;
                if !p.set_ok {
                    return Err(Error::Invalid(format!("{name} is read-only")));
                }
                if self.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let wire = params::encode_named(p, raw.trim_matches('"'))?;
                self.rpc.queued = Some(QueuedSet {
                    name: p.name,
                    cid: p.cid,
                    body: SetBody::Int(wire),
                });
                Ok(None)
            }
            "af-target" | "af-area" => {
                if self.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let rect = json_norm_rect(&v)?;
                // CID 34 is a notify. Both AF-S and AF-C write CID 24.
                self.rpc.queued = Some(QueuedSet {
                    name: if cmd == "af-target" {
                        "focus-target"
                    } else {
                        "focus-area"
                    },
                    cid: params::FOCUS_TARGET_CID,
                    body: SetBody::Rect(rect),
                });
                Ok(None)
            }
            "heartbeat" => {
                if let Some(c) = self.client.as_mut() {
                    if c.pace_ready() {
                        match c.queue_heartbeat() {
                            Ok(()) => {}
                            Err(e) if e.is_transient_write() => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
                Ok(None)
            }
            "preview" => {
                let on = v.get("on").and_then(serde_json::Value::as_bool).unwrap_or(false);
                if on {
                    if self.preview.is_some() {
                        return Ok(Some(serde_json::json!({"event":"preview","on": true})));
                    }
                    let dev = self
                        .disc
                        .video
                        .clone()
                        .ok_or_else(|| Error::Invalid("no YoloCam UVC device".into()))?;
                    match Preview::start(&dev, 1280, 720, 15) {
                        Ok(p) => {
                            let dir = p.dir.clone();
                            self.last_slot.clear();
                            self.preview = Some(p);
                            Ok(Some(serde_json::json!({"event":"preview","on": true, "device": dev, "dir": dir, "size": "1280x720"})))
                        }
                        Err(yolocam::Error::Busy) => {
                            let holder = yolocam::holders::summarize(&dev);
                            Ok(Some(serde_json::json!({"event":"preview","on": false, "error": "busy", "holder": holder.name, "holderKind": holder.kind})))
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    self.preview = None;
                    Ok(Some(serde_json::json!({"event":"preview","on": false})))
                }
            }
            "reconnect" => {
                self.rpc.clear();
                // `open_client` emits connected or disconnected. Do not also
                // emit connected when the TCP open failed.
                self.client = open_client(&self.disc);
                Ok(None)
            }
            _ => Err(Error::Invalid(format!("unknown cmd {cmd}"))),
        }
    }
}

fn open_client(disc: &Discovery) -> Option<Client> {
    if !disc.usb {
        return None;
    }
    match connect_watch(disc) {
        Ok(mut c) => {
            let _ = c.set_nonblocking(true);
            let _ = c.queue_heartbeat();
            emit(serde_json::json!({"event":"connected"}));
            Some(c)
        }
        Err(e) => {
            emit(serde_json::json!({"event":"disconnected","error": e.to_string()}));
            None
        }
    }
}

fn dispatch(focus: &mut FocusEmit, pending: &mut Option<Pending>, msgs: Vec<pb::Message>) {
    for mut msg in msgs {
        if let Some(p) = pending.as_ref() {
            if msg.is_reply_to(p.seq, p.cid) {
                let name = p.name;
                let cid = p.cid;
                let body = p.body;
                *pending = None;
                let value = set_param_value(name, body.set_wire(), &msg);
                emit(serde_json::json!({"event":"param","name": name, "cid": cid, "resp": msg.resp_code, "value": value}));
                if let Some(r) = body.set_rect() {
                    if cid == params::FOCUS_AREA_CID || cid == params::FOCUS_TARGET_CID {
                        emit(serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}));
                    }
                }
                continue;
            }
        }
        if msg.is_notify() && msg.id == params::FACE_BOX_CID {
            if let pb::Body::Rects(ref mut rects) = msg.body {
                *rects = params::pick_primary_face(rects);
            }
            if let Some(ev) = focus.event("focus-box", &msg) {
                emit(ev);
            }
            continue;
        }
        if msg.is_notify() && msg.id == params::FOCUS_AREA_CID {
            if let Some(ev) = focus.event("focus-area", &msg) {
                emit(ev);
            }
            continue;
        }
        if let Some(p) = params::find_cid(msg.id) {
            if !msg.is_notify() {
                emit(serde_json::json!({"event":"param","name":p.name,"cid":p.cid,"resp":msg.resp_code,"value": params::body_json(&msg)}));
            }
        } else if !msg.is_notify() {
            emit(serde_json::json!({"event":"raw","cid":msg.id,"type":msg.ty.map(|t| t as u32),"resp":msg.resp_code,"value": params::body_json(&msg)}));
        }
    }
}

#[derive(Default)]
struct FocusEmit {
    last_face: Vec<(i16, i16, i16, i16)>,
    last_area: Vec<(i16, i16, i16, i16)>,
}

impl FocusEmit {
    fn event(&mut self, name: &str, msg: &pb::Message) -> Option<serde_json::Value> {
        let rects = match &msg.body {
            pb::Body::Rects(r) => r.as_slice(),
            _ => &[],
        };
        let key: Vec<_> = rects
            .iter()
            .map(|r| {
                (
                    (r.x * 200.0).round() as i16,
                    (r.y * 200.0).round() as i16,
                    (r.w * 200.0).round() as i16,
                    (r.h * 200.0).round() as i16,
                )
            })
            .collect();
        let last = if name == "focus-area" {
            &mut self.last_area
        } else {
            &mut self.last_face
        };
        if key == *last {
            return None;
        }
        *last = key;
        let json: Vec<_> = rects
            .iter()
            .map(|r| serde_json::json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h}))
            .collect();
        Some(serde_json::json!({"event": name, "rects": json}))
    }
}

fn drain_preview(preview: &mut Option<Preview>, last_slot: &mut String, disc: &Discovery) {
    let Some(p) = preview.as_ref() else {
        return;
    };
    if let Ok(slot) = std::fs::read_to_string(p.dir.join("slot")) {
        if slot != *last_slot {
            last_slot.clone_from(&slot);
            let file = if slot.trim() == "a" {
                p.dir.join("preview-a.jpg")
            } else {
                p.dir.join("preview-b.jpg")
            };
            let covered =
                std::fs::read_to_string(p.dir.join("covered")).is_ok_and(|s| s.trim() == "1");
            emit(serde_json::json!({"event":"frame","slot": slot.trim(), "path": file, "covered": covered}));
        }
    }
    if p.dir.join("error").exists() {
        if let Ok(e) = std::fs::read_to_string(p.dir.join("error")) {
            let err = e.trim().to_string();
            let holder = if err == "busy" {
                disc.video.as_deref().map(yolocam::holders::summarize)
            } else {
                None
            };
            emit(serde_json::json!({
                "event":"preview-error",
                "error": err,
                "holder": holder.as_ref().map(|h| &h.name),
                "holderKind": holder.as_ref().map(|h| h.kind)
            }));
            let _ = std::fs::remove_file(p.dir.join("error"));
            *preview = None;
        }
    }
}

struct Fill {
    at: usize,
    order: Vec<usize>,
    group: &'static str,
    urgent: bool,
}

impl Fill {
    fn new(group: &'static str) -> Self {
        Self {
            at: 0,
            order: params::fill_order(group),
            group,
            urgent: false,
        }
    }

    fn reset(&mut self) {
        self.at = 0;
        self.order = params::fill_order(self.group);
        self.urgent = false;
    }

    fn refresh(&mut self) {
        self.at = 0;
        self.order = params::refresh_order(self.group);
        self.urgent = true;
    }

    fn set_tab(&mut self, tab: &str) {
        self.group = params::tab_group(tab);
        params::prefer_group(&mut self.order, self.at, self.group);
    }

    fn peek_due(&self, last_fill: Instant) -> Option<&'static Param> {
        if self.at >= self.order.len() {
            return None;
        }
        if !self.urgent && last_fill.elapsed() <= Duration::from_millis(220) {
            return None;
        }
        params::PARAMS.get(self.order[self.at])
    }

    fn commit(&mut self) {
        if self.at < self.order.len() {
            self.at += 1;
        }
    }

    fn take_refreshed(&mut self, idle: bool) -> bool {
        if self.urgent && idle && self.at >= self.order.len() {
            self.urgent = false;
            true
        } else {
            false
        }
    }
}

fn set_param_value(name: &'static str, set_wire: Option<u32>, msg: &pb::Message) -> serde_json::Value {
    if let Some(wire) = set_wire {
        if matches!(msg.body, pb::Body::Empty) {
            return params::find(name).map_or(serde_json::json!(wire), |p| {
                serde_json::json!(params::decode_int(p, wire))
            });
        }
    }
    params::body_json(msg)
}

fn set_timeout_event(p: &Pending) -> Option<serde_json::Value> {
    if !p.body.timeout_is_error() {
        return None;
    }
    Some(serde_json::json!({
        "event": "param",
        "name": p.name,
        "cid": p.cid,
        "error": format!("timeout waiting for cid {}", p.cid),
        "value": serde_json::Value::Null
    }))
}

fn json_f32(v: &serde_json::Value, key: &str) -> Result<f32> {
    v.get(key)
        .and_then(serde_json::Value::as_f64)
        .map(|n| n as f32)
        .ok_or_else(|| Error::Invalid(format!("missing {key}")))
}

fn json_norm_rect(v: &serde_json::Value) -> Result<pb::RectF> {
    let r = params::clamp_norm_rect(pb::RectF {
        x: json_f32(v, "x")?,
        y: json_f32(v, "y")?,
        w: json_f32(v, "w")?,
        h: json_f32(v, "h")?,
    });
    if r.w < 0.03 || r.h < 0.03 {
        return Err(Error::Invalid("af box too small".into()));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::{Fill, RpcBody, SetBody};
    use std::time::{Duration, Instant};
    use yolocam::params::{self, Kind};

    #[test]
    fn refresh_queues_open_tab_only() {
        let mut fill = Fill::new("image");
        fill.refresh();
        assert!(fill.urgent);
        assert!(!fill.order.is_empty());
        assert!(fill
            .order
            .iter()
            .all(|&i| params::PARAMS[i].group == "image"));
        assert!(fill
            .order
            .iter()
            .all(|&i| !matches!(params::PARAMS[i].kind, Kind::Info)));
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("clock");
        let n = fill.order.len();
        let mut got = 0;
        while fill.peek_due(past).is_some() {
            fill.commit();
            got += 1;
        }
        assert_eq!(got, n);
        assert!(fill.take_refreshed(true));
        assert!(!fill.urgent);
        assert!(!fill.take_refreshed(true));
    }

    #[test]
    fn peek_does_not_advance() {
        let mut fill = Fill::new("zoom");
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("clock");
        let first = fill.peek_due(past).expect("zoom params");
        assert_eq!(fill.at, 0);
        let again = fill.peek_due(past).expect("same cid");
        assert_eq!(first.name, again.name);
        fill.commit();
        assert_eq!(fill.at, 1);
    }

    #[test]
    fn idle_fill_respects_gap() {
        let fill = Fill::new("zoom");
        assert!(fill.peek_due(Instant::now()).is_none());
    }

    #[test]
    fn set_wins_over_fill() {
        let mut rpc = super::Rpc::new();
        let mut fill = Fill::new("exposure");
        let past = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("clock");
        rpc.queued = Some(super::QueuedSet {
            name: "iso",
            cid: 17,
            body: SetBody::Int(200),
        });
        match super::take_send(&mut rpc, &mut fill, past) {
            Some(super::SendKind::Set(q)) => assert_eq!(q.name, "iso"),
            other => panic!("expected set, got {other:?}"),
        }
        assert!(rpc.queued.is_none());
    }

    #[test]
    fn last_set_replaces_queue() {
        let mut rpc = super::Rpc::new();
        rpc.queued = Some(super::QueuedSet {
            name: "iso",
            cid: 17,
            body: SetBody::Int(100),
        });
        rpc.queued = Some(super::QueuedSet {
            name: "ev",
            cid: 15,
            body: SetBody::Int(8),
        });
        assert_eq!(rpc.queued.unwrap().name, "ev");
    }

    #[test]
    fn set_timeout_emits_error_not_success() {
        let p = super::Pending {
            seq: 1,
            name: "iso",
            cid: 17,
            started: Instant::now(),
            body: RpcBody::Set(SetBody::Int(200)),
        };
        let ev = super::set_timeout_event(&p).expect("set timeout event");
        assert_eq!(ev["event"], "param");
        assert!(ev["error"].as_str().unwrap().contains("timeout"));
        assert!(ev["value"].is_null());
    }

    #[test]
    fn rect_set_timeout_is_silent() {
        let p = super::Pending {
            seq: 1,
            name: "focus-area",
            cid: 24,
            started: Instant::now(),
            body: RpcBody::Set(SetBody::Rect(yolocam::pb::RectF {
                x: 0.2,
                y: 0.2,
                w: 0.3,
                h: 0.3,
            })),
        };
        assert!(super::set_timeout_event(&p).is_none());
    }

    #[test]
    fn get_timeout_emits_nothing() {
        let p = super::Pending {
            seq: 1,
            name: "iso",
            cid: 17,
            started: Instant::now(),
            body: RpcBody::Get,
        };
        assert!(super::set_timeout_event(&p).is_none());
    }

    #[test]
    fn rpc_body_flags_cannot_diverge() {
        assert!(RpcBody::Set(SetBody::Int(1)).timeout_is_error());
        assert!(!RpcBody::Set(SetBody::Rect(yolocam::pb::RectF::default())).timeout_is_error());
        assert!(!RpcBody::Get.timeout_is_error());
        assert_eq!(RpcBody::Set(SetBody::Int(9)).set_wire(), Some(9));
        assert!(RpcBody::Set(SetBody::Rect(yolocam::pb::RectF::default()))
            .set_wire()
            .is_none());
        assert!(RpcBody::Get.set_rect().is_none());
    }
}
