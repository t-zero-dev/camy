//! Restart the camera control process over telnet so a wedged TCP 12345
//! does not require a power-cycle.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

const USER: &str = "root";
const PASS: &str = "yunxi.tv";
const IAC: u8 = 255;
const WILL: u8 = 251;
const WONT: u8 = 252;
const DO: u8 = 253;
const DONT: u8 = 254;

pub fn control_refused(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("connection refused") || e.contains("camera closed the socket")
}

/// Kill and relaunch `/app/bin/live_camera_app`, then wait until `ip:port` accepts.
pub fn restart_control(ip: &str, port: u16) -> Result<()> {
    telnet_exec(
        ip,
        "killall live_camera_app; sleep 2; /app/bin/live_camera_app >/dev/null 2>&1 &",
    )?;
    wait_port(ip, port, Duration::from_secs(8))
}

fn telnet_exec(ip: &str, cmd: &str) -> Result<()> {
    let addr = format!("{ip}:23");
    let mut stream = TcpStream::connect_timeout(&parse_addr(&addr)?, Duration::from_secs(3))
        .map_err(|e| Error::io_ctx(e, format!("telnet {addr}")))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(800)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .ok();
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

fn parse_addr(addr: &str) -> Result<SocketAddr> {
    addr.to_socket_addrs()
        .map_err(|e| Error::io_ctx(e, "addr"))?
        .next()
        .ok_or_else(|| Error::Invalid(format!("no address for {addr}")))
}

fn write_line(stream: &mut TcpStream, line: &str) -> Result<()> {
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .map_err(|e| Error::io_ctx(e, "telnet write"))
}

fn wait_contains(stream: &mut TcpStream, needle: &str) -> Result<String> {
    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut raw = [0u8; 1024];
    while Instant::now() < deadline {
        match stream.read(&mut raw) {
            Ok(0) => return Err(Error::Disconnected("telnet closed")),
            Ok(n) => {
                let chunk = eat_iac(stream, &raw[..n])?;
                text.push_str(&String::from_utf8_lossy(&chunk));
                if text.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) {
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
    use crate::Error;

    #[test]
    fn detects_refused() {
        assert!(control_refused("connect 192.168.129.10:12345: Connection refused"));
        assert!(control_refused("camera closed the socket"));
        assert!(!control_refused("timeout waiting for cid 7"));
    }

    #[test]
    fn missing_host_is_typed() {
        let err = super::parse_addr("not-a-socket").unwrap_err();
        assert!(matches!(err, Error::Io { .. } | Error::Invalid(_)));
        assert!(!err.to_string().is_empty());
    }
}
