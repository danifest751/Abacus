//! Full block-verification bench for candidate A: the cost a node pays per received block.
//!
//! For each `(n, k)`: instance expansion from the preheader alone, and the complete `verify` (shape and
//! canonicality, score hash, instance expansion, challenge derivation, `k` Freivalds checks). Each is
//! timed `REPS` times on one CPU thread and the median (min, max) is reported. The product itself is
//! computed once, untimed.
//!
//! Run: cargo run --release --quiet --bin abacus-block-bench

use abacus_chain::{instance, preheader, score, verify, Profile};
use abacus_verifier::matmul;
use std::time::Instant;

const REPS: usize = 7;

fn stats(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn time<F: FnMut()>(mut f: F) -> (f64, f64, f64) {
    stats(
        (0..REPS)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64()
            })
            .collect(),
    )
}

fn main() {
    println!("{{\n  \"reps\": {REPS},\n  \"runs\": [");
    let mut first = true;
    for &n in &[64usize, 256, 512] {
        let ph = preheader(&[0xAB; 32], 2, 7, &[0x11; 32], 1_760_000_000, 0, 42);
        let (a, b) = instance(&ph, n);
        let c = matmul(&a, &b, n);
        let sc = score(&ph, &c);
        let (exp_s, exp_min, exp_max) = time(|| {
            std::hint::black_box(instance(&ph, n));
        });
        for &k in &[2usize, 3] {
            let p = Profile { n, k, bits: 0 };
            let mut ok = true;
            let (ver_s, ver_min, ver_max) = time(|| ok &= verify(&p, &ph, &c, &sc, None));
            if !first {
                println!(",");
            }
            first = false;
            print!(
                "    {{\"n\": {n}, \"k\": {k}, \"ok\": {ok}, \"expand_s\": {exp_s:.6}, \"expand_s_min\": {exp_min:.6}, \
                 \"expand_s_max\": {exp_max:.6}, \"verify_block_s\": {ver_s:.6}, \"verify_block_s_min\": {ver_min:.6}, \
                 \"verify_block_s_max\": {ver_max:.6}}}"
            );
        }
    }
    println!("\n  ]\n}}");
}
