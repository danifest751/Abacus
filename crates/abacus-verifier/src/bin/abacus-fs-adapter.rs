//! Fiat-Shamir challenge adapter: reads (n, k, preheader, C) from stdin, prints the k*n challenge
//! elements flattened, one per line. Lets the Python reference check derivation parity.
//!
//! Protocol: line 1 = "n k"; line 2 = preheader as hex ("-" for empty); line 3+ = n*n field
//! elements (C), whitespace-separated.

use abacus_verifier::freivalds_fs::fs_challenges;
use std::io::{self, BufRead, Write};

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s == "-" {
        return Some(Vec::new());
    }
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let header = lines.next().and_then(|r| r.ok()).unwrap_or_default();
    let mut it = header.split_whitespace();
    let n: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let k: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    assert!(n > 0 && k > 0, "bad header");
    let ph_line = lines.next().and_then(|r| r.ok()).unwrap_or_default();
    let ph = unhex(ph_line.trim()).expect("bad preheader hex");

    let mut c: Vec<u64> = Vec::with_capacity(n * n);
    while c.len() < n * n {
        match lines.next() {
            Some(Ok(l)) => c.extend(l.split_whitespace().filter_map(|x| x.parse::<u64>().ok())),
            _ => break,
        }
    }
    c.truncate(n * n);

    let rs = fs_challenges(&ph, &c, n, k);
    let out = io::stdout();
    let mut w = io::BufWriter::new(out.lock());
    for v in &rs {
        for x in v {
            writeln!(w, "{x}").unwrap();
        }
    }
    w.flush().unwrap();
}
