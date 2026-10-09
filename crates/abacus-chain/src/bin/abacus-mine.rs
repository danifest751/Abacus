//! Minimal local mine loop for candidate A. Mines a short chain with block timestamps 2 s apart (5x
//! faster than the 10 s target spacing), so every retarget window raises the difficulty by one bit, and
//! verifies each block.
//!
//! Run: cargo run --release --quiet --bin abacus-mine [blocks]

use abacus_chain::{preheader, verify, Chain, Profile, TARGET_SPACING};
use std::time::Instant;

fn main() {
    let wanted: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20);

    let profile = Profile { n: 8, k: 8, bits: 4 };
    let mut chain = Chain::new(profile, [0xABu8; 32], 2);

    println!(
        "{{\"profile\": {{\"n\": {}, \"k\": {}, \"bits\": {}}}, \"blocks\": [",
        profile.n, profile.k, profile.bits
    );
    let mut ts = 0u64;
    for i in 0..wanted {
        ts += TARGET_SPACING / 5; // 2 s
        let t0 = Instant::now();
        let b = chain.mine_next(ts, 10_000_000).expect("mined");
        let dt = t0.elapsed().as_secs_f64();
        let ph = preheader(&chain.chain_id, chain.version, b.height, &b.prev, b.timestamp, b.bits, b.nonce);
        let ok = verify(&Profile { n: profile.n, k: profile.k, bits: b.bits }, &ph, &b.c, &b.score, None);
        println!(
            "  {{\"height\": {}, \"nonce\": {}, \"bits\": {}, \"seconds\": {dt:.4}, \"verified\": {ok}}}{}",
            b.height,
            b.nonce,
            b.bits,
            if i + 1 == wanted { "" } else { "," }
        );
    }
    println!("]}}");
}
