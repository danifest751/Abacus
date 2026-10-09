//! Minimal local mine loop for candidate A. Mines a short chain and verifies it.
//!
//! Run: cargo run --release --quiet --bin abacus-mine [blocks]

use abacus_chain::{mine, preheader, verify, Profile};
use std::time::Instant;

fn main() {
    let blocks_wanted: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(5);

    // Easy prototype target (first byte 0x10 -> ~1/16 attempts); small n/k for CPU speed.
    let mut target = [0u8; 32];
    target[0] = 0x10;
    let profile = Profile { n: 8, k: 8, target };
    let chain_id = [0xABu8; 32];

    let mut prev = [0u8; 32];
    println!("{{\"profile\": {{\"n\": {}, \"k\": {}, \"target0\": {}}}, \"blocks\": [", profile.n, profile.k, target[0]);

    for height in 0..blocks_wanted {
        let t0 = Instant::now();
        let (nonce, c, sc) = mine(&profile, &chain_id, 1, height, &prev, 0, 1_000_000).expect("mined");
        let dt = t0.elapsed().as_secs_f64();
        let ph = preheader(&chain_id, 1, height, &prev, nonce);
        let ok = verify(&profile, &ph, &c, &sc);
        println!(
            "  {{\"height\": {height}, \"nonce\": {nonce}, \"attempts\": {}, \"seconds\": {dt:.4}, \"verified\": {ok}}}{}",
            nonce + 1,
            if height + 1 == blocks_wanted { "" } else { "," }
        );
        // next prev = block id (recompute here; the chain module stores it, the loop keeps it simple)
        prev = abacus_chain::block_id(&ph, &c);
    }
    println!("]}}");
}
