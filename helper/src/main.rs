#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps
)]

mod watch;

use std::io::{self, Write};
use std::time::Duration;

use yolocam::client::{BlockingClient, Client};
use yolocam::discover::{self, Discovery};
use yolocam::params::{self, Kind, ParamId, SetPayload};
use yolocam::pb;
use yolocam::preview::Preview;
use yolocam::{Error, Result};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        args.push("status".into());
    }
    let cmd = args[0].as_str();
    let result = match cmd {
        "status" => cmd_status(),
        "probe" => cmd_probe(),
        "list" => cmd_list(),
        "get" => cmd_get(&args[1..]),
        "get-all" => cmd_get_all(),
        "dump-cids" => cmd_dump_cids(&args[1..]),
        "set" => cmd_set(&args[1..]),
        "iso" => cmd_set_alias("iso", &args[1..]),
        "shutter" => cmd_shutter(&args[1..]),
        "wb-temp" => cmd_set_alias("wb-temp", &args[1..]),
        "aperture" => cmd_set_alias("aperture", &args[1..]),
        "watch" => watch::run(),
        "ctl" => cmd_ctl(&args[1..]),
        "recover" => Err(Error::Invalid("recover is disabled".into())),
        "preview" => cmd_preview(&args[1..]),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        _ => Err(Error::Invalid(format!("unknown command {cmd}"))),
    };
    if let Err(e) = result {
        let _ = writeln!(
            io::stderr(),
            "{}",
            serde_json::json!({"ok": false, "error": e.to_string()})
        );
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "yolocam — YoloCam S7/S3 control\n\
         \n\
         Commands:\n\
           status              USB, gadget NIC, control port, firmware\n\
           probe               version handshake\n\
           list                named parameters\n\
           get <name...>       read parameters\n\
           get-all             read every named parameter\n\
           set <name> <value>  write a parameter\n\
           iso|shutter|wb-temp|aperture <value>\n\
           watch               JSON-lines events on stdout; commands on stdin\n\
           preview start|stop  MJPEG viewfinder (panel-open only)\n"
    );
}

fn emit(v: serde_json::Value) {
    let mut out = io::stdout();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn ok(v: serde_json::Value) {
    emit(json_ok(v));
}

fn json_ok(v: serde_json::Value) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("ok".into(), serde_json::Value::Bool(true));
    if let serde_json::Value::Object(obj) = v {
        for (k, val) in obj {
            m.insert(k, val);
        }
    } else {
        m.insert("data".into(), v);
    }
    serde_json::Value::Object(m)
}

fn connect(disc: &Discovery) -> Result<BlockingClient> {
    let addr = format!("{}:{}", disc.camera_ip, disc.port);
    BlockingClient::connect(&addr, Duration::from_secs(3))
}

fn connect_watch(disc: &Discovery) -> Result<Client> {
    let addr = format!("{}:{}", disc.camera_ip, disc.port);
    Client::connect(&addr, Duration::from_secs(3))
}

fn watch_owns_control() -> bool {
    ctl_path().exists()
}

fn cmd_status() -> Result<()> {
    let inv = discover::discover_all();
    let disc = inv.primary();
    let mut firmware = serde_json::Value::Null;
    let mut reachable = false;
    if watch_owns_control() {
        reachable = disc.usb;
    } else if disc.usb {
        if let Ok(mut c) = connect(&disc) {
            reachable = true;
            if let Ok(msg) = c.get(1) {
                firmware = params::body_json(&msg);
            }
        }
    }
    let cameras: Vec<_> = inv
        .plugged()
        .iter()
        .map(|d| {
            serde_json::json!({
                "model": d.model,
                "id": d.serial,
                "serial": d.serial,
                "usb": d.usb,
                "vidPid": d.vid_pid,
                "iface": d.iface,
                "hostIp": d.host_ip,
                "cameraIp": d.camera_ip,
                "video": d.video,
                "videoName": d.video_name
            })
        })
        .collect();
    ok(serde_json::json!({
        "usb": disc.usb,
        "model": disc.model,
        "vidPid": disc.vid_pid,
        "serial": disc.serial,
        "product": disc.product,
        "iface": disc.iface,
        "hostIp": disc.host_ip,
        "cameraIp": disc.camera_ip,
        "port": disc.port,
        "control": reachable,
        "video": disc.video,
        "videoName": disc.video_name,
        "firmware": firmware,
        "glyph": inv.glyph_code(),
        "cameras": cameras
    }));
    Ok(())
}

fn cmd_probe() -> Result<()> {
    if watch_owns_control() {
        return Err(Error::Invalid("watch owns TCP 12345; use the panel".into()));
    }
    let disc = discover::discover();
    let mut c = connect(&disc)?;
    let msg = c.get(1)?;
    ok(serde_json::json!({
        "resp": msg.resp_code,
        "version": params::body_json(&msg)
    }));
    Ok(())
}

fn cmd_list() -> Result<()> {
    let list: Vec<_> = params::PARAMS
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "cid": p.cid,
                "label": p.label,
                "group": p.group,
                "set": p.kind.settable(),
                "kind": match p.kind {
                    Kind::Int { min, max } => serde_json::json!({"type":"int","min":min,"max":max}),
                    Kind::Enum(opts) => serde_json::json!({"type":"enum","options": opts.iter().map(|(l,v)| serde_json::json!({"label":l,"value":v})).collect::<Vec<_>>()}),
                    Kind::GetOnly | Kind::WbOffset => serde_json::json!({"type":"info"}),
                }
            })
        })
        .collect();
    ok(serde_json::json!({"params": list}));
    Ok(())
}

fn param_object(name: &str, msg: &pb::Message) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "cid": msg.id,
        "resp": msg.resp_code,
        "value": params::body_json(msg)
    })
}

fn cmd_get(args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(Error::Invalid("get <name...>".into()));
    }
    if watch_owns_control() {
        return Err(Error::Invalid("watch owns TCP 12345; use the panel".into()));
    }
    let disc = discover::discover();
    let mut c = connect(&disc)?;
    let mut items = Vec::new();
    for name in args {
        let p =
            params::find(name).ok_or_else(|| Error::Invalid(format!("unknown param {name}")))?;
        let msg = c.get(p.cid)?;
        items.push(param_object(p.name, &msg));
    }
    ok(serde_json::json!({"items": items}));
    Ok(())
}

fn cmd_dump_cids(args: &[String]) -> Result<()> {
    if watch_owns_control() {
        return Err(Error::Invalid("watch owns TCP 12345; use the panel".into()));
    }
    let inv = discover::discover_all();
    let mut rest = args;
    let disc = match rest.first().map(String::as_str) {
        Some("s3") => {
            rest = &rest[1..];
            inv.s3
                .clone()
                .filter(|d| d.usb)
                .ok_or_else(|| Error::Invalid("no S3".into()))?
        }
        Some("s7") => {
            rest = &rest[1..];
            inv.s7
                .clone()
                .filter(|d| d.usb)
                .ok_or_else(|| Error::Invalid("no S7".into()))?
        }
        _ => inv.primary(),
    };
    let parsed: Vec<u32> = rest.iter().filter_map(|s| s.parse().ok()).collect();
    let default: &[u32] = &[25, 29, 102, 105, 111];
    let cids: Vec<u32> = if parsed.is_empty() {
        default.to_vec()
    } else {
        parsed
    };
    let mut c = connect(&disc)?;
    let _ = c.heartbeat();
    let mut items = Vec::new();
    for cid in &cids {
        match c.get_within(*cid, Duration::from_millis(280)) {
            Ok(msg) => items.push(serde_json::json!({
                "cid": cid,
                "resp": msg.resp_code,
                "value": params::body_json(&msg)
            })),
            Err(e) => items.push(serde_json::json!({"cid": cid, "error": e.to_string()})),
        }
    }
    ok(serde_json::json!({"model": disc.model, "serial": disc.serial, "items": items}));
    Ok(())
}

fn cmd_get_all() -> Result<()> {
    if watch_owns_control() {
        return Err(Error::Invalid("watch owns TCP 12345; use the panel".into()));
    }
    let disc = discover::discover();
    let mut c = connect(&disc)?;
    let _ = c.heartbeat();
    let mut items = Vec::new();
    for p in params::PARAMS {
        match c.get_within(p.cid, Duration::from_millis(280)) {
            Ok(msg) => items.push(param_object(p.name, &msg)),
            Err(e) => items
                .push(serde_json::json!({"name": p.name, "cid": p.cid, "error": e.to_string()})),
        }
    }
    ok(serde_json::json!({"items": items}));
    Ok(())
}

fn cmd_set(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return Err(Error::Invalid("set <name> <value>".into()));
    }
    apply_set(&args[0], &args[1])
}

fn cmd_set_alias(name: &str, args: &[String]) -> Result<()> {
    let v = args
        .first()
        .ok_or_else(|| Error::Invalid(format!("{name} <value>")))?;
    apply_set(name, v)
}

fn cmd_shutter(args: &[String]) -> Result<()> {
    let v = args
        .first()
        .ok_or_else(|| Error::Invalid("shutter <1/N or microseconds>".into()))?;
    let us = if let Some(rest) = v.strip_prefix("1/") {
        params::shutter_from_denom(
            rest.parse()
                .map_err(|_| Error::Invalid("bad denom".into()))?,
        )
        .ok_or_else(|| Error::Invalid(format!("unknown shutter {v}")))?
    } else {
        v.parse()
            .map_err(|_| Error::Invalid("bad shutter".into()))?
    };
    apply_set("shutter-us", &us.to_string())
}

fn apply_set(name: &str, raw: &str) -> Result<()> {
    let p = ParamId::parse(name)
        .map(ParamId::spec)
        .ok_or_else(|| Error::Invalid(format!("unknown param {name}")))?;
    let payload = params::parse_set(p, &serde_json::json!({"name": name, "value": raw}))?;
    let SetPayload::Int(wire) = payload else {
        return Err(Error::Invalid(format!("{name} needs a structured value")));
    };
    if watch_owns_control() {
        return cmd_ctl(&["set".into(), name.into(), raw.into()]);
    }
    let disc = discover::discover();
    let mut c = connect(&disc)?;
    let mut msg = c.set(p.cid, wire)?;
    if matches!(msg.body, yolocam::pb::Body::Empty) {
        msg.body = yolocam::pb::Body::Int(wire);
    }
    ok(param_object(p.name, &msg));
    Ok(())
}

fn cmd_preview(args: &[String]) -> Result<()> {
    let sub = args.first().map_or("start", String::as_str);
    match sub {
        "stop" => {
            ok(serde_json::json!({"preview": "stopped"}));
            Ok(())
        }
        "start" => {
            let disc = discover::discover();
            let dev = disc
                .video
                .ok_or_else(|| Error::Invalid("no YoloCam UVC device".into()))?;
            let prev = Preview::start(&dev, 1280, 720, 15)?;
            ok(serde_json::json!({
                "preview": "running",
                "device": dev,
                "dir": prev.dir,
                "size": "1280x720"
            }));
            // Block until stdin closes. The panel uses `watch` for preview.
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            ctrlc_stub(tx);
            let _ = rx.recv();
            prev.stop();
            Ok(())
        }
        _ => Err(Error::Invalid("preview start|stop".into())),
    }
}

fn ctrlc_stub(tx: std::sync::mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let mut dummy = String::new();
        let _ = io::stdin().read_line(&mut dummy);
        let _ = tx.send(());
    });
}

fn ctl_path() -> std::path::PathBuf {
    yolocam::preview::runtime_dir().join("ctl.sock")
}

fn cmd_ctl(args: &[String]) -> Result<()> {
    let payload = if args.len() == 1 {
        args[0].clone()
    } else if args[0] == "preview" && args.len() >= 2 {
        let on = matches!(args[1].as_str(), "on" | "true" | "1" | "start");
        serde_json::json!({"cmd": "preview", "on": on}).to_string()
    } else if args.len() >= 2 {
        serde_json::json!({"cmd": args[0], "name": args.get(1), "value": args.get(2)}).to_string()
    } else {
        return Err(Error::Invalid(
            "ctl '<json>' | ctl set <name> <value> | ctl preview on|off".into(),
        ));
    };
    if payload.len() > 8 * 1024 {
        return Err(Error::Invalid("command too long".into()));
    }
    let sock = std::os::unix::net::UnixStream::connect(ctl_path())
        .map_err(|e| Error::io_ctx(e, "watch not running"))?;
    let mut s = sock;
    s.write_all(payload.as_bytes())
        .map_err(|e| Error::io_ctx(e, "ctl write"))?;
    s.write_all(b"\n").ok();
    ok(serde_json::json!({"sent": true}));
    Ok(())
}
