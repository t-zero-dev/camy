use std::fs;
use std::path::Path;

pub const USB_VID: &str = "46f8";
pub const USB_PID: &str = "0851";
pub const DEFAULT_IP: &str = "192.168.129.10";
pub const DEFAULT_PORT: u16 = 12345;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Discovery {
    pub usb: bool,
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

impl Default for Discovery {
    fn default() -> Self {
        Self {
            usb: false,
            vid_pid: format!("{USB_VID}:{USB_PID}"),
            serial: String::new(),
            product: String::new(),
            iface: None,
            host_ip: None,
            camera_ip: DEFAULT_IP.to_string(),
            port: DEFAULT_PORT,
            video: None,
            video_name: None,
        }
    }
}

pub fn discover() -> Discovery {
    let mut d = Discovery::default();
    if let Some(usb) = find_usb() {
        d.usb = true;
        d.serial = usb.serial;
        d.product = usb.product;
        if let Some(iface) = usb.iface {
            d.iface = Some(iface.clone());
            if let Some(ip) = iface_ipv4(&iface) {
                d.host_ip = Some(ip.clone());
                if let Some(cam) = sibling_ten(&ip) {
                    d.camera_ip = cam;
                }
            }
        }
    }
    if let Some((dev, name)) = find_video() {
        d.video = Some(dev);
        d.video_name = Some(name);
    }
    d
}

struct UsbDev {
    serial: String,
    product: String,
    iface: Option<String>,
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn find_usb() -> Option<UsbDev> {
    let root = Path::new("/sys/bus/usb/devices");
    let entries = fs::read_dir(root).ok()?;
    for ent in entries.flatten() {
        let p = ent.path();
        let Some(vid) = read_trim(&p.join("idVendor")) else { continue };
        let Some(pid) = read_trim(&p.join("idProduct")) else { continue };
        if !vid.eq_ignore_ascii_case(USB_VID) || !pid.eq_ignore_ascii_case(USB_PID) {
            continue;
        }
        let serial = read_trim(&p.join("serial")).unwrap_or_default();
        let product = read_trim(&p.join("product")).unwrap_or_else(|| "YoloCam".into());
        let iface = find_net_iface(&p);
        return Some(UsbDev {
            serial,
            product,
            iface,
        });
    }
    None
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

fn sibling_ten(host: &str) -> Option<String> {
    let mut oct: Vec<u8> = host.split('.').filter_map(|s| s.parse().ok()).collect();
    if oct.len() != 4 {
        return None;
    }
    oct[3] = 10;
    Some(format!("{}.{}.{}.{}", oct[0], oct[1], oct[2], oct[3]))
}

fn find_video() -> Option<(String, String)> {
    let dir = Path::new("/sys/class/video4linux");
    let rd = fs::read_dir(dir).ok()?;
    let mut found = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if !name.starts_with("video") {
            continue;
        }
        let label = read_trim(&ent.path().join("name")).unwrap_or_default();
        let l = label.to_ascii_lowercase();
        if l.contains("yololiv") || l.contains("yolocam") || l.contains("y-cam") || l.contains("yx-cam")
        {
            found.push((format!("/dev/{name}"), label));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.into_iter().next()
}
