use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const USB_VID: &str = "46f8";
pub const USB_PID_S7: &str = "0851";
pub const USB_PID_S3: &str = "0855";
pub const USB_PID: &str = USB_PID_S7;
pub const DEFAULT_IP_S7: &str = "192.168.129.10";
pub const DEFAULT_IP_S3: &str = "192.168.126.10";
pub const DEFAULT_IP: &str = DEFAULT_IP_S7;
pub const DEFAULT_PORT: u16 = 12345;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    S7,
    S3,
}

impl Model {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::S7 => "s7",
            Self::S3 => "s3",
        }
    }

    pub fn pid(self) -> &'static str {
        match self {
            Self::S7 => USB_PID_S7,
            Self::S3 => USB_PID_S3,
        }
    }

    pub fn default_ip(self) -> &'static str {
        match self {
            Self::S7 => DEFAULT_IP_S7,
            Self::S3 => DEFAULT_IP_S3,
        }
    }

    pub fn from_pid(pid: &str) -> Option<Self> {
        if pid.eq_ignore_ascii_case(USB_PID_S7) {
            Some(Self::S7)
        } else if pid.eq_ignore_ascii_case(USB_PID_S3) {
            Some(Self::S3)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub usb: bool,
    pub model: Model,
    pub vid_pid: String,
    pub serial: String,
    pub product: String,
    pub iface: Option<String>,
    pub host_ip: Option<String>,
    pub camera_ip: String,
    pub port: u16,
    pub video: Option<String>,
    pub video_name: Option<String>,
}

impl Discovery {
    pub fn empty(model: Model) -> Self {
        Self {
            usb: false,
            model,
            vid_pid: format!("{}:{}", USB_VID, model.pid()),
            serial: String::new(),
            product: String::new(),
            iface: None,
            host_ip: None,
            camera_ip: model.default_ip().to_string(),
            port: DEFAULT_PORT,
            video: None,
            video_name: None,
        }
    }
}

impl Default for Discovery {
    fn default() -> Self {
        Self::empty(Model::S7)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub s7: Option<Discovery>,
    pub s3: Option<Discovery>,
}

impl Inventory {
    pub fn glyph_code(&self) -> u8 {
        glyph_code(
            self.s3.as_ref().is_some_and(|d| d.usb),
            self.s7.as_ref().is_some_and(|d| d.usb),
        )
    }

    pub fn plugged(&self) -> Vec<&Discovery> {
        let mut out = Vec::new();
        if let Some(d) = self.s7.as_ref().filter(|d| d.usb) {
            out.push(d);
        }
        if let Some(d) = self.s3.as_ref().filter(|d| d.usb) {
            out.push(d);
        }
        out
    }

    /// S7 if plugged, else S3, else an unplugged S7.
    pub fn primary(&self) -> Discovery {
        if let Some(d) = self.s7.as_ref().filter(|d| d.usb) {
            return d.clone();
        }
        if let Some(d) = self.s3.as_ref().filter(|d| d.usb) {
            return d.clone();
        }
        Discovery::default()
    }

    pub fn get(&self, model: Model) -> Option<&Discovery> {
        match model {
            Model::S7 => self.s7.as_ref(),
            Model::S3 => self.s3.as_ref(),
        }
    }

    pub fn same_plug(&self, other: &Self) -> bool {
        slot_same(self.s7.as_ref(), other.s7.as_ref())
            && slot_same(self.s3.as_ref(), other.s3.as_ref())
    }
}

fn slot_same(a: Option<&Discovery>, b: Option<&Discovery>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.usb == y.usb && x.serial == y.serial && x.iface == y.iface && x.video == y.video
        }
        _ => false,
    }
}

pub fn glyph_code(s3: bool, s7: bool) -> u8 {
    u8::from(s3) * 3 + u8::from(s7) * 7
}

pub fn discover() -> Discovery {
    discover_all().primary()
}

pub fn discover_all() -> Inventory {
    discover_from(
        Path::new("/sys/bus/usb/devices"),
        Path::new("/sys/class/video4linux"),
    )
}

pub fn discover_from(usb_root: &Path, v4l_root: &Path) -> Inventory {
    let mut s7: Option<Discovery> = None;
    let mut s3: Option<Discovery> = None;
    let Ok(entries) = fs::read_dir(usb_root) else {
        return Inventory { s7, s3 };
    };
    let videos = list_videos(v4l_root);
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        let Some(vid) = read_trim(&p.join("idVendor")) else {
            continue;
        };
        let Some(pid) = read_trim(&p.join("idProduct")) else {
            continue;
        };
        if !vid.eq_ignore_ascii_case(USB_VID) {
            continue;
        }
        let Some(model) = Model::from_pid(&pid) else {
            continue;
        };
        let slot = match model {
            Model::S7 => &mut s7,
            Model::S3 => &mut s3,
        };
        if slot.is_some() {
            continue;
        }
        *slot = Some(fill_camera(model, &p, &videos));
    }
    Inventory { s7, s3 }
}

struct VideoNode {
    dev: String,
    name: String,
    usb: Option<PathBuf>,
}

fn fill_camera(model: Model, usb: &Path, videos: &[VideoNode]) -> Discovery {
    let mut d = Discovery::empty(model);
    d.usb = true;
    d.serial = read_trim(&usb.join("serial")).unwrap_or_default();
    d.product = read_trim(&usb.join("product")).unwrap_or_else(|| "YoloCam".into());
    if let Some(iface) = find_net_iface(usb) {
        d.iface = Some(iface.clone());
        if let Some(ip) = iface_ipv4(&iface) {
            d.host_ip = Some(ip.clone());
            if let Some(cam) = sibling_ten(&ip) {
                d.camera_ip = cam;
            }
        }
    }
    if let Some((dev, name)) = pair_video(usb, videos, &d.serial) {
        d.video = Some(dev);
        d.video_name = Some(name);
    } else if let Some((dev, name)) = fallback_named_video(videos) {
        d.video = Some(dev);
        d.video_name = Some(name);
    }
    d
}

fn list_videos(v4l_root: &Path) -> Vec<VideoNode> {
    let Ok(rd) = fs::read_dir(v4l_root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if !name.starts_with("video") {
            continue;
        }
        let sys = ent.path();
        let label = read_trim(&sys.join("name")).unwrap_or_default();
        let usb = usb_root_from(&sys.join("device"));
        found.push(VideoNode {
            dev: format!("/dev/{name}"),
            name: label,
            usb,
        });
    }
    found.sort_by(|a, b| a.dev.cmp(&b.dev));
    found
}

fn pair_video(usb: &Path, videos: &[VideoNode], serial: &str) -> Option<(String, String)> {
    let want = fs::canonicalize(usb).unwrap_or_else(|_| usb.to_path_buf());
    for v in videos {
        let Some(root) = v.usb.as_ref() else {
            continue;
        };
        if *root == want {
            return Some((v.dev.clone(), v.name.clone()));
        }
    }
    if !serial.is_empty() {
        for v in videos {
            if v.name.contains(serial) {
                return Some((v.dev.clone(), v.name.clone()));
            }
        }
    }
    None
}

fn fallback_named_video(videos: &[VideoNode]) -> Option<(String, String)> {
    let named: Vec<_> = videos
        .iter()
        .filter(|v| {
            let l = v.name.to_ascii_lowercase();
            l.contains("yololiv")
                || l.contains("yolocam")
                || l.contains("y-cam")
                || l.contains("yx-cam")
                || l.starts_with("s3:")
                || l.starts_with("s3 ")
        })
        .collect();
    if named.len() == 1 {
        return Some((named[0].dev.clone(), named[0].name.clone()));
    }
    None
}

pub fn usb_root_from(start: &Path) -> Option<PathBuf> {
    let mut p = if start.is_symlink() {
        fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf())
    } else {
        start.to_path_buf()
    };
    for _ in 0..16 {
        if p.join("idVendor").is_file() && p.join("idProduct").is_file() {
            return fs::canonicalize(&p).ok().or(Some(p));
        }
        if !p.pop() {
            return None;
        }
    }
    None
}

fn read_trim(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut buf = [0u8; 256];
    let n = f.read(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).trim().to_string())
}

fn find_net_iface(usb: &Path) -> Option<String> {
    fn walk(dir: &Path, depth: u8) -> Option<String> {
        if depth == 0 {
            return None;
        }
        let net = dir.join("net");
        if net.is_dir() {
            if let Ok(mut it) = fs::read_dir(&net) {
                if let Some(Ok(ent)) = it.next() {
                    return Some(ent.file_name().to_string_lossy().into_owned());
                }
            }
        }
        if let Ok(rd) = fs::read_dir(dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_dir() {
                    if let Some(n) = walk(&p, depth - 1) {
                        return Some(n);
                    }
                }
            }
        }
        None
    }
    walk(usb, 6)
}

fn iface_ipv4(iface: &str) -> Option<String> {
    let addrs = nix::ifaddrs::getifaddrs().ok()?;
    for ifaddr in addrs {
        if ifaddr.interface_name != iface {
            continue;
        }
        let Some(addr) = ifaddr.address else {
            continue;
        };
        let Some(sin) = addr.as_sockaddr_in() else {
            continue;
        };
        let ip = sin.ip();
        if !ip.is_unspecified() && !ip.is_loopback() {
            return Some(ip.to_string());
        }
    }
    None
}

pub fn sibling_ten(host: &str) -> Option<String> {
    let mut oct: Vec<u8> = host.split('.').filter_map(|s| s.parse().ok()).collect();
    if oct.len() != 4 {
        return None;
    }
    oct[3] = 10;
    Some(format!("{}.{}.{}.{}", oct[0], oct[1], oct[2], oct[3]))
}

pub const MAX_ID_LEN: usize = 64;

pub fn sanitize_id(serial: &str) -> String {
    let mut s: String = serial
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(MAX_ID_LEN)
        .collect();
    if s.is_empty() {
        "camera".into()
    } else {
        s.truncate(MAX_ID_LEN);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn write(path: &Path, s: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, s).unwrap();
    }

    #[test]
    fn glyph_codes() {
        assert_eq!(glyph_code(false, false), 0);
        assert_eq!(glyph_code(true, false), 3);
        assert_eq!(glyph_code(false, true), 7);
        assert_eq!(glyph_code(true, true), 10);
    }

    #[test]
    fn pid_maps() {
        assert_eq!(Model::from_pid("0851"), Some(Model::S7));
        assert_eq!(Model::from_pid("0855"), Some(Model::S3));
        assert_eq!(Model::from_pid("0000"), None);
    }

    #[test]
    fn sibling_ten_rewrites_last_octet() {
        assert_eq!(
            sibling_ten("192.168.129.11").as_deref(),
            Some("192.168.129.10")
        );
        assert_eq!(
            sibling_ten("192.168.126.11").as_deref(),
            Some("192.168.126.10")
        );
        assert_eq!(sibling_ten("bad"), None);
    }

    #[test]
    fn sanitize_id_strips_junk() {
        assert_eq!(sanitize_id("Y-CAM-25360005"), "Y-CAM-25360005");
        assert_eq!(sanitize_id("a/b:c"), "a-b-c");
        assert_eq!(sanitize_id(".."), "--");
        assert_eq!(sanitize_id(""), "camera");
        assert_eq!(sanitize_id(&"A".repeat(80)).len(), MAX_ID_LEN);
    }

    #[test]
    fn inventory_picks_first_of_each_and_ignores_second_s7() {
        let root = std::env::temp_dir().join(format!("yolocam-inv-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let usb = root.join("usb");
        let v4l = root.join("v4l");
        fs::create_dir_all(&v4l).unwrap();
        write(&usb.join("1-1/idVendor"), "46f8");
        write(&usb.join("1-1/idProduct"), "0851");
        write(&usb.join("1-1/serial"), "S7-FIRST");
        write(&usb.join("1-1/product"), "YoloLiv YoloCam");
        write(&usb.join("1-2/idVendor"), "46f8");
        write(&usb.join("1-2/idProduct"), "0851");
        write(&usb.join("1-2/serial"), "S7-SECOND");
        write(&usb.join("2-1/idVendor"), "46f8");
        write(&usb.join("2-1/idProduct"), "0855");
        write(&usb.join("2-1/serial"), "S3-ONLY");
        write(&usb.join("2-1/product"), "YoloLiv YoloCam");
        write(&usb.join("hub/idVendor"), "1d6b");
        write(&usb.join("hub/idProduct"), "0003");

        let inv = discover_from(&usb, &v4l);
        assert_eq!(inv.glyph_code(), 10);
        assert_eq!(inv.s7.as_ref().map(|d| d.serial.as_str()), Some("S7-FIRST"));
        assert_eq!(inv.s3.as_ref().map(|d| d.serial.as_str()), Some("S3-ONLY"));
        assert_eq!(inv.primary().serial, "S7-FIRST");
        assert_eq!(inv.plugged().len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn pairs_video_by_usb_parent_not_name_order() {
        let root = std::env::temp_dir().join(format!("yolocam-pair-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let usb = root.join("usb");
        let v4l = root.join("v4l");
        write(&usb.join("s7/idVendor"), "46f8");
        write(&usb.join("s7/idProduct"), "0851");
        write(&usb.join("s7/serial"), "S7");
        write(&usb.join("s7/product"), "YoloLiv YoloCam");
        write(&usb.join("s3/idVendor"), "46f8");
        write(&usb.join("s3/idProduct"), "0855");
        write(&usb.join("s3/serial"), "S3");
        write(&usb.join("s3/product"), "YoloLiv YoloCam");

        write(&v4l.join("video10/name"), "YoloLiv YoloCam: S3");
        fs::create_dir_all(v4l.join("video10")).unwrap();
        symlink(usb.join("s3"), v4l.join("video10/device")).unwrap();
        write(&v4l.join("video34/name"), "YoloLiv YoloCam: S7");
        fs::create_dir_all(v4l.join("video34")).unwrap();
        symlink(usb.join("s7"), v4l.join("video34/device")).unwrap();

        let inv = discover_from(&usb, &v4l);
        assert_eq!(
            inv.s7.as_ref().and_then(|d| d.video.as_deref()),
            Some("/dev/video34")
        );
        assert_eq!(
            inv.s3.as_ref().and_then(|d| d.video.as_deref()),
            Some("/dev/video10")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn named_s3_label_is_a_yolocam() {
        let l = "S3: S3-26220290".to_ascii_lowercase();
        assert!(l.starts_with("s3:"));
    }

    #[test]
    fn empty_inventory_primary_is_unplugged_s7() {
        let root = std::env::temp_dir().join(format!("yolocam-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("usb")).unwrap();
        fs::create_dir_all(root.join("v4l")).unwrap();
        let inv = discover_from(&root.join("usb"), &root.join("v4l"));
        let p = inv.primary();
        assert!(!p.usb);
        assert_eq!(p.model, Model::S7);
        assert_eq!(p.camera_ip, DEFAULT_IP_S7);
        assert_eq!(inv.glyph_code(), 0);
        let _ = fs::remove_dir_all(&root);
    }
}
