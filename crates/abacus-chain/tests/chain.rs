use abacus_chain::{accept, best_chain, mine, preheader, score, verify, Block, Chain, Profile};
use abacus_verifier::P;

fn profile(bits: u32) -> Profile {
    Profile { n: 4, k: 4, bits }
}

fn mine_one(bits: u32) -> (Profile, Vec<u8>, Vec<u64>, [u8; 32]) {
    let p = profile(bits);
    let chain = [7u8; 32];
    let prev = [0u8; 32];
    let (nonce, c, sc) = mine(&p, &chain, 1, 0, &prev, 0, 100_000, None).expect("mined");
    let ph = preheader(&chain, 1, 0, &prev, 0, nonce);
    (p, ph, c, sc)
}

#[test]
fn mines_and_verifies() {
    let (p, ph, c, sc) = mine_one(8);
    assert!(accept(&sc, 8));
    assert!(verify(&p, &ph, &c, &sc, None));
}

#[test]
fn rejects_tampered_product() {
    let (p, ph, mut c, sc) = mine_one(8);
    c[0] = (c[0] + 1) % P;
    assert!(!verify(&p, &ph, &c, &sc, None));
}

#[test]
fn rejects_wrong_score() {
    let (p, ph, c, mut sc) = mine_one(8);
    sc[0] ^= 0xFF;
    assert!(!verify(&p, &ph, &c, &sc, None));
}

#[test]
fn memory_hard_gather_roundtrip_and_domain_separation() {
    use abacus_chain::{build_dataset, verify};
    let p = Profile { n: 4, k: 4, bits: 6 };
    let ds = build_dataset(&[9u8; 32], 512);
    // deterministic dataset
    assert_eq!(ds, build_dataset(&[9u8; 32], 512));
    assert_ne!(ds, build_dataset(&[10u8; 32], 512));

    let mut chain = Chain::new(p, [1u8; 32], 1).with_dataset(ds.clone());
    let b = chain.mine_next(10, 1_000_000).expect("mined (hard)");
    let ph = preheader(&chain.chain_id, 1, b.height, &b.prev, b.timestamp, b.nonce);
    assert!(verify(&p, &ph, &b.c, &b.score, Some(&ds)));
    // The same block must not verify against the plain (non-gathered) instance.
    assert!(!verify(&p, &ph, &b.c, &b.score, None));
}

#[test]
fn chain_appends_and_rejects_bad_prev() {
    let mut chain = Chain::new(profile(4), [1u8; 32], 1);
    for i in 0..3u64 {
        assert!(chain.mine_next((i + 1) * 10, 100_000).is_some());
    }
    assert_eq!(chain.height(), 3);
    let bad = Block {
        height: chain.height(),
        prev: [9u8; 32],
        timestamp: 100,
        nonce: 0,
        c: vec![0u64; 16],
        score: [0u8; 32],
        id: [0u8; 32],
        bits: 4,
    };
    assert!(!chain.append_checked(bad));
}

#[test]
fn retarget_moves_bits_with_time() {
    // Fast blocks (small timestamp deltas) should raise the difficulty at the window boundary.
    let mut fast = Chain::new(profile(4), [2u8; 32], 1);
    for i in 0..16u64 {
        fast.mine_next(i + 1, 1_000_000).unwrap(); // ~1s spacing -> too fast
    }
    assert!(fast.next_bits() > 4);

    // Slow blocks (large deltas) should lower it.
    let mut slow = Chain::new(profile(12), [3u8; 32], 1);
    for i in 0..16u64 {
        slow.mine_next((i + 1) * 1000, 1_000_000).unwrap(); // ~1000s spacing -> too slow
    }
    assert!(slow.next_bits() < 12);
}

#[test]
fn best_chain_prefers_more_work() {
    let mut a = Chain::new(profile(4), [4u8; 32], 1);
    a.mine_next(10, 1_000_000).unwrap();
    let mut b = Chain::new(profile(4), [5u8; 32], 1);
    for i in 0..2u64 {
        b.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    assert!(std::ptr::eq(best_chain(&a, &b), &b));
    assert!(b.cumulative_work() > a.cumulative_work());
}
