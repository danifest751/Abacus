//! Verifier-throughput bench: time `verify_fs` (hash of `C`, challenge expansion and `k` Fiat–Shamir
//! Freivalds checks) for selected `(n, k)`, against one naive `n^3` CPU product. CPU-only; informs the
//! parameter profile. Each verification is timed `REPS` times and the median is reported. `k = 2, 3`
//! are the profiles suggested by ADR 0009; `32, 128` are kept for comparison.
//!
//! Instance expansion from the header is not included here (see `abacus-block-bench` in
//! `abacus-chain` for the full block check).
//!
//! Run: cargo run --release --quiet --bin abacus-verify-bench

use abacus_verifier::freivalds_fs::verify_fs;
use abacus_verifier::{matmul, Rng};
use std::time::Instant;

const REPS: usize = 7;

fn median(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn main() {
    println!("{{\n  \"reps\": {REPS},\n  \"runs\": [");
    let mut first = true;
    for &n in &[256usize, 512, 1024] {
        let mut rng = Rng::new(1);
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let t0 = Instant::now();
        let c = matmul(&a, &b, n);
        let build_s = t0.elapsed().as_secs_f64();

        for &k in &[2usize, 3, 32, 128] {
            let mut ok = true;
            let times: Vec<f64> = (0..REPS)
                .map(|_| {
                    let t1 = Instant::now();
                    ok &= verify_fs(b"bench", &a, &b, &c, n, k);
                    t1.elapsed().as_secs_f64()
                })
                .collect();
            let (verify_s, vmin, vmax) = median(times);
            if !first {
                println!(",");
            }
            first = false;
            print!(
                "    {{\"n\": {n}, \"k\": {k}, \"ok\": {ok}, \"build_s\": {build_s:.6}, \"verify_s\": {verify_s:.6}, \
                 \"verify_s_min\": {vmin:.6}, \"verify_s_max\": {vmax:.6}, \"verify_over_build\": {:.4}}}",
                verify_s / build_s
            );
        }
    }
    println!("\n  ]\n}}");
}
