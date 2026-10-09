use abacus_chain::{
    accept, best_chain, block_id, build_dataset, dataset_ref, mine, preheader, score, score_lead, verify, Block, Chain,
    Profile, Reject, MTP_WINDOW,
};
use abacus_verifier::P;

fn profile(bits: u32) -> Profile {
    Profile { n: 4, k: 4, bits }
}

fn mine_one(bits: u32) -> (Profile, Vec<u8>, Vec<u64>, [u8; 32]) {
    let p = profile(bits);
    let chain = [7u8; 32];
    let prev = [0u8; 32];
    let (nonce, c, sc) = mine(&p, &chain, 1, 0, &prev, 0, 100_000, None).expect("mined");
    let ph = preheader(&chain, 1, 0, &prev, 0, bits, nonce);
    (p, ph, c, sc)
}

/// Build a correctly mined block for `chain` at an arbitrary (possibly wrong) `bits`.
fn block_at(chain: &Chain, bits: u32, timestamp: u64) -> Block {
    let p = Profile { n: chain.profile.n, k: chain.profile.k, bits };
    let (h, prev) = (chain.height(), chain.tip());
    let (nonce, c, sc) =
        mine(&p, &chain.chain_id, chain.version, h, &prev, timestamp, 1_000_000, chain.dataset_slice()).expect("mined");
    let ph = preheader(&chain.chain_id, chain.version, h, &prev, timestamp, bits, nonce);
    Block { height: h, prev, timestamp, bits, nonce, id: block_id(&ph, &c), c, score: sc }
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
fn rejects_wrong_length_or_noncanonical_c_without_panicking() {
    let (p, ph, c, _) = mine_one(4);
    let short = &c[..3];
    assert!(!verify(&Profile { bits: 0, ..p }, &ph, short, &score(&ph, short), None));
    let mut big = c.clone();
    big[0] = big[0].wrapping_add(P); // may wrap; either way not a canonical residue or not the product
    assert!(!verify(&p, &ph, &big, &score(&ph, &big), None));
}

#[test]
fn memory_hard_gather_roundtrip_and_domain_separation() {
    let p = Profile { n: 4, k: 4, bits: 6 };
    let ds = build_dataset(&[9u8; 32], 512);
    assert_eq!(ds, build_dataset(&[9u8; 32], 512));
    assert_ne!(ds, build_dataset(&[10u8; 32], 512));

    let mut chain = Chain::new(p, [1u8; 32], 1).with_dataset(ds.clone());
    let b = chain.mine_next(10, 1_000_000).expect("mined (hard)");
    let ph = preheader(&chain.chain_id, 1, b.height, &b.prev, b.timestamp, b.bits, b.nonce);
    assert!(verify(&p, &ph, &b.c, &b.score, Some(&ds)));
    // The same block must not verify against the plain (non-gathered) instance.
    assert!(!verify(&p, &ph, &b.c, &b.score, None));
}

#[test]
fn dataset_reference_is_data_dependent() {
    let ds = build_dataset(&[3u8; 32], 64);
    for u in 1..64 {
        let r = dataset_ref(&ds[u - 1], u);
        assert!(r < u);
    }
    // Changing the content of block u-1 changes ref(u) for some u: the reference is read from data.
    let mut tweaked = ds[10];
    tweaked[0] ^= 1;
    let differs = (11..64).any(|u| dataset_ref(&ds[10], u) != dataset_ref(&tweaked, u));
    assert!(differs);
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
        bits: 4,
        nonce: 0,
        c: vec![0u64; 16],
        score: [0u8; 32],
        id: [0u8; 32],
    };
    assert_eq!(chain.try_append(bad), Err(Reject::Prev));
}

#[test]
fn rejects_block_below_required_difficulty() {
    let mut chain = Chain::new(profile(10), [11u8; 32], 1);
    assert_eq!(chain.next_bits(), 10);
    let easy = block_at(&chain, 1, 10);
    assert_eq!(chain.try_append(easy), Err(Reject::Bits));
    let ok = block_at(&chain, 10, 10);
    assert!(chain.append_checked(ok));
}

#[test]
fn rejects_claimed_bits_above_required() {
    // Claiming more work than required (e.g. the achieved lead) is rejected: work counts the target.
    let chain = Chain::new(profile(2), [12u8; 32], 1);
    let blk = block_at(&chain, 3, 10);
    assert!(score_lead(&blk.score) >= 3);
    assert_eq!(chain.clone().try_append(blk), Err(Reject::Bits));
}

#[test]
fn bits_are_committed_in_the_block_id() {
    let mut a = Chain::new(profile(2), [13u8; 32], 1);
    let blk = a.mine_next(10, 100_000).unwrap();
    let mut forged = blk.clone();
    forged.bits = 3; // a relay rewrites the difficulty
    let mut b = Chain::new(profile(3), [13u8; 32], 1); // even where 3 bits would be required
    assert_eq!(b.try_append(forged), Err(Reject::Id));
}

#[test]
fn rejects_timestamp_not_above_median_time_past() {
    let mut chain = Chain::new(profile(2), [14u8; 32], 1);
    for i in 0..MTP_WINDOW as u64 {
        chain.mine_next(1000 + i * 10, 100_000).unwrap();
    }
    let mtp = chain.median_time_past();
    let stale = block_at(&chain, chain.next_bits(), mtp);
    assert_eq!(chain.clone().try_append(stale), Err(Reject::Timestamp));
    let fresh = block_at(&chain, chain.next_bits(), mtp + 1);
    assert!(chain.append_checked(fresh));
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
    assert_eq!(b.cumulative_work(), 2 * 16);
}

#[test]
fn rejects_wrong_height() {
    let mut chain = Chain::new(profile(2), [15u8; 32], 1);
    chain.mine_next(10, 100_000).unwrap();
    let mut blk = block_at(&chain, chain.next_bits(), 20);
    blk.height += 1;
    assert_eq!(chain.try_append(blk), Err(Reject::Height));
}

#[test]
fn rejects_wrong_product_and_wrong_score_as_pow() {
    let mut chain = Chain::new(profile(2), [16u8; 32], 1);
    let good = block_at(&chain, chain.next_bits(), 10);
    let ph = preheader(&chain.chain_id, 1, 0, &good.prev, 10, good.bits, good.nonce);

    // A different C with a consistent id and score: only the PoW check can reject it.
    let mut bad_c = good.clone();
    bad_c.c[0] = (bad_c.c[0] + 1) % P;
    bad_c.score = score(&ph, &bad_c.c);
    bad_c.id = block_id(&ph, &bad_c.c);
    assert_eq!(chain.clone().try_append(bad_c), Err(Reject::Pow));

    // A claimed score that is not the hash of (preheader, C).
    let mut bad_score = good.clone();
    bad_score.score = [0u8; 32];
    assert_eq!(chain.clone().try_append(bad_score), Err(Reject::Pow));

    // A non-canonical entry is rejected before any hashing.
    let mut noncanonical = good.clone();
    noncanonical.c[0] = P;
    assert_eq!(chain.clone().try_append(noncanonical), Err(Reject::Pow));

    assert!(chain.append_checked(good));
}

#[test]
fn retarget_boundary_uses_window_minus_one_intervals() {
    // 16 blocks spaced exactly at the target: 15 intervals of 10 s = the expected 150 s, no change.
    let mut on_target = Chain::new(profile(3), [17u8; 32], 1);
    for i in 0..16u64 {
        on_target.mine_next(1000 + i * 10, 1_000_000).unwrap();
    }
    assert_eq!(on_target.next_bits(), 3);
}
