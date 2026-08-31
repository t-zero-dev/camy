//! MJPEG viewfinder. Opens UVC only while running; EBUSY is a data error.
//!
//! Frames stay JPEG from V4L2 through to QML. Quickshell `Image` can only
//! load a URL, so the helper writes ping-pong files under `$XDG_RUNTIME_DIR`
//! (tmpfs — RAM, not a disk). Qt still decodes for display. We decode luma
//! on each displayed frame only to decide "lens covered" (~200 ms of dark).

use std::fs::{self, File};
use std::io::Write;
use std::num::NonZeroUsize;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nix::sys::mman::{mmap, munmap, MapFlags, ProtFlags};

use crate::error::{Error, Result};

const MJPEG: u32 = u32::from_le_bytes(*b"MJPG");
const V4L2_BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
const V4L2_MEMORY_MMAP: u32 = 1;
const VIDIOC_S_FMT: u64 = 0xC0D0_5605;
const VIDIOC_REQBUFS: u64 = 0xC014_5608;
const VIDIOC_QUERYBUF: u64 = 0xC058_5609;
const VIDIOC_QBUF: u64 = 0xC058_560F;
const VIDIOC_DQBUF: u64 = 0xC058_5611;
const VIDIOC_STREAMON: u64 = 0x4004_5612;
const VIDIOC_STREAMOFF: u64 = 0x4004_5613;

const COVER_STREAK: u8 = 3;

#[repr(C)]
struct V4l2PixFormat {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    flags: u32,
    ycbcr_enc: u32,
    quantization: u32,
    xfer_func: u32,
}

#[repr(C)]
struct V4l2Format {
    typ: u32,
    _pad: u32,
    pix: V4l2PixFormat,
    _tail: [u8; 200],
}

#[repr(C)]
struct V4l2RequestBuffers {
    count: u32,
    typ: u32,
    memory: u32,
    capabilities: u32,
    flags: u8,
    reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct V4l2Timecode {
    typ: u32,
    flags: u32,
    frames: u8,
    seconds: u8,
    minutes: u8,
    hours: u8,
    userbits: [u8; 4],
}

#[repr(C)]
struct V4l2Buffer {
    index: u32,
    typ: u32,
    bytesused: u32,
    flags: u32,
    field: u32,
    timestamp: libc::timeval,
    timecode: V4l2Timecode,
    sequence: u32,
    memory: u32,
    offset: u32,
    _pad: u32,
    length: u32,
    reserved2: u32,
    request_fd: i32,
}

fn ioctl<T>(fd: i32, req: u64, arg: &mut T) -> Result<()> {
    // SAFETY: `req` is a V4L2 ioctl; `arg` is the matching `#[repr(C)]`
    // struct and lives for the call. `fd` is an open V4L2 capture device.
    let rc = unsafe { libc::ioctl(fd, req as libc::c_ulong, std::ptr::from_mut(arg)) };
    if rc < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EBUSY) {
            return Err(Error::Busy);
        }
        return Err(err.into());
    }
    Ok(())
}

pub struct Preview {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub dir: PathBuf,
}

impl Preview {
    pub fn start(device: &str, width: u32, height: u32, fps: u32) -> Result<Self> {
        let dir = ensure_runtime_dir()?;
        wipe_preview_files(&dir);
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let device = device.to_string();
        let dir2 = dir.clone();
        let _ = fps;
        let thread = thread::Builder::new()
            .name("yolocam-preview".into())
            .spawn(move || {
                if let Err(e) = run(&device, width, height, &dir2, &stop2) {
                    let _ = std::fs::write(dir2.join("error"), e.to_string().as_bytes());
                }
            })
            .map_err(|e| Error::io_ctx(e, "preview thread"))?;
        Ok(Self {
            stop,
            thread: Some(thread),
            dir,
        })
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn runtime_dir() -> PathBuf {
    let xdg = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let tmp = std::env::var_os("TMPDIR").map(PathBuf::from);
    let chosen = choose_runtime_dir(xdg.as_deref(), tmp.as_deref(), current_uid());
    if !xdg.as_ref().is_some_and(|p| p.is_absolute() && chosen.starts_with(p)) {
        warn_runtime_fallback();
    }
    chosen
}

/// Create `$XDG_RUNTIME_DIR/yolocam` (or a 0700 fallback) and restrict it.
pub fn ensure_runtime_dir() -> Result<PathBuf> {
    let dir = runtime_dir();
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|e| Error::io_ctx(e, "mkdir"))?;
    restrict_mode(&dir, 0o700)?;
    Ok(dir)
}

fn wipe_preview_files(dir: &Path) {
    for name in [
        "preview-a.jpg",
        "preview-b.jpg",
        "preview-a.jpg.tmp",
        "preview-b.jpg.tmp",
        "slot",
        "covered",
        "error",
    ] {
        let _ = fs::remove_file(dir.join(name));
    }
}

pub fn restrict_mode(path: &Path, mode: u32) -> Result<()> {
    let meta = fs::metadata(path).map_err(|e| Error::io_ctx(e, format!("stat {}", path.display())))?;
    if meta.uid() != current_uid() {
        return Ok(());
    }
    let mut perms = meta.permissions();
    perms.set_mode(mode);
    fs::set_permissions(path, perms)
        .map_err(|e| Error::io_ctx(e, format!("chmod {}", path.display())))
}

pub(crate) fn choose_runtime_dir(xdg: Option<&Path>, tmpdir: Option<&Path>, uid: u32) -> PathBuf {
    if let Some(p) = xdg {
        if p.is_absolute() {
            return p.join("yolocam");
        }
    }
    let tmp = tmpdir
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| Path::new("/tmp"));
    tmp.join(format!("yolocam-{uid}"))
}

fn current_uid() -> u32 {
    // SAFETY: getuid is always defined and has no preconditions.
    unsafe { libc::getuid() }
}

fn warn_runtime_fallback() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        eprintln!("yolocam: XDG_RUNTIME_DIR missing or invalid; using a 0700 fallback");
    });
}

struct Map {
    ptr: NonNull<libc::c_void>,
    len: usize,
}

impl Drop for Map {
    fn drop(&mut self) {
        // SAFETY: `ptr`/`len` come from `mmap` in `run`. Ignore munmap
        // errors: panicking in Drop can abort (rust-lang/lang-team#97).
        unsafe {
            let _ = munmap(self.ptr, self.len);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run(
    device: &str,
    width: u32,
    height: u32,
    dir: &Path,
    stop: &AtomicBool,
) -> Result<()> {
    let file = File::options()
        .read(true)
        .write(true)
        .open(device)
        .map_err(|e| {
            if e.raw_os_error() == Some(libc::EBUSY) {
                Error::Busy
            } else {
                Error::io_ctx(e, format!("open {device}"))
            }
        })?;
    let fd = file.as_raw_fd();

    let mut fmt = V4l2Format {
        typ: V4L2_BUF_TYPE_VIDEO_CAPTURE,
        _pad: 0,
        pix: V4l2PixFormat {
            width,
            height,
            pixelformat: MJPEG,
            field: 1,
            bytesperline: 0,
            sizeimage: 0,
            colorspace: 0,
            priv_: 0,
            flags: 0,
            ycbcr_enc: 0,
            quantization: 0,
            xfer_func: 0,
        },
        _tail: [0; 200],
    };
    ioctl(fd, VIDIOC_S_FMT, &mut fmt)?;

    let mut req = V4l2RequestBuffers {
        count: 2,
        typ: V4L2_BUF_TYPE_VIDEO_CAPTURE,
        memory: V4L2_MEMORY_MMAP,
        capabilities: 0,
        flags: 0,
        reserved: [0; 3],
    };
    ioctl(fd, VIDIOC_REQBUFS, &mut req)?;
    if req.count < 1 {
        return Err(Error::io("no buffers"));
    }

    let mut maps = Vec::new();
    for i in 0..req.count {
        let mut buf = empty_buf(i);
        ioctl(fd, VIDIOC_QUERYBUF, &mut buf)?;
        let len = NonZeroUsize::new(buf.length as usize).ok_or_else(|| Error::io("mmap len"))?;
        // SAFETY: `fd` is a V4L2 capture fd; QUERYBUF returned a streaming
        // mmap offset/length. Valid until STREAMOFF + munmap.
        let ptr = unsafe {
            mmap(
                None,
                len,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &file,
                i64::from(buf.offset),
            )
            .map_err(|e| Error::Protocol(format!("mmap: {e}")))?
        };
        maps.push(Map {
            ptr,
            len: buf.length as usize,
        });
        ioctl(fd, VIDIOC_QBUF, &mut buf)?;
    }

    let mut typ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    ioctl(fd, VIDIOC_STREAMON, &mut typ)?;

    let mut slot = 0u8;
    let min_interval = Duration::from_millis(66); // ~15 fps display
    let mut last = Instant::now().checked_sub(min_interval).unwrap_or_else(Instant::now);
    let mut dark_streak: u8 = 0;
    let mut last_covered = false;
    let _ = std::fs::write(dir.join("covered"), b"0");
    while !stop.load(Ordering::Relaxed) {
        let mut buf = empty_buf(0);
        if ioctl(fd, VIDIOC_DQBUF, &mut buf).is_err() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let idx = buf.index as usize;
        let used = buf.bytesused as usize;
        if idx < maps.len() && used > 2 && last.elapsed() >= min_interval {
            let used = used.min(maps[idx].len);
            // SAFETY: DQBUF filled `bytesused` bytes of this mapping.
            let raw = unsafe { std::slice::from_raw_parts(maps[idx].ptr.as_ptr().cast::<u8>(), used) };
            let jpeg = trim_jpeg(raw);
            let name = if slot == 0 {
                "preview-a.jpg"
            } else {
                "preview-b.jpg"
            };
            let tmp = dir.join(format!("{name}.tmp"));
            let dest = dir.join(name);
            if let Ok(mut f) = File::create(&tmp) {
                if f.write_all(jpeg).is_ok() {
                    let _ = std::fs::rename(&tmp, &dest);
                    let dark = frame_is_covered(jpeg);
                    if dark {
                        dark_streak = dark_streak.saturating_add(1);
                    } else {
                        dark_streak = 0;
                    }
                    let covered = dark_streak >= COVER_STREAK;
                    if covered != last_covered {
                        last_covered = covered;
                        let _ = std::fs::write(
                            dir.join("covered"),
                            if covered { b"1" } else { b"0" },
                        );
                    }
                    let _ = std::fs::write(dir.join("slot"), if slot == 0 { b"a" } else { b"b" });
                    slot ^= 1;
                    last = Instant::now();
                }
            }
        }
        let _ = ioctl(fd, VIDIOC_QBUF, &mut buf);
    }

    let mut typ = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    let _ = ioctl(fd, VIDIOC_STREAMOFF, &mut typ);
    drop(maps);
    Ok(())
}

#[allow(clippy::many_single_char_names)]
fn frame_is_covered(jpeg: &[u8]) -> bool {
    let mut dec = zune_jpeg::JpegDecoder::new(jpeg);
    let Ok(pixels) = dec.decode() else {
        return false;
    };
    if pixels.is_empty() {
        return false;
    }
    let mut sum: u64 = 0;
    let mut max_l: u32 = 0;
    let mut n: u64 = 0;
    let step = 16.max(pixels.len() / 4000);
    let mut i = 0;
    while i + 2 < pixels.len() {
        let r = u32::from(pixels[i]);
        let g = u32::from(pixels[i + 1]);
        let b = u32::from(pixels[i + 2]);
        let y = (r * 77 + g * 150 + b * 29) >> 8;
        sum += u64::from(y);
        if y > max_l {
            max_l = y;
        }
        n += 1;
        i += step;
        i -= i % 3;
    }
    if n == 0 {
        return false;
    }
    let mean = sum / n;
    mean <= 18 && max_l <= 48
}

fn trim_jpeg(data: &[u8]) -> &[u8] {
    let start = data.windows(2).position(|w| w == [0xFF, 0xD8]).unwrap_or(0);
    let rest = &data[start..];
    if let Some(rel) = rest.windows(2).rposition(|w| w == [0xFF, 0xD9]) {
        &rest[..rel + 2]
    } else {
        rest
    }
}

fn empty_buf(index: u32) -> V4l2Buffer {
    V4l2Buffer {
        index,
        typ: V4L2_BUF_TYPE_VIDEO_CAPTURE,
        bytesused: 0,
        flags: 0,
        field: 0,
        timestamp: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        timecode: V4l2Timecode {
            typ: 0,
            flags: 0,
            frames: 0,
            seconds: 0,
            minutes: 0,
            hours: 0,
            userbits: [0; 4],
        },
        sequence: 0,
        memory: V4L2_MEMORY_MMAP,
        offset: 0,
        _pad: 0,
        length: 0,
        reserved2: 0,
        request_fd: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_runtime_dir, restrict_mode, trim_jpeg};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    #[test]
    fn prefers_absolute_xdg() {
        let p = choose_runtime_dir(Some(Path::new("/run/user/1000")), None, 1000);
        assert_eq!(p, PathBuf::from("/run/user/1000/yolocam"));
    }

    #[test]
    fn rejects_relative_xdg() {
        let p = choose_runtime_dir(Some(Path::new("relative")), Some(Path::new("/var/tmp")), 42);
        assert_eq!(p, PathBuf::from("/var/tmp/yolocam-42"));
    }

    #[test]
    fn fallback_without_xdg() {
        let p = choose_runtime_dir(None, None, 7);
        assert_eq!(p, PathBuf::from("/tmp/yolocam-7"));
    }

    #[test]
    fn restrict_mode_sets_0700() {
        let dir = std::env::temp_dir().join(format!("yolocam-mode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        restrict_mode(&dir, 0o700).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trim_keeps_soi_eoi() {
        let mut jpeg = vec![0x00, 0xFF, 0xD8, 0x01, 0x02, 0xFF, 0xD9, 0x00];
        assert_eq!(trim_jpeg(&jpeg), &jpeg[1..7]);
        jpeg.pop();
        assert_eq!(trim_jpeg(&jpeg), &jpeg[1..]);
    }
}
