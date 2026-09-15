//! Plug/unplug without polling sysfs: inotify on `/dev` and USB/net class dirs.

use std::ffi::OsString;
use std::os::fd::{AsFd, BorrowedFd};

use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};

pub struct PlugWatch {
    inf: Inotify,
}

impl PlugWatch {
    pub fn new() -> Option<Self> {
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
        Some(Self { inf })
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.inf.as_fd()
    }

    /// True when a device node or USB/net/v4l sysfs entry appeared or vanished.
    pub fn changed(&self) -> bool {
        match self.inf.read_events() {
            Ok(events) => events.iter().any(|e| is_interesting(e.name.as_ref())),
            Err(_) => false,
        }
    }
}

/// `video*`, `en*`/`eth*`, or a hyphen (USB sysfs ids such as `1-2.4`).
fn is_interesting(name: Option<&OsString>) -> bool {
    let Some(name) = name else {
        return true;
    };
    let n = name.to_string_lossy();
    n.starts_with("video") || n.starts_with("en") || n.starts_with("eth") || n.contains('-')
}
