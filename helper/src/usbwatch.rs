//! Plug/unplug without polling sysfs: kernel uevents, with inotify fallback.
//!
//! inotify on sysfs often misses nested-hub plugs, and `/dev` inotify never
//! sees a CDC-only camera whose UVC probe failed. `NETLINK_KOBJECT_UEVENT`
//! is the same channel udev uses. Poll stays in `watch.rs` only when this
//! watch cannot be created.

use std::ffi::OsString;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use nix::errno::Errno;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::socket::{
    bind, recv, setsockopt, socket, sockopt, AddressFamily, MsgFlags, NetlinkAddr, SockFlag,
    SockProtocol, SockType,
};

/// Kernel multicast group for `kobject_uevent` (udev `UEVENT_NUM_ENVP` group 1).
const UEVENT_GROUP_KERNEL: u32 = 1;
const UEVENT_RCVBUF: usize = 2 * 1024 * 1024;
const UEVENT_MSG: usize = 8192;

enum Source {
    Uevent(OwnedFd),
    Inotify(Inotify),
}

pub struct PlugWatch {
    src: Source,
}

impl PlugWatch {
    pub fn new() -> Option<Self> {
        if let Some(fd) = open_uevent() {
            return Some(Self {
                src: Source::Uevent(fd),
            });
        }
        open_inotify().map(|inf| Self {
            src: Source::Inotify(inf),
        })
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        match &self.src {
            Source::Uevent(fd) => fd.as_fd(),
            Source::Inotify(inf) => inf.as_fd(),
        }
    }

    /// True when a USB, net, or V4L device appeared, vanished, or rebound.
    pub fn changed(&self) -> bool {
        match &self.src {
            Source::Uevent(fd) => drain_uevent(fd),
            Source::Inotify(inf) => match inf.read_events() {
                Ok(events) => events.iter().any(|e| is_interesting(e.name.as_ref())),
                Err(_) => false,
            },
        }
    }
}

fn open_uevent() -> Option<OwnedFd> {
    let fd = socket(
        AddressFamily::Netlink,
        SockType::Raw,
        SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
        SockProtocol::NetlinkKObjectUEvent,
    )
    .ok()?;
    let addr = NetlinkAddr::new(0, UEVENT_GROUP_KERNEL);
    bind(fd.as_raw_fd(), &addr).ok()?;
    let _ = setsockopt(&fd, sockopt::RcvBuf, &UEVENT_RCVBUF);
    Some(fd)
}

fn open_inotify() -> Option<Inotify> {
    let inf = Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK).ok()?;
    let mask = AddWatchFlags::IN_CREATE
        | AddWatchFlags::IN_DELETE
        | AddWatchFlags::IN_MOVED_FROM
        | AddWatchFlags::IN_MOVED_TO;
    inf.add_watch("/dev", mask).ok()?;
    for extra in [
        "/sys/bus/usb/devices",
        "/sys/class/net",
        "/sys/class/video4linux",
    ] {
        let _ = inf.add_watch(extra, mask);
    }
    Some(inf)
}

fn drain_uevent(fd: &OwnedFd) -> bool {
    let mut buf = [0u8; UEVENT_MSG];
    let mut hit = false;
    loop {
        match recv(fd.as_raw_fd(), &mut buf, MsgFlags::empty()) {
            Ok(0) | Err(Errno::EAGAIN) => break,
            Ok(n) => {
                if uevent_interesting(&buf[..n]) {
                    hit = true;
                }
            }
            Err(Errno::EINTR) => {}
            Err(Errno::ENOBUFS | Errno::ENOMEM) => {
                hit = true;
                break;
            }
            Err(_) => break,
        }
    }
    hit
}

fn uevent_interesting(buf: &[u8]) -> bool {
    let mut action = None;
    let mut subsystem = None;
    for field in buf.split(|b| *b == 0) {
        if field.is_empty() {
            continue;
        }
        let Ok(s) = std::str::from_utf8(field) else {
            continue;
        };
        if let Some(v) = s.strip_prefix("ACTION=") {
            action = Some(v);
        } else if let Some(v) = s.strip_prefix("SUBSYSTEM=") {
            subsystem = Some(v);
        }
    }
    match (action, subsystem) {
        (Some(a), Some(s)) => action_interesting(a) && subsystem_interesting(s),
        _ => true,
    }
}

fn action_interesting(action: &str) -> bool {
    matches!(
        action,
        "add" | "remove" | "bind" | "unbind" | "change" | "move"
    )
}

fn subsystem_interesting(subsystem: &str) -> bool {
    matches!(subsystem, "usb" | "net" | "video4linux" | "usbmisc")
}

/// `video*`, `en*`/`eth*`, or a hyphen (USB sysfs ids such as `1-2.4`).
fn is_interesting(name: Option<&OsString>) -> bool {
    let Some(name) = name else {
        return true;
    };
    let n = name.to_string_lossy();
    n.starts_with("video") || n.starts_with("en") || n.starts_with("eth") || n.contains('-')
}

#[cfg(test)]
mod tests {
    use super::{is_interesting, uevent_interesting};
    use std::ffi::OsString;

    fn payload(fields: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                out.push(0);
            }
            out.extend_from_slice(f.as_bytes());
        }
        out.push(0);
        out
    }

    #[test]
    fn overflow_and_usb_net_video_names_are_interesting() {
        assert!(is_interesting(None));
        assert!(is_interesting(Some(&OsString::from("video34"))));
        assert!(is_interesting(Some(&OsString::from("enp0s13f0u1u1"))));
        assert!(is_interesting(Some(&OsString::from("2-1.2.4"))));
        assert!(!is_interesting(Some(&OsString::from("null"))));
    }

    #[test]
    fn usb_net_v4l_uevents_are_interesting() {
        assert!(uevent_interesting(&payload(&[
            "add@/devices/pci0000:00/usb2/2-1.2.4",
            "ACTION=add",
            "DEVPATH=/devices/pci0000:00/usb2/2-1.2.4",
            "SUBSYSTEM=usb",
            "DEVTYPE=usb_device",
        ])));
        assert!(uevent_interesting(&payload(&[
            "ACTION=add",
            "SUBSYSTEM=net",
            "INTERFACE=enp0s13f0u1u2u4",
        ])));
        assert!(uevent_interesting(&payload(&[
            "ACTION=bind",
            "SUBSYSTEM=video4linux",
        ])));
        assert!(uevent_interesting(&payload(&[
            "ACTION=remove",
            "SUBSYSTEM=usbmisc",
        ])));
    }

    #[test]
    fn other_subsystems_are_ignored_and_unknown_payloads_rescan() {
        assert!(!uevent_interesting(&payload(&[
            "ACTION=add",
            "SUBSYSTEM=input",
        ])));
        assert!(!uevent_interesting(&payload(&[
            "ACTION=offline",
            "SUBSYSTEM=usb",
        ])));
        assert!(uevent_interesting(b"truncated-no-keys"));
    }
}
