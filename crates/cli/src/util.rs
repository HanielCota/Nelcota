//! Utilidades pequenas: segredos aleatórios, datas, prompts, healthcheck.

use std::{
    io::{BufRead, IsTerminal, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const ALPHANUMERIC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Segredo alfanumérico (seguro para URLs e `.env`), sem viés de módulo.
pub fn secret(len: usize) -> String {
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    while out.len() < len {
        getrandom::fill(&mut buf).expect("fonte de aleatoriedade do sistema indisponível");
        for b in buf {
            // 248 = 62 * 4: descarta o resto para a distribuição ficar uniforme.
            if b < 248 && out.len() < len {
                out.push(ALPHANUMERIC[usize::from(b) % ALPHANUMERIC.len()] as char);
            }
        }
    }
    out
}

/// Data/hora UTC atual em `AAAAMMDDTHHMMSSZ`.
pub fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format_timestamp(secs)
}

fn format_timestamp(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    // Algoritmo civil_from_days (Howard Hinnant).
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

pub fn interactive() -> bool {
    std::io::stdin().is_terminal()
}

/// Pergunta com valor padrão (Enter aceita o padrão).
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
        ask(&format!("{question} (s/N)"), "")
            .to_lowercase()
            .as_str(),
        "s" | "sim" | "y" | "yes"
    )
}

/// GET /health via TCP puro (a imagem é `scratch`, sem curl).
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
    fn segredo_alfanumerico_do_tamanho_pedido() {
        let s = secret(40);
        assert_eq!(s.len(), 40);
        assert!(s.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_ne!(secret(40), s);
    }

    #[test]
    fn formata_timestamp_utc() {
        assert_eq!(format_timestamp(0), "19700101T000000Z");
        assert_eq!(format_timestamp(1_791_308_348), "20261006T173908Z");
        assert_eq!(format_timestamp(951_782_400), "20000229T000000Z");
    }
}
