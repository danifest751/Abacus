//! Differential adapter: reads cases from stdin, prints one Freivalds verdict per line.
//!
//! Protocol (whitespace-separated):
//!   line 1: n cases
//!   then `cases` lines, each with `3*n*n + n` integers in this order:
//!     A (n*n), B (n*n), C (n*n), r (n)
//! Output: one line per case, `1` if Freivalds accepts C == A*B, else `0`.

use abacus_verifier::freivalds_verify;
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    let header = match lines.next() {
        Some(Ok(l)) => l,
        _ => {
            eprintln!("adapter: missing header");
            std::process::exit(2);
        }
    };
    let mut h = header.split_whitespace();
    let n: usize = h.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let cases: usize = h.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    if n == 0 {
        eprintln!("adapter: bad n");
        std::process::exit(2);
    }

    let nn = n * n;
    let mut out = io::BufWriter::new(io::stdout().lock());

    for _ in 0..cases {
        let line = match lines.next() {
            Some(Ok(l)) => l,
            _ => {
                eprintln!("adapter: missing case");
                std::process::exit(2);
            }
        };
        let nums: Vec<u64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if nums.len() != 3 * nn + n {
            writeln!(out, "E").unwrap();
            continue;
        }
        let a = &nums[0..nn];
        let b = &nums[nn..2 * nn];
        let c = &nums[2 * nn..3 * nn];
        let r = &nums[3 * nn..3 * nn + n];
        let ok = freivalds_verify(a, b, c, n, r);
        writeln!(out, "{}", if ok { 1 } else { 0 }).unwrap();
    }
    out.flush().unwrap();
}
