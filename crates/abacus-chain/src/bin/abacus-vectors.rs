//! Derivation vectors for the Python/Rust differential test (`tests/test_chain_parity.py`).
//!
//! Args: chain_id_hex version height prev_hex timestamp bits nonce n k nblocks epoch_seed_hex
//! Prints `key=value` lines (hex or comma-separated decimals) for every consensus derivation:
//! preheader, plain instance, product, score, block id, Fiat–Shamir challenges, the A' dataset,
//! gather indices and gathered instance, and a Fiat–Shamir sumcheck transcript.

use abacus_chain::*;
use abacus_verifier::{freivalds_fs::fs_challenges, matmul, sha256::sha256, sumcheck};

fn unhex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    assert_eq!(s.len(), 64, "expected 32-byte hex");
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex");
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn dec(v: &[u64]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(a.len(), 11, "usage: chain_id version height prev ts bits nonce n k nblocks epoch_seed");
    let chain_id = unhex32(&a[0]);
    let version: u32 = a[1].parse().unwrap();
    let height: u64 = a[2].parse().unwrap();
    let prev = unhex32(&a[3]);
    let ts: u64 = a[4].parse().unwrap();
    let bits: u32 = a[5].parse().unwrap();
    let nonce: u64 = a[6].parse().unwrap();
    let n: usize = a[7].parse().unwrap();
    let k: usize = a[8].parse().unwrap();
    let nblocks: usize = a[9].parse().unwrap();
    let epoch = unhex32(&a[10]);

    let ph = preheader(&chain_id, version, height, &prev, ts, bits, nonce);
    println!("ph={}", hex(&ph));
    let (ma, mb) = instance(&ph, n);
    let c = matmul(&ma, &mb, n);
    println!("a={}", dec(&ma));
    println!("b={}", dec(&mb));
    println!("c={}", dec(&c));
    println!("score={}", hex(&score(&ph, &c)));
    println!("lead={}", score_lead(&score(&ph, &c)));
    println!("id={}", hex(&block_id(&ph, &c)));
    println!("fs={}", dec(&fs_challenges(&ph, &c, n, k).concat()));

    let ds = build_dataset(&epoch, nblocks);
    let flat: Vec<u8> = ds.iter().flatten().copied().collect();
    println!("ds_first={}", hex(&ds[0]));
    println!("ds_last={}", hex(&ds[nblocks - 1]));
    println!("ds_sha={}", hex(&sha256(&flat)));
    let mut sm = Vec::new();
    sm.extend_from_slice(DOM_INSTANCE);
    sm.extend_from_slice(&ph);
    let seed = sha256(&sm);
    let idx: Vec<u64> = expand_indices(&seed, 2 * n * n, nblocks).into_iter().map(|x| x as u64).collect();
    println!("idx={}", dec(&idx));
    let (ha, hb) = instance_hard(&ph, n, &ds);
    let hc = matmul(&ha, &hb, n);
    println!("hard_a={}", dec(&ha));
    println!("hard_b={}", dec(&hb));
    println!("hard_score={}", hex(&score(&ph, &hc)));

    // Sumcheck over a 16-entry table expanded from the preheader.
    let table = expand(&ph, 16);
    let (claimed, rounds) = sumcheck::prove(&table, 4);
    let flat_rounds: Vec<u64> = rounds.iter().flat_map(|&(g0, g1)| [g0, g1]).collect();
    println!("sc_table={}", dec(&table));
    println!("sc_claimed={claimed}");
    println!("sc_rounds={}", dec(&flat_rounds));
    println!("sc_challenges={}", dec(&sumcheck::challenges(&table, 4, claimed, &rounds)));
}
