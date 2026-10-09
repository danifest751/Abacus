//! Verifier-throughput bench: time `verify_fs` (k Fiat–Shamir Freivalds challenges) for selected
//! `(n, k)`. CPU-only; informs the parameter profile. Not a miner or work model. `k = 2, 3` are the
//! profiles suggested by ADR 0009; `32, 128` are kept for comparison with the earlier table.
//!
//! Run: cargo run --release --quiet --bin abacus-verify-bench

use abacus_verifier::freivalds_fs::verify_fs;
use abacus_verifier::{matmul, Rng};
use std::time::Instant;

fn main() {
    println!("{{\n  \"runs\": [");
    let mut first = true;
    for &n in &[256usize, 512, 1024] {
        for &k in &[2usize, 3, 32, 128] {
            let mut rng = Rng::new(1);
            let a = rng.matrix(n);
            let b = rng.matrix(n);

            let t0 = Instant::now();
            let c = matmul(&a, &b, n);
            let build_s = t0.elapsed().as_secs_f64();

            let t1 = Instant::now();
            let ok = verify_fs(b"bench", &a, &b, &c, n, k);
            let verify_s = t1.elapsed().as_secs_f64();

            let macs = (n as f64) * (n as f64) * (n as f64);
            if !first {
                println!(",");
            }
            first = false;
            println!(
                "    {{\"n\": {n}, \"k\": {k}, \"ok\": {ok}, \"build_s\": {build_s:.6}, \"verify_s\": {verify_s:.6}, \"build_GMAC_s\": {:.3}, \"verify_GMAC_s_equiv\": {:.3}, \"verify_over_build\": {:.4}}}",
                macs / build_s / 1e9,
                macs / verify_s / 1e9,
                verify_s / build_s
            );
        }
    }
    println!("\n  ]\n}}");
}
