//! One `poll` over camera TCP, ctl socket, stdin, and USB inotify.
//! Idle GET is a pending RPC, not a blocking walk.
//!
//! At most one S7 and one S3 session. Commands without `model`/`id` hit the
//! selected camera (S7 if both just appeared).

use std::io::{self, BufRead};
use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use yolocam::client::Client;
use yolocam::discover::{self, Discovery, Inventory, Model};
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
    Wb { r: u32, b: u32 },
}

impl SetBody {
    fn set_wire(self) -> Option<u32> {
        match self {
            Self::Int(w) => Some(w),
            Self::Rect(_) | Self::Wb { .. } => None,
        }
    }

    fn set_rect(self) -> Option<pb::RectF> {
        match self {
            Self::Rect(r) => Some(r),
            Self::Int(_) | Self::Wb { .. } => None,
        }
    }

    fn set_wb(self) -> Option<(u32, u32)> {
        match self {
            Self::Wb { r, b } => Some((r, b)),
            _ => None,
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
        matches!(self, Self::Set(SetBody::Int(_) | SetBody::Wb { .. }))
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

struct Session {
    disc: Discovery,
    client: Option<Client>,
    rpc: Rpc,
    fill: Fill,
    preview: Option<Preview>,
    last_slot: String,
    focus: FocusEmit,
    last_beat: Instant,
    last_reconnect: Instant,
    last_fill: Instant,
    last_covered: Option<bool>,
    unsupported: Vec<&'static str>,
}

impl Session {
    fn new(disc: Discovery) -> Self {
        let client = open_client(&disc);
        let unsupported = match disc.model {
            Model::S3 => vec![
                "hue",
                "gamma",
                "zoom",
                "lens-detail",
                "mic-type",
                "audio-volume",
                "audio-denoise",
                "audio-reverb",
                "audio-delay",
                "audio-agc",
                "hdmi-format",
                "hdmi-color",
                "brightness",
                "distortion",
                "clut",
                "clut-mix",
            ],
            Model::S7 => Vec::new(),
        };
        Self {
            disc,
            client,
            rpc: Rpc::new(),
            fill: Fill::new("exposure"),
            preview: None,
            last_slot: String::new(),
            focus: FocusEmit::default(),
            last_beat: Instant::now(),
            last_reconnect: Instant::now(),
            last_fill: Instant::now(),
            last_covered: None,
            unsupported,
        }
    }
}

struct Watch {
    s7: Option<Session>,
    s3: Option<Session>,
    selected: Model,
    last_usb_fallback: Instant,
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
        let _dir = yolocam::preview::ensure_runtime_dir()?;
        let sock_path = ctl_path();
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).map_err(|e| Error::io_ctx(e, "ctl.sock"))?;
        listener.set_nonblocking(true).ok();
        yolocam::preview::restrict_mode(&sock_path, 0o700)?;

        let plugs = PlugWatch::new();
        let usb_fallback = if plugs.is_some() {
            None
        } else {
            Some(Duration::from_millis(800))
        };
        let mut w = Self {
            s7: None,
            s3: None,
            selected: Model::S7,
            last_usb_fallback: Instant::now(),
            plugs,
            usb_fallback,
            listener,
            sock_path,
        };
        w.sync_inventory(true);
        Ok(w)
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
        self.tick_heartbeat(Model::S7);
        self.tick_heartbeat(Model::S3);
        let hits = self.poll_fds(input);
        if hits.plug && self.plugs.as_ref().is_some_and(PlugWatch::changed) {
            self.sync_inventory(false);
        }
        self.flush_camera(Model::S7);
        self.flush_camera(Model::S3);
        if hits.s7 {
            self.on_camera(Model::S7);
        }
        if hits.s3 {
            self.on_camera(Model::S3);
        }
        self.expire_pending(Model::S7);
        self.expire_pending(Model::S3);
        self.kick_set(Model::S7);
        self.kick_set(Model::S3);
        self.drain_preview(Model::S7);
        self.drain_preview(Model::S3);
        if hits.ctl {
            self.on_ctl();
        }
        if hits.stdin {
            line.clear();
            if input.read_line(line).unwrap_or(0) == 0 {
                return false;
            }
            let line = line.trim();
            if !line.is_empty() {
                self.on_stdin(line);
            }
        }
        self.emit_refreshed(Model::S7);
        self.emit_refreshed(Model::S3);
        true
    }

    fn slot(&self, model: Model) -> Option<&Session> {
        match model {
            Model::S7 => self.s7.as_ref(),
            Model::S3 => self.s3.as_ref(),
        }
    }

    fn slot_mut(&mut self, model: Model) -> Option<&mut Session> {
        match model {
            Model::S7 => self.s7.as_mut(),
            Model::S3 => self.s3.as_mut(),
        }
    }

    fn tick_usb(&mut self) {
        let fallback_due = self
            .usb_fallback
            .is_some_and(|gap| self.last_usb_fallback.elapsed() > gap);
        if self.plugs.as_ref().is_some_and(PlugWatch::changed) || fallback_due {
            self.last_usb_fallback = Instant::now();
            self.sync_inventory(false);
        }
        self.try_reconnect(Model::S7);
        self.try_reconnect(Model::S3);
    }

    fn try_reconnect(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        if s.client.is_none() && s.disc.usb && s.last_reconnect.elapsed() > Duration::from_secs(3) {
            s.last_reconnect = Instant::now();
            s.client = open_client(&s.disc);
            if s.client.is_some() {
                s.fill.reset();
                s.rpc.clear();
                s.last_fill = Instant::now();
            }
        }
    }

    fn tick_heartbeat(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        let Some(mut c) = s.client.take() else {
            return;
        };
        if s.last_beat.elapsed() > Duration::from_secs(25) && c.pace_ready() && s.rpc.queued.is_none()
        {
            match c.queue_heartbeat() {
                Ok(()) => s.last_beat = Instant::now(),
                Err(e) if e.is_transient_write() => {}
                Err(e) => {
                    drop(c);
                    disconnect_session(s, &e);
                    return;
                }
            }
        }
        s.client = Some(c);
    }

    fn poll_timeout(&self) -> PollTimeout {
        let mut wait = Duration::from_millis(250);
        for s in [self.s7.as_ref(), self.s3.as_ref()].into_iter().flatten() {
            if s.preview.is_some() {
                wait = wait.min(Duration::from_millis(40));
            }
            if let Some(p) = &s.rpc.pending {
                wait = wait.min(
                    Duration::from_millis(400)
                        .saturating_sub(p.started.elapsed())
                        .max(Duration::from_millis(5)),
                );
            }
            if s.client.is_none() && s.disc.usb {
                wait = wait.min(
                    Duration::from_secs(3)
                        .saturating_sub(s.last_reconnect.elapsed())
                        .max(Duration::from_millis(20)),
                );
            }
            if s.client.is_some() && s.preview.is_none() {
                wait = wait.min(
                    Duration::from_secs(25)
                        .saturating_sub(s.last_beat.elapsed())
                        .max(Duration::from_millis(20)),
                );
            }
        }
        PollTimeout::try_from(wait).unwrap_or(PollTimeout::ZERO)
    }

    fn poll_fds(&self, input: &impl AsFd) -> Hits {
        let timeout = self.poll_timeout();
        let mut fds: Vec<PollFd> = Vec::with_capacity(5);
        fds.push(PollFd::new(input.as_fd(), PollFlags::POLLIN));
        fds.push(PollFd::new(self.listener.as_fd(), PollFlags::POLLIN));
        if let Some(p) = self.plugs.as_ref() {
            fds.push(PollFd::new(p.fd(), PollFlags::POLLIN));
        }
        let s7_live = self.s7.as_ref().is_some_and(|s| s.client.is_some());
        let s3_live = self.s3.as_ref().is_some_and(|s| s.client.is_some());
        if let Some(c) = self.s7.as_ref().and_then(|s| s.client.as_ref()) {
            let mut flags = PollFlags::POLLIN;
            if c.write_pending() {
                flags |= PollFlags::POLLOUT;
            }
            fds.push(PollFd::new(c.fd(), flags));
        }
        if let Some(c) = self.s3.as_ref().and_then(|s| s.client.as_ref()) {
            let mut flags = PollFlags::POLLIN;
            if c.write_pending() {
                flags |= PollFlags::POLLOUT;
            }
            fds.push(PollFd::new(c.fd(), flags));
        }
        let _ = poll(&mut fds, timeout);
        let stdin = fds[0].any() == Some(true);
        let ctl = fds[1].any() == Some(true);
        let mut idx = 2;
        let plug = if self.plugs.is_some() {
            let hit = fds[idx].any() == Some(true);
            idx += 1;
            hit
        } else {
            false
        };
        let s7 = if s7_live {
            let hit = fds.get(idx).is_some_and(|f| f.any() == Some(true));
            idx += 1;
            hit
        } else {
            false
        };
        let s3 = s3_live && fds.get(idx).is_some_and(|f| f.any() == Some(true));
        Hits { stdin, ctl, plug, s7, s3 }
    }

    fn flush_camera(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        let Some(mut c) = s.client.take() else {
            return;
        };
        if c.write_pending() {
            match c.flush_write() {
                Ok(()) => {}
                Err(e) if e.is_transient_write() => {}
                Err(e) => {
                    drop(c);
                    disconnect_session(s, &e);
                    return;
                }
            }
        }
        s.client = Some(c);
    }

    fn on_camera(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        let Some(mut c) = s.client.take() else {
            return;
        };
        match c.try_read() {
            Ok(msgs) => {
                s.client = Some(c);
                dispatch(&s.disc, &mut s.focus, &mut s.rpc.pending, &mut s.unsupported, msgs);
            }
            Err(e) => disconnect_session(s, &e),
        }
    }

    fn expire_pending(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        let timed_out = s
            .rpc
            .pending
            .as_ref()
            .is_some_and(|p| p.started.elapsed() > Duration::from_millis(400));
        if timed_out {
            if let Some(p) = s.rpc.pending.take() {
                if let Some(ev) = set_timeout_event(&p) {
                    emit_cam(&s.disc, ev);
                }
            }
        }
    }

    fn any_preview(&self) -> bool {
        self.s7.as_ref().is_some_and(|s| s.preview.is_some())
            || self.s3.as_ref().is_some_and(|s| s.preview.is_some())
    }

    fn kick_set(&mut self, model: Model) {
        let hold_gets = self.any_preview();
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        if s.rpc.pending.is_some() {
            return;
        }
        let Some(mut c) = s.client.take() else {
            return;
        };
        if !c.pace_ready() {
            s.client = Some(c);
            return;
        }
        let send_err = match take_send(&mut s.rpc, &mut s.fill, s.last_fill) {
            Some(SendKind::Set(q)) => {
                let started = Instant::now();
                let send = match q.body {
                    SetBody::Int(wire) => c.begin_set(q.cid, wire),
                    SetBody::Rect(rect) => c.begin_set_rect(q.cid, rect),
                    SetBody::Wb { r, b } => c.begin_set_wb(q.cid, r, b),
                };
                match send {
                    Ok(seq) => {
                        if let Some(r) = q.body.set_rect() {
                            emit_cam(&s.disc, serde_json::json!({"event":"param","name": q.name, "cid": q.cid, "resp": 0, "value": {"x": r.x, "y": r.y, "w": r.w, "h": r.h}}));
                            emit_cam(&s.disc, serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}));
                        }
                        if let Some((wr, wb)) = q.body.set_wb() {
                            emit_cam(&s.disc, serde_json::json!({"event":"param","name": "wb-offset", "cid": q.cid, "resp": 0, "value": {"r": wr, "b": wb}}));
                        }
                        s.rpc.pending = Some(Pending {
                            seq,
                            name: q.name,
                            cid: q.cid,
                            started,
                            body: RpcBody::Set(q.body),
                        });
                        None
                    }
                    Err(e) if e.is_transient_write() => {
                        s.rpc.queued = Some(q);
                        None
                    }
                    Err(e) => Some(e),
                }
            }
            Some(SendKind::Get(_)) if hold_gets && !s.fill.is_urgent() => None,
            Some(SendKind::Get(p)) if s.unsupported.contains(&p.name) => {
                s.fill.commit();
                None
            }
            Some(SendKind::Get(p)) => match c.begin_get(p.cid) {
                Ok(seq) => {
                    s.fill.commit();
                    s.last_fill = Instant::now();
                    s.rpc.pending = Some(Pending {
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
            disconnect_session(s, &e);
            return;
        }
        s.client = Some(c);
    }

    fn drain_preview(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        drain_preview(s);
    }

    fn emit_refreshed(&mut self, model: Model) {
        let Some(s) = self.slot_mut(model) else {
            return;
        };
        if s.fill.take_refreshed(s.rpc.pending.is_none()) {
            emit_cam(&s.disc, serde_json::json!({"event":"refreshed"}));
        }
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

    fn sync_inventory(&mut self, force: bool) {
        let inv = discover::discover_all();
        let before = Inventory {
            s7: self.s7.as_ref().map(|s| s.disc.clone()),
            s3: self.s3.as_ref().map(|s| s.disc.clone()),
        };
        if !force && before.same_plug(&inv) {
            return;
        }
        sync_slot(&mut self.s7, inv.s7.clone());
        sync_slot(&mut self.s3, inv.s3.clone());
        if self.slot(self.selected).is_none() {
            self.selected = if self.s7.as_ref().is_some_and(|s| s.disc.usb) {
                Model::S7
            } else {
                Model::S3
            };
        }
        self.emit_state();
        if self.s7.is_none() && self.s3.is_none() {
            emit(serde_json::json!({"event":"disconnected","error":"camera not on USB"}));
        }
    }

    fn emit_state(&self) {
        emit(inventory_json(
            &Inventory {
                s7: self.s7.as_ref().map(|s| s.disc.clone()),
                s3: self.s3.as_ref().map(|s| s.disc.clone()),
            },
            self.selected,
            self.s7.as_ref().is_some_and(|s| s.client.is_some()),
            self.s3.as_ref().is_some_and(|s| s.client.is_some()),
        ));
        if let Some(s) = self.slot(self.selected) {
            emit_cam(&s.disc, discover_json(&s.disc));
            if !s.unsupported.is_empty() {
                emit_cam(
                    &s.disc,
                    serde_json::json!({"event":"caps","unsupported": s.unsupported}),
                );
            }
        }
    }

    fn target_model(&self, v: &serde_json::Value) -> Model {
        if let Some(m) = v.get("model").and_then(|x| x.as_str()).and_then(parse_model) {
            return m;
        }
        if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
            if self.s7.as_ref().is_some_and(|s| s.disc.serial == id) {
                return Model::S7;
            }
            if self.s3.as_ref().is_some_and(|s| s.disc.serial == id) {
                return Model::S3;
            }
        }
        self.selected
    }

    #[allow(clippy::too_many_lines)]
    fn handle_cmd(&mut self, line: &str) -> Result<Option<serde_json::Value>> {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| Error::Protocol(format!("json: {e}")))?;
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        if cmd == "select" {
            let model = self.target_model(&v);
            if self.slot(model).is_none() {
                return Err(Error::Invalid(format!("{model:?} not plugged")));
            }
            if self.selected != model {
                if let Some(s) = self.slot_mut(self.selected) {
                    if s.preview.is_some() {
                        s.preview = None;
                        emit_cam(&s.disc, serde_json::json!({"event":"preview","on": false}));
                    }
                }
                self.selected = model;
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.clear();
                    s.fill.restart();
                }
                self.emit_state();
            }
            return Ok(None);
        }
        let model = self.target_model(&v);
        match cmd {
            "tab" => {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("exposure");
                if let Some(s) = self.slot_mut(model) {
                    s.fill.set_tab(name);
                }
                Ok(None)
            }
            "get-all" => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.pending = None;
                    s.fill.restart();
                }
                Ok(None)
            }
            "refresh" => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.pending = None;
                    s.fill.refresh();
                    if s.fill.is_done() {
                        emit_cam(&s.disc, serde_json::json!({"event":"refreshed"}));
                    }
                }
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
                if name == "wb-offset" {
                    let r = v.get("r").and_then(serde_json::Value::as_u64).unwrap_or(10) as u32;
                    let b = v.get("b").and_then(serde_json::Value::as_u64).unwrap_or(10) as u32;
                    let s = self
                        .slot_mut(model)
                        .ok_or(Error::Disconnected("disconnected"))?;
                    if s.client.is_none() {
                        return Err(Error::Disconnected("disconnected"));
                    }
                    s.rpc.queued = Some(QueuedSet {
                        name: "wb-offset",
                        cid: 33,
                        body: SetBody::Wb { r, b },
                    });
                    return Ok(None);
                }
                let p = params::find(name).ok_or_else(|| Error::Invalid(format!("unknown {name}")))?;
                if !p.set_ok {
                    return Err(Error::Invalid(format!("{name} is read-only")));
                }
                let s = self
                    .slot_mut(model)
                    .ok_or(Error::Disconnected("disconnected"))?;
                if s.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let wire = params::encode_named(p, raw.trim_matches('"'))?;
                s.rpc.queued = Some(QueuedSet {
                    name: p.name,
                    cid: p.cid,
                    body: SetBody::Int(wire),
                });
                Ok(None)
            }
            "af-target" | "af-area" => {
                let s = self
                    .slot_mut(model)
                    .ok_or(Error::Disconnected("disconnected"))?;
                if s.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let rect = json_norm_rect(&v)?;
                s.rpc.queued = Some(QueuedSet {
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
                if let Some(s) = self.slot_mut(model) {
                    if let Some(c) = s.client.as_mut() {
                        if c.pace_ready() {
                            match c.queue_heartbeat() {
                                Ok(()) => {}
                                Err(e) if e.is_transient_write() => {}
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Ok(None)
            }
            "preview" => {
                let on = v.get("on").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let s = self
                    .slot_mut(model)
                    .ok_or_else(|| Error::Invalid("no YoloCam UVC device".into()))?;
                if on {
                    if s.preview.is_some() {
                        return Ok(Some(tagged(&s.disc, serde_json::json!({"event":"preview","on": true}))));
                    }
                    let dev = s
                        .disc
                        .video
                        .clone()
                        .ok_or_else(|| Error::Invalid("no YoloCam UVC device".into()))?;
                    let serial = s.disc.serial.clone();
                    match Preview::start_in(&dev, 1280, 720, 15, Some(serial.as_str())) {
                        Ok(p) => {
                            let dir = p.dir.clone();
                            s.last_slot.clear();
                            s.preview = Some(p);
                            Ok(Some(tagged(
                                &s.disc,
                                serde_json::json!({"event":"preview","on": true, "device": dev, "dir": dir, "size": "1280x720"}),
                            )))
                        }
                        Err(yolocam::Error::Busy) => {
                            let holder = yolocam::holders::summarize(&dev);
                            Ok(Some(tagged(
                                &s.disc,
                                serde_json::json!({"event":"preview","on": false, "error": "busy", "holder": holder.name, "holderKind": holder.kind}),
                            )))
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    s.preview = None;
                    s.last_covered = None;
                    Ok(Some(tagged(&s.disc, serde_json::json!({"event":"preview","on": false}))))
                }
            }
            "reconnect" => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.clear();
                    s.client = open_client(&s.disc);
                }
                Ok(None)
            }
            _ => Err(Error::Invalid(format!("unknown cmd {cmd}"))),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
struct Hits {
    stdin: bool,
    ctl: bool,
    plug: bool,
    s7: bool,
    s3: bool,
}

fn parse_model(s: &str) -> Option<Model> {
    match s.to_ascii_lowercase().as_str() {
        "s7" | "7" => Some(Model::S7),
        "s3" | "3" => Some(Model::S3),
        _ => None,
    }
}

fn tagged(disc: &Discovery, mut v: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(m) = &mut v {
        m.insert("model".into(), serde_json::json!(disc.model));
        m.insert("id".into(), serde_json::json!(disc.serial));
    }
    v
}

fn emit_cam(disc: &Discovery, v: serde_json::Value) {
    emit(tagged(disc, v));
}

fn discover_json(disc: &Discovery) -> serde_json::Value {
    serde_json::json!({
        "event": "discover",
        "ok": true,
        "usb": disc.usb,
        "video": disc.video,
        "cameraIp": disc.camera_ip,
        "iface": disc.iface,
        "serial": disc.serial,
        "product": disc.product
    })
}

fn inventory_json(inv: &Inventory, selected: Model, s7_ctrl: bool, s3_ctrl: bool) -> serde_json::Value {
    let mut cameras = Vec::new();
    for d in inv.plugged() {
        let control = match d.model {
            Model::S7 => s7_ctrl,
            Model::S3 => s3_ctrl,
        };
        cameras.push(serde_json::json!({
            "model": d.model,
            "id": d.serial,
            "usb": true,
            "control": control,
            "serial": d.serial,
            "product": d.product,
            "iface": d.iface,
            "cameraIp": d.camera_ip,
            "video": d.video
        }));
    }
    serde_json::json!({
        "event": "inventory",
        "glyph": inv.glyph_code(),
        "selected": selected,
        "cameras": cameras
    })
}

fn sync_slot(slot: &mut Option<Session>, next: Option<Discovery>) {
    match (slot.as_ref().map(|s| s.disc.clone()), next) {
        (None, None) => {}
        (_, Some(d)) if !d.usb => {
            *slot = None;
        }
        (None, Some(d)) => {
            *slot = Some(Session::new(d));
        }
        (Some(_), None) => {
            *slot = None;
        }
        (Some(cur), Some(d)) => {
            if cur.serial == d.serial && cur.iface == d.iface && cur.video == d.video {
                return;
            }
            if cur.serial != d.serial {
                *slot = Some(Session::new(d));
                return;
            }
            if let Some(s) = slot.as_mut() {
                if s.preview.is_some() && s.disc.video != d.video {
                    s.preview = None;
                    emit_cam(&d, serde_json::json!({"event":"preview","on": false}));
                }
                s.disc = d;
            }
        }
    }
}

fn disconnect_session(s: &mut Session, e: &Error) {
    s.client = None;
    s.rpc.clear();
    emit_cam(&s.disc, serde_json::json!({"event":"disconnected","error": e.to_string()}));
}

fn open_client(disc: &Discovery) -> Option<Client> {
    if !disc.usb {
        return None;
    }
    match connect_watch(disc) {
        Ok(mut c) => {
            let _ = c.set_nonblocking(true);
            let _ = c.queue_heartbeat();
            emit_cam(disc, serde_json::json!({"event":"connected"}));
            Some(c)
        }
        Err(e) => {
            emit_cam(disc, serde_json::json!({"event":"disconnected","error": e.to_string()}));
            None
        }
    }
}

fn dispatch(
    disc: &Discovery,
    focus: &mut FocusEmit,
    pending: &mut Option<Pending>,
    unsupported: &mut Vec<&'static str>,
    msgs: Vec<pb::Message>,
) {
    for mut msg in msgs {
        if let Some(p) = pending.as_ref() {
            if msg.is_reply_to(p.seq, p.cid) {
                let name = p.name;
                let cid = p.cid;
                let body = p.body;
                *pending = None;
                if msg.resp_code == 100 && matches!(body, RpcBody::Get) && !unsupported.contains(&name) {
                    unsupported.push(name);
                }
                let value = set_param_value(name, body.set_wire(), &msg);
                emit_cam(disc, serde_json::json!({"event":"param","name": name, "cid": cid, "resp": msg.resp_code, "value": value}));
                if let Some(r) = body.set_rect() {
                    if cid == params::FOCUS_AREA_CID || cid == params::FOCUS_TARGET_CID {
                        emit_cam(disc, serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}));
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
                emit_cam(disc, ev);
            }
            continue;
        }
        if msg.is_notify() && msg.id == params::FOCUS_AREA_CID {
            if let Some(ev) = focus.event("focus-area", &msg) {
                emit_cam(disc, ev);
            }
            continue;
        }
        if let Some(p) = params::find_cid(msg.id) {
            if !msg.is_notify() {
                emit_cam(disc, serde_json::json!({"event":"param","name":p.name,"cid":p.cid,"resp":msg.resp_code,"value": params::body_json(&msg)}));
            }
        } else if !msg.is_notify() {
            emit_cam(disc, serde_json::json!({"event":"raw","cid":msg.id,"type":msg.ty.map(|t| t as u32),"resp":msg.resp_code,"value": params::body_json(&msg)}));
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

fn drain_preview(s: &mut Session) {
    let Some(p) = s.preview.as_ref() else {
        return;
    };
    if let Ok(slot) = std::fs::read_to_string(p.dir.join("slot")) {
        if slot != s.last_slot {
            s.last_slot.clone_from(&slot);
            let file = if slot.trim() == "a" {
                p.dir.join("preview-a.jpg")
            } else {
                p.dir.join("preview-b.jpg")
            };
            let covered = std::fs::read_to_string(p.dir.join("covered")).is_ok_and(|s| s.trim() == "1");
            emit_cam(
                &s.disc,
                serde_json::json!({"event":"frame","slot": slot.trim(), "path": file, "covered": covered}),
            );
            if s.last_covered != Some(covered) {
                s.last_covered = Some(covered);
                emit_cam(&s.disc, serde_json::json!({"event":"covered","on": covered}));
            }
        }
    }
    if p.dir.join("error").exists() {
        if let Ok(e) = std::fs::read_to_string(p.dir.join("error")) {
            let err = e.trim().to_string();
            let holder = if err == "busy" {
                s.disc.video.as_deref().map(yolocam::holders::summarize)
            } else {
                None
            };
            emit_cam(
                &s.disc,
                serde_json::json!({
                    "event":"preview-error",
                    "error": err,
                    "holder": holder.as_ref().map(|h| &h.name),
                    "holderKind": holder.as_ref().map(|h| h.kind)
                }),
            );
            let _ = std::fs::remove_file(p.dir.join("error"));
            s.preview = None;
            s.last_covered = None;
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

    fn restart(&mut self) {
        self.at = 0;
        self.order = params::fill_order(self.group);
        self.urgent = true;
    }

    fn refresh(&mut self) {
        self.at = 0;
        self.order = params::refresh_order(self.group);
        self.urgent = true;
    }

    fn is_urgent(&self) -> bool {
        self.urgent
    }

    fn is_done(&self) -> bool {
        self.at >= self.order.len()
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
