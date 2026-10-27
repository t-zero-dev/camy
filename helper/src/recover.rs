//! Restart the camera control process over telnet so a wedged TCP 12345
//! does not require a power-cycle.

use crate::error::{Error, Result};

#[cfg(feature = "recover")]
use std::io::{Read, Write};
#[cfg(feature = "recover")]
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(feature = "recover")]
use std::time::{Duration, Instant};

#[cfg(feature = "recover")]
const USER: &str = "root";
#[cfg(feature = "recover")]
const PASS: &str = "yunxi.tv";
#[cfg(feature = "recover")]
const IAC: u8 = 255;
#[cfg(feature = "recover")]
const WILL: u8 = 251;
#[cfg(feature = "recover")]
const WONT: u8 = 252;
#[cfg(feature = "recover")]
const DO: u8 = 253;
#[cfg(feature = "recover")]
const DONT: u8 = 254;

pub fn control_refused(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("connection refused") || e.contains("camera closed the socket")
}

/// Kill and relaunch `/app/bin/live_camera_app`, then wait until `ip:port` accepts.
///
/// The telnet path is compiled only with `--features recover`. The helper the
/// panel ships never contains vendor default credentials and never opens :23.
pub fn restart_control(ip: &str, port: u16) -> Result<()> {
    #[cfg(not(feature = "recover"))]
    {
        let _ = (ip, port);
        Err(Error::Invalid("recover is disabled".into()))
    }
    #[cfg(feature = "recover")]
    {
        telnet_exec(
            ip,
            "killall live_camera_app; sleep 2; /app/bin/live_camera_app >/dev/null 2>&1 &",
        )?;
        wait_port(ip, port, Duration::from_secs(8))
    }
}

#[cfg(feature = "recover")]
fn telnet_exec(ip: &str, cmd: &str) -> Result<()> {
    let addr = format!("{ip}:23");
    let mut stream = TcpStream::connect_timeout(&parse_addr(&addr)?, Duration::from_secs(3))
        .map_err(|e| Error::io_ctx(e, format!("telnet {addr}")))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(800)))
        .ok();
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok();
    wait_contains(&mut stream, "login:")?;
    write_line(&mut stream, USER)?;
    wait_contains(&mut stream, "assword:")?;
    write_line(&mut stream, PASS)?;
    wait_contains(&mut stream, "# ")?;
    write_line(&mut stream, cmd)?;
    let _ = wait_contains(&mut stream, "# ");
    write_line(&mut stream, "exit")?;
    Ok(())
}

#[cfg(feature = "recover")]
fn parse_addr(addr: &str) -> Result<SocketAddr> {
    addr.to_socket_addrs()
        .map_err(|e| Error::io_ctx(e, "addr"))?
        .next()
        .ok_or_else(|| Error::Invalid(format!("no address for {addr}")))
}

#[cfg(feature = "recover")]
fn write_line(stream: &mut TcpStream, line: &str) -> Result<()> {
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .map_err(|e| Error::io_ctx(e, "telnet write"))
}

#[cfg(feature = "recover")]
fn wait_contains(stream: &mut TcpStream, needle: &str) -> Result<String> {
    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut raw = [0u8; 1024];
    while Instant::now() < deadline {
        match stream.read(&mut raw) {
            Ok(0) => return Err(Error::Disconnected("telnet closed")),
            Ok(n) => {
                let chunk = eat_iac(stream, &raw[..n])?;
                if text.len().saturating_add(chunk.len()) > 8192 {
                    return Err(Error::Protocol("telnet reply too large".into()));
                }
                text.push_str(&String::from_utf8_lossy(&chunk));
                if text
                    .to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
                {
                    return Ok(text);
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(Error::io_ctx(e, "telnet read")),
        }
    }
    Err(Error::Protocol(format!(
        "telnet timeout waiting for {needle:?}"
    )))
}

#[cfg(feature = "recover")]
fn eat_iac(stream: &mut TcpStream, data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == IAC && i + 2 < data.len() {
            let cmd = data[i + 1];
            let opt = data[i + 2];
            if cmd == WILL || cmd == DO {
                let reply = if cmd == WILL { DONT } else { WONT };
                stream
                    .write_all(&[IAC, reply, opt])
                    .map_err(|e| Error::io_ctx(e, "telnet iac"))?;
            }
            i += 3;
            continue;
        }
        if data[i] != IAC {
            out.push(data[i]);
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(feature = "recover")]
fn wait_port(ip: &str, port: u16, budget: Duration) -> Result<()> {
    let addr = parse_addr(&format!("{ip}:{port}"))?;
    let deadline = Instant::now() + budget;
    let mut last = String::new();
    while Instant::now() < deadline {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(400)) {
            Ok(_) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(Error::Protocol(format!(
        "{ip}:{port} still down after restart ({last})"
    )))
}

#[cfg(test)]
mod tests {
    use super::control_refused;

    #[test]
    fn detects_refused() {
        assert!(control_refused(
            "connect 192.168.129.10:12345: Connection refused"
        ));
        assert!(control_refused("camera closed the socket"));
        assert!(!control_refused("timeout waiting for cid 7"));
    }

    #[cfg(feature = "recover")]
    #[test]
    fn missing_host_is_typed() {
        let err = super::parse_addr("not-a-socket").unwrap_err();
        assert!(matches!(err, Error::Io { .. } | Error::Invalid(_)));
        assert!(!err.to_string().is_empty());
    }

    #[cfg(not(feature = "recover"))]
    #[test]
    fn recover_is_disabled() {
        let err = super::restart_control("127.0.0.1", 12345).unwrap_err();
        assert!(err.to_string().contains("disabled"));
    }
}
