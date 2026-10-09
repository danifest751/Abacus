//! Verifier for the interactive proof of tensor throughput (spec/06).
//!
//! Args: --prover HOST:PORT  --n N (default 4096)  --m M (products, default 4)  --k K (rows, default 32)
//!       --rounds R (challenges, default 1)
//! Prints one JSON line per round: timings, the implied throughput and the probability of catching a
//! prover that skipped a fraction f of the rows.

use abacus_attest::net::verify_round;
use abacus_attest::{os_random32, pass_probability, MAX_N};

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned()).unwrap_or_else(|| default.to_string())
}

fn main() {
    let prover = arg("--prover", "127.0.0.1:9600");
    let n: usize = arg("--n", "4096").parse().expect("--n");
    let m: usize = arg("--m", "4").parse().expect("--m");
    let k: usize = arg("--k", "32").parse().expect("--k");
    let rounds: u64 = arg("--rounds", "1").parse().expect("--rounds");
    assert!(n > 0 && n <= MAX_N && n.is_multiple_of(16) && m > 0, "n must be a multiple of 16 in (0, 2^16], m > 0");
    for t in 0..rounds {
        let seed = os_random32(2 * t);
        let secret = os_random32(2 * t + 1);
        match verify_round(&prover, n, m, k, &seed, &secret) {
            Ok(r) => {
                let det = |f: f64| 1.0 - pass_probability(f, n * m, k);
                println!(
                    "{{\"n\": {n}, \"m\": {m}, \"k\": {k}, \"accepted\": {}, \"rows_ok\": {}, \"rows_opened\": {}, \
                     \"precompute_ms\": {:.1}, \"prove_ms\": {:.1}, \"open_ms\": {:.1}, \"verify_ms\": {:.2}, \
                     \"implied_TMAC_s\": {:.3}, \"detect_f01\": {:.4}, \"detect_f05\": {:.4}, \"detect_f10\": {:.4}}}",
                    r.accepted,
                    r.rows_ok,
                    r.rows_opened,
                    r.precompute_ms,
                    r.prove_ms,
                    r.open_ms,
                    r.verify_ms,
                    r.implied_tmac_s(),
                    det(0.01),
                    det(0.05),
                    det(0.10)
                );
            }
            Err(e) => println!("{{\"error\": \"{e}\"}}"),
        }
    }
}
