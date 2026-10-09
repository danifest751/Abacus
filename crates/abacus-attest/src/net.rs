//! Network roles of the attestation protocol (spec/06): the verifier client and the reference CPU
//! prover. Line protocol over TCP:
//!
//! ```text
//! V -> P   CHAL <seed hex> <n> <m>
//! P -> V   ROOT <root hex>                       (the verifier's clock measures CHAL -> ROOT)
//! V -> P   OPEN j:i j:i ...                      (chosen only after ROOT)
//! P -> V   ROW <j> <i> <row hex> <path hex>      (one per opened row)
//! P -> V   END
//! ```

use crate::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn read_line<R: BufRead>(r: &mut R, max: usize) -> std::io::Result<String> {
    let mut buf = Vec::new();
    r.by_ref().take(max as u64 + 1).read_until(b'\n', &mut buf)?;
    if buf.len() > max {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "line too long"));
    }
    Ok(String::from_utf8_lossy(&buf).trim_end().to_string())
}

/// Outcome of one challenge as seen by the verifier.
#[derive(Debug, Clone)]
pub struct RoundResult {
    pub n: usize,
    pub m: usize,
    pub k: usize,
    pub accepted: bool,
    pub rows_ok: usize,
    pub rows_opened: usize,
    pub precompute_ms: f64,
    pub prove_ms: f64,
    pub open_ms: f64,
    pub verify_ms: f64,
}

impl RoundResult {
    /// Multiply-adds per second implied by the measured CHAL -> ROOT time.
    pub fn implied_tmac_s(&self) -> f64 {
        self.m as f64 * (self.n as f64).powi(3) / (self.prove_ms / 1e3) / 1e12
    }
}

/// Run one challenge against `prover` with the given seed and verifier secret.
pub fn verify_round(
    prover: &str,
    n: usize,
    m: usize,
    k: usize,
    seed: &[u8; 32],
    secret: &[u8; 32],
) -> std::io::Result<RoundResult> {
    let t = Instant::now();
    let pre = precompute(seed, n, m, secret);
    let precompute_ms = t.elapsed().as_secs_f64() * 1e3;

    let stream = TcpStream::connect(prover)?;
    stream.set_read_timeout(Some(Duration::from_secs(600)))?;
    stream.set_nodelay(true)?;
    let mut w = stream.try_clone()?;
    let mut r = BufReader::new(stream);
    let depth = tree_depth(n * m);
    let max_line = 32 + 8 * n + 64 * depth;

    let t0 = Instant::now();
    writeln!(w, "CHAL {} {n} {m}", hex(seed))?;
    w.flush()?;
    let root_line = read_line(&mut r, 128)?;
    let prove_ms = t0.elapsed().as_secs_f64() * 1e3;
    let root = root_line.strip_prefix("ROOT ").and_then(unhex32);

    let picks = root.map(|rt| sample(secret, &rt, n, m, k)).unwrap_or_default();
    let t1 = Instant::now();
    let open: Vec<String> = picks.iter().map(|(j, i)| format!("{j}:{i}")).collect();
    writeln!(w, "OPEN {}", open.join(" "))?;
    w.flush()?;
    let mut rows = Vec::with_capacity(picks.len());
    loop {
        let line = read_line(&mut r, max_line)?;
        if line == "END" || line.is_empty() {
            break;
        }
        rows.push(line);
    }
    let open_ms = t1.elapsed().as_secs_f64() * 1e3;

    let t2 = Instant::now();
    let mut rows_ok = 0usize;
    if let Some(rt) = root {
        for (line, &(pj, pi)) in rows.iter().zip(&picks) {
            if let Some((j, i, row, path)) = decode_row_line(line, n, depth) {
                let idx = j as usize * n + i as usize;
                if (j, i) == (pj, pi)
                    && verify_path(&rt, &leaf_hash(j, i, &row), idx, &path)
                    && check_row(&pre, seed, j, i as usize, &row)
                {
                    rows_ok += 1;
                }
            }
        }
    }
    let verify_ms = t2.elapsed().as_secs_f64() * 1e3;
    let accepted = root.is_some() && !picks.is_empty() && rows_ok == picks.len() && rows.len() == picks.len();
    Ok(RoundResult { n, m, k, accepted, rows_ok, rows_opened: rows.len(), precompute_ms, prove_ms, open_ms, verify_ms })
}

/// Serve one challenge with the reference CPU prover (`n <= 1024`, `m <= 64`). `cheat_rows` commits
/// zeros for the last rows of every product instead of computing them.
pub fn serve_cpu(stream: TcpStream, cheat_rows: usize) -> std::io::Result<()> {
    let mut w = stream.try_clone()?;
    let mut r = BufReader::new(stream);
    let line = read_line(&mut r, 256)?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 4 || parts[0] != "CHAL" {
        return Ok(());
    }
    let seed = unhex32(parts[1]).unwrap_or([0u8; 32]);
    let n: usize = parts[2].parse().unwrap_or(0);
    let m: usize = parts[3].parse().unwrap_or(0);
    if n == 0 || n > 1024 || m == 0 || m > 64 {
        return Ok(());
    }
    let rows = cpu_prove(&seed, n, m, cheat_rows.min(n));
    let levels = merkle_levels(&leaves_of(&rows));
    writeln!(w, "ROOT {}", hex(&levels.last().unwrap()[0]))?;
    w.flush()?;
    let open = read_line(&mut r, 1 << 20)?;
    for item in open.strip_prefix("OPEN ").unwrap_or("").split(' ').filter(|s| !s.is_empty()) {
        if let Some((a, b)) = item.split_once(':') {
            let (j, i) = (a.parse::<usize>().unwrap_or(usize::MAX), b.parse::<usize>().unwrap_or(usize::MAX));
            if j < m && i < n {
                let path = merkle_path(&levels, j * n + i);
                writeln!(w, "{}", encode_row_line(j as u32, i as u32, &rows[j][i], &path))?;
            }
        }
    }
    writeln!(w, "END")?;
    w.flush()
}
