//! One `poll` over camera TCP, ctl socket, stdin, and USB inotify.
//! Idle GET is a pending RPC, not a blocking walk.
//!
//! At most one S7 and one S3 session. Commands without `model`/`id` hit the
//! selected camera (S7 if both just appeared).

use std::io::{self, BufRead, Read};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MAX_CMD_LINE: usize = 8 * 1024;
const MAX_SLOT_FILE: usize = 64;
const GET_RPC_TIMEOUT: Duration = Duration::from_millis(400);
const SET_RPC_TIMEOUT: Duration = Duration::from_millis(2500);

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use camy::client::Client;
use camy::cmd::Cmd;
use camy::discover::{self, Discovery, Inventory, Model};
use camy::params::{self, Param, ParamId, SetPayload};
use camy::pb;
use camy::preview::{self, Preview};
use camy::usbwatch::PlugWatch;
use camy::{Error, Result};

use super::{connect_watch, ctl_path, emit};

#[derive(Clone, Copy, Debug)]
enum RpcBody {
    Get,
    Set(SetPayload),
}

impl RpcBody {
    fn timeout_is_error(self) -> bool {
        matches!(self, Self::Set(SetPayload::Int(_) | SetPayload::Wb { .. }))
    }

    fn rpc_timeout(self) -> Duration {
        if self.timeout_is_error() {
            SET_RPC_TIMEOUT
        } else {
            GET_RPC_TIMEOUT
        }
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
    body: SetPayload,
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
    unsupported: Vec<ParamId>,
    s3_zoom: u32,
}

impl Session {
    fn new(disc: Discovery) -> Self {
        let client = open_client(&disc);
        let unsupported = match disc.model {
            Model::S3 => params::S3_UNSUPPORTED.to_vec(),
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
            s3_zoom: 50,
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
        let _dir = camy::preview::ensure_runtime_dir()?;
        let _ = camy::preview::harden_runtime_tree();
        let sock_path = ctl_path();
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).map_err(|e| Error::io_ctx(e, "ctl.sock"))?;
        listener.set_nonblocking(true).ok();
        camy::preview::restrict_mode(&sock_path, 0o700)?;

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
            match read_cmd_line(input, line) {
                Ok(0) => return false,
                Ok(_) => {
                    let line = line.trim();
                    if !line.is_empty() {
                        self.on_stdin(line);
                    }
                }
                Err(_) => {
                    emit(serde_json::json!({"event":"error","error":"command too long"}));
                }
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
        if s.last_beat.elapsed() > Duration::from_secs(25)
            && c.pace_ready()
            && s.rpc.queued.is_none()
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
                    p.body
                        .rpc_timeout()
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
        Hits {
            stdin,
            ctl,
            plug,
            s7,
            s3,
        }
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
                dispatch(
                    &s.disc,
                    &mut s.focus,
                    &mut s.rpc.pending,
                    &mut s.unsupported,
                    msgs,
                );
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
            .is_some_and(|p| p.started.elapsed() > p.body.rpc_timeout());
        if timed_out {
            if let Some(p) = s.rpc.pending.take() {
                if p.name == "peek" {
                    write_peek(p.cid, 0, &serde_json::json!("timeout"));
                }
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
        if let Some(q) = s.rpc.queued.as_mut() {
            if q.name == "zoom" {
                if let Some(wire) = q.body.set_wire() {
                    if model == Model::S3 {
                        s.s3_zoom = wire;
                    }
                    q.body = params::zoom_set_payload(model == Model::S3, wire);
                }
            }
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
                    SetPayload::Int(wire) => c.begin_set(q.cid, wire),
                    SetPayload::Rect(rect) => c.begin_set_rect(q.cid, rect),
                    SetPayload::Wb { r, b } => c.begin_set_wb(q.cid, r, b),
                };
                match send {
                    Ok(seq) => {
                        if let Some(r) = q.body.set_rect() {
                            if q.name == "zoom" {
                                emit_cam(
                                    &s.disc,
                                    serde_json::json!({"event":"param","name":"zoom","cid":13,"resp":0,"value": params::zoom_raw_from_rect(r)}),
                                );
                            } else {
                                emit_cam(
                                    &s.disc,
                                    serde_json::json!({"event":"param","name": q.name, "cid": q.cid, "resp": 0, "value": {"x": r.x, "y": r.y, "w": r.w, "h": r.h}}),
                                );
                                emit_cam(
                                    &s.disc,
                                    serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}),
                                );
                            }
                        }
                        if let Some((wr, wb)) = q.body.set_wb() {
                            emit_cam(
                                &s.disc,
                                serde_json::json!({"event":"param","name": "wb-offset", "cid": q.cid, "resp": 0, "value": {"r": wr, "b": wb}}),
                            );
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
            Some(SendKind::Get(p)) if p.id == ParamId::Zoom && model == Model::S3 => {
                s.fill.commit();
                s.last_fill = Instant::now();
                emit_s3_zoom(&s.disc, s.s3_zoom);
                None
            }
            Some(SendKind::Get(p)) if p.id == ParamId::ImageVersion => {
                s.fill.commit();
                None
            }
            Some(SendKind::Get(_)) if hold_gets && !s.fill.is_urgent() => None,
            Some(SendKind::Get(p)) if s.unsupported.contains(&p.id) => {
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
            if !peer_is_self(&stream) {
                return;
            }
            stream.set_nonblocking(false).ok();
            let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
            let mut buf = String::new();
            let mut reader = io::BufReader::new(&stream);
            match read_cmd_line(&mut reader, &mut buf) {
                Ok(0) => {}
                Ok(_) => {
                    let line = buf.trim();
                    if !line.is_empty() {
                        self.on_stdin(line);
                    }
                }
                Err(_) => {
                    emit(serde_json::json!({"event":"error","error":"command too long"}));
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
            let names: Vec<&'static str> = s.unsupported.iter().map(|id| id.as_str()).collect();
            emit_cam(
                &s.disc,
                serde_json::json!({"event":"caps","unsupported": names}),
            );
            if s.disc.model == Model::S3 {
                emit_s3_zoom(&s.disc, s.s3_zoom);
            }
        }
    }

    fn target_model(&self, v: &serde_json::Value) -> Model {
        if let Some(m) = v
            .get("model")
            .and_then(|x| x.as_str())
            .and_then(parse_model)
        {
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
        if line.len() > MAX_CMD_LINE {
            return Err(Error::Invalid("command too long".into()));
        }
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| Error::Protocol(format!("json: {e}")))?;
        let cmd = Cmd::parse(&v)?;
        let model = self.target_model(&v);
        match cmd {
            Cmd::Select => {
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
                Ok(None)
            }
            Cmd::Tab { group } => {
                if let Some(s) = self.slot_mut(model) {
                    s.fill.set_tab(group);
                }
                Ok(None)
            }
            Cmd::GetAll => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.pending = None;
                    s.fill.restart();
                }
                Ok(None)
            }
            Cmd::Refresh => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.pending = None;
                    s.fill.refresh();
                    if s.fill.is_done() {
                        emit_cam(&s.disc, serde_json::json!({"event":"refreshed"}));
                    }
                }
                Ok(None)
            }
            Cmd::Set { param, payload } => {
                let s = self
                    .slot_mut(model)
                    .ok_or(Error::Disconnected("disconnected"))?;
                if s.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let p = param.spec();
                s.rpc.queued = Some(QueuedSet {
                    name: p.name,
                    cid: p.cid,
                    body: payload,
                });
                Ok(None)
            }
            Cmd::AfTarget | Cmd::AfArea => {
                let s = self
                    .slot_mut(model)
                    .ok_or(Error::Disconnected("disconnected"))?;
                if s.client.is_none() {
                    return Err(Error::Disconnected("disconnected"));
                }
                let rect = json_norm_rect(&v)?;
                s.rpc.queued = Some(QueuedSet {
                    name: if matches!(cmd, Cmd::AfTarget) {
                        "focus-target"
                    } else {
                        "focus-area"
                    },
                    cid: params::FOCUS_TARGET_CID,
                    body: SetPayload::Rect(rect),
                });
                Ok(None)
            }
            Cmd::Heartbeat => {
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
            Cmd::Preview { on } => {
                let s = self
                    .slot_mut(model)
                    .ok_or_else(|| Error::Invalid("no YoloCam UVC device".into()))?;
                if on {
                    if s.preview.is_some() {
                        return Ok(Some(tagged(
                            &s.disc,
                            serde_json::json!({"event":"preview","on": true}),
                        )));
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
                        Err(camy::Error::Busy) => {
                            let holder = camy::holders::summarize(&dev);
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
                    Ok(Some(tagged(
                        &s.disc,
                        serde_json::json!({"event":"preview","on": false}),
                    )))
                }
            }
            Cmd::Peek { cid } => {
                let s = self
                    .slot_mut(model)
                    .ok_or(Error::Disconnected("disconnected"))?;
                if s.rpc.pending.is_some() {
                    return Err(Error::Busy);
                }
                let c = s
                    .client
                    .as_mut()
                    .ok_or(Error::Disconnected("disconnected"))?;
                if !c.pace_ready() || c.write_pending() {
                    return Err(Error::Busy);
                }
                let seq = c.begin_get(cid)?;
                s.rpc.pending = Some(Pending {
                    seq,
                    name: "peek",
                    cid,
                    started: Instant::now(),
                    body: RpcBody::Get,
                });
                Ok(None)
            }
            Cmd::Reconnect => {
                if let Some(s) = self.slot_mut(model) {
                    s.rpc.clear();
                    s.client = open_client(&s.disc);
                }
                Ok(None)
            }
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

fn inventory_json(
    inv: &Inventory,
    selected: Model,
    s7_ctrl: bool,
    s3_ctrl: bool,
) -> serde_json::Value {
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
    emit_cam(
        &s.disc,
        serde_json::json!({"event":"disconnected","error": e.to_string()}),
    );
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
            emit_cam(
                disc,
                serde_json::json!({"event":"disconnected","error": e.to_string()}),
            );
            None
        }
    }
}

fn dispatch(
    disc: &Discovery,
    focus: &mut FocusEmit,
    pending: &mut Option<Pending>,
    unsupported: &mut Vec<ParamId>,
    msgs: Vec<pb::Message>,
) {
    for mut msg in msgs {
        if let Some(p) = pending.as_ref() {
            if msg.is_reply_to(p.seq, p.cid) {
                let name = p.name;
                let cid = p.cid;
                let body = p.body;
                *pending = None;
                if msg.resp_code == 100 && matches!(body, RpcBody::Get) {
                    if let Some(id) = ParamId::parse(name) {
                        if id != ParamId::ImageVersion && !unsupported.contains(&id) {
                            unsupported.push(id);
                        }
                    }
                }
                if name == "zoom" && disc.model == Model::S3 {
                    let raw = body
                        .set_rect()
                        .map(params::zoom_raw_from_rect)
                        .or_else(|| body.set_wire())
                        .unwrap_or(50);
                    emit_cam(
                        disc,
                        serde_json::json!({"event":"param","name":"zoom","cid":13,"resp":200,"value": raw}),
                    );
                    continue;
                }
                let value = set_param_value(name, body.set_wire(), &msg);
                if name == "peek" {
                    write_peek(cid, msg.resp_code, &value);
                }
                let resp = if name == "image-version" && msg.resp_code == 100 && matches!(body, RpcBody::Get)
                {
                    0
                } else {
                    msg.resp_code
                };
                emit_cam(
                    disc,
                    serde_json::json!({"event":"param","name": name, "cid": cid, "resp": resp, "value": value}),
                );
                if let Some(r) = body.set_rect() {
                    if cid == params::FOCUS_AREA_CID || cid == params::FOCUS_TARGET_CID {
                        emit_cam(
                            disc,
                            serde_json::json!({"event":"focus-area","rects":[{"x": r.x, "y": r.y, "w": r.w, "h": r.h}]}),
                        );
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
                emit_cam(
                    disc,
                    serde_json::json!({"event":"param","name":p.name,"cid":p.cid,"resp":msg.resp_code,"value": params::body_json(&msg)}),
                );
            }
        } else if !msg.is_notify() {
            emit_cam(
                disc,
                serde_json::json!({"event":"raw","cid":msg.id,"type":msg.ty.map(|t| t as u32),"resp":msg.resp_code,"value": params::body_json(&msg)}),
            );
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

fn read_small(path: &std::path::Path, max: usize) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; max];
    let n = f.read(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

fn peer_is_self(stream: &UnixStream) -> bool {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut cred).cast(),
            &mut len,
        )
    };
    rc == 0 && cred.uid == unsafe { libc::getuid() }
}

fn read_cmd_line(input: &mut impl BufRead, line: &mut String) -> io::Result<usize> {
    line.clear();
    let mut buf = Vec::new();
    let n = Read::take(&mut *input, MAX_CMD_LINE as u64 + 1).read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(0);
    }
    if buf.len() > MAX_CMD_LINE {
        if !buf.ends_with(&[b'\n']) {
            let mut rest = Vec::new();
            let _ = input.read_until(b'\n', &mut rest);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "command too long",
        ));
    }
    *line = String::from_utf8_lossy(&buf).into_owned();
    Ok(n)
}

fn drain_preview(s: &mut Session) {
    let Some(p) = s.preview.as_ref() else {
        return;
    };
    if let Some(slot) = read_small(&p.dir.join("slot"), MAX_SLOT_FILE) {
        if slot != s.last_slot {
            s.last_slot.clone_from(&slot);
            let file = if slot.trim() == "a" {
                p.dir.join("preview-a.jpg")
            } else {
                p.dir.join("preview-b.jpg")
            };
            let covered = read_small(&p.dir.join("covered"), MAX_SLOT_FILE)
                .is_some_and(|s| s.trim() == "1");
            emit_cam(
                &s.disc,
                serde_json::json!({"event":"frame","slot": slot.trim(), "path": file, "covered": covered}),
            );
            if s.last_covered != Some(covered) {
                s.last_covered = Some(covered);
                emit_cam(
                    &s.disc,
                    serde_json::json!({"event":"covered","on": covered}),
                );
            }
        }
    }
    if p.dir.join("error").exists() {
        if let Some(e) = read_small(&p.dir.join("error"), 512) {
            let err = e.trim().to_string();
            let holder = if err == "busy" {
                s.disc.video.as_deref().map(camy::holders::summarize)
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

fn write_peek(cid: u32, resp: u32, value: &serde_json::Value) {
    let path = preview::runtime_dir().join("peek.json");
    let body = serde_json::json!({"cid": cid, "resp": resp, "value": value});
    let _ = std::fs::write(path, body.to_string());
}

fn emit_s3_zoom(disc: &Discovery, raw: u32) {
    emit_cam(
        disc,
        serde_json::json!({"event":"param","name":"zoom","cid":13,"resp":200,"value": raw}),
    );
}

fn set_param_value(
    name: &'static str,
    set_wire: Option<u32>,
    msg: &pb::Message,
) -> serde_json::Value {
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
    use super::{Fill, RpcBody};
    use std::time::{Duration, Instant};
    use camy::params::{self, Kind, SetPayload};

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
            .all(|&i| !matches!(params::PARAMS[i].kind, Kind::GetOnly | Kind::WbOffset)));
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
            body: SetPayload::Int(200),
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
            body: SetPayload::Int(100),
        });
        rpc.queued = Some(super::QueuedSet {
            name: "ev",
            cid: 15,
            body: SetPayload::Int(8),
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
            body: RpcBody::Set(SetPayload::Int(200)),
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
            body: RpcBody::Set(SetPayload::Rect(camy::pb::RectF {
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
    fn cmd_line_rejects_oversize() {
        let mut cursor = std::io::Cursor::new(vec![b'x'; super::MAX_CMD_LINE + 8]);
        cursor.get_mut().push(b'\n');
        let mut line = String::new();
        let err = super::read_cmd_line(&mut cursor, &mut line).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn cmd_line_reads_short_json() {
        let mut cursor = std::io::Cursor::new(b"{\"cmd\":\"refresh\"}\nmore\n".to_vec());
        let mut line = String::new();
        let n = super::read_cmd_line(&mut cursor, &mut line).unwrap();
        assert!(n > 0);
        assert!(line.contains("refresh"));
    }

    #[test]
    fn rpc_body_flags_cannot_diverge() {
        assert!(RpcBody::Set(SetPayload::Int(1)).timeout_is_error());
        assert!(!RpcBody::Set(SetPayload::Rect(camy::pb::RectF::default())).timeout_is_error());
        assert!(!RpcBody::Get.timeout_is_error());
        assert_eq!(
            RpcBody::Set(SetPayload::Int(1)).rpc_timeout(),
            super::SET_RPC_TIMEOUT
        );
        assert_eq!(RpcBody::Get.rpc_timeout(), super::GET_RPC_TIMEOUT);
        assert!(super::SET_RPC_TIMEOUT > super::GET_RPC_TIMEOUT);
        assert_eq!(RpcBody::Set(SetPayload::Int(9)).set_wire(), Some(9));
        assert!(
            RpcBody::Set(SetPayload::Rect(camy::pb::RectF::default()))
                .set_wire()
                .is_none()
        );
        assert!(RpcBody::Get.set_rect().is_none());
    }
}
