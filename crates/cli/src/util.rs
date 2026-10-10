//! Small utilities: random secrets, dates, prompts, healthcheck.

use std::{
    io::{BufRead, IsTerminal, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ALPHANUMERIC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Alphanumeric secret (safe for URLs and `.env`), without modulo bias.
pub fn secret(len: usize) -> String {
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    while out.len() < len {
        getrandom::fill(&mut buf).expect("system randomness source unavailable");
        for b in buf {
            // 248 = 62 * 4: discard the remainder so the distribution stays uniform.
            if b < 248 && out.len() < len {
                out.push(ALPHANUMERIC[usize::from(b) % ALPHANUMERIC.len()] as char);
            }
        }
    }
    out
}

/// Current UTC date/time as `YYYYMMDDTHHMMSSZ`.
pub fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_timestamp(secs)
}

fn format_timestamp(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    // civil_from_days algorithm (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// How long ago `time` was, e.g. `45m`, `5h`, `3d`.
pub fn age(time: SystemTime) -> String {
    format_age(SystemTime::now().duration_since(time).unwrap_or_default())
}

fn format_age(age: Duration) -> String {
    let secs = age.as_secs();
    match secs {
        0..3600 => format!("{}m", secs / 60),
        3600..172_800 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

pub fn interactive() -> bool {
    std::io::stdin().is_terminal()
}

/// Question with a default value (Enter accepts the default).
pub fn ask(question: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{question}: ");
    } else {
        print!("{question} [{default}]: ");
    }
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    let line = line.trim();
    if line.is_empty() {
        default.to_owned()
    } else {
        line.to_owned()
    }
}

pub fn confirm(question: &str) -> bool {
    matches!(
        ask(&format!("{question} (y/N)"), "")
            .to_lowercase()
            .as_str(),
        "y" | "yes"
    )
}

/// GET /health over plain TCP (the image is `scratch`, no curl).
pub fn healthcheck(addr: &str) -> bool {
    let Some(socket) = addr.to_socket_addrs().ok().and_then(|mut a| a.next()) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&socket, Duration::from_secs(2)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let request = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = [0u8; 16];
    let mut read = 0;
    while read < response.len() {
        match stream.read(&mut response[read..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => read += n,
        }
    }
    response[..read].starts_with(b"HTTP/1.1 200")
}

pub fn step(message: &str) {
    println!("→ {message}");
}

pub fn ok(message: &str) {
    println!("  ✓ {message}");
}

pub fn warn(message: &str) {
    println!("  ! {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphanumeric_secret_of_the_requested_length() {
        let s = secret(40);
        assert_eq!(s.len(), 40);
        assert!(s.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_ne!(secret(40), s);
    }

    #[test]
    fn formats_ages() {
        assert_eq!(format_age(Duration::from_secs(59)), "0m");
        assert_eq!(format_age(Duration::from_secs(45 * 60)), "45m");
        assert_eq!(format_age(Duration::from_secs(5 * 3600 + 10)), "5h");
        assert_eq!(format_age(Duration::from_secs(47 * 3600)), "47h");
        assert_eq!(format_age(Duration::from_secs(3 * 86_400)), "3d");
    }

    #[test]
    fn formats_utc_timestamp() {
        assert_eq!(format_timestamp(0), "19700101T000000Z");
        assert_eq!(format_timestamp(1_791_308_348), "20261006T173908Z");
        assert_eq!(format_timestamp(951_782_400), "20000229T000000Z");
    }
}
