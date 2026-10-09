use abacus_chain::{block_id, mine, preheader, score, verify, Chain, Profile};
use abacus_verifier::P;

fn profile() -> Profile {
    let mut target = [0u8; 32];
    target[0] = 0x40; // ~1/4 attempts
    Profile { n: 4, k: 4, target }
}

fn mine_one() -> (Profile, Vec<u8>, Vec<u64>, [u8; 32]) {
    let p = profile();
    let chain = [7u8; 32];
    let prev = [0u8; 32];
    let (nonce, c, sc) = mine(&p, &chain, 1, 0, &prev, 0, 100_000).expect("mined");
    let ph = preheader(&chain, 1, 0, &prev, nonce);
    (p, ph, c, sc)
}

#[test]
fn mines_and_verifies() {
    let (p, ph, c, sc) = mine_one();
    assert!(verify(&p, &ph, &c, &sc));
}

#[test]
fn rejects_tampered_product() {
    let (p, ph, mut c, sc) = mine_one();
    c[0] = (c[0] + 1) % P;
    assert!(!verify(&p, &ph, &c, &sc));
}

#[test]
fn rejects_wrong_score() {
    let (p, ph, c, mut sc) = mine_one();
    sc[0] ^= 0xFF;
    assert!(!verify(&p, &ph, &c, &sc));
}

#[test]
fn chain_appends_and_rejects_bad_prev() {
    let p = profile();
    let mut chain = Chain::new(p, [1u8; 32], [0u8; 32]);
    for _ in 0..3 {
        assert!(chain.mine_next(1, 100_000).is_some());
    }
    assert_eq!(chain.height(), 3);

    // A block with the wrong prev must be rejected.
    let bad = abacus_chain::Block {
        height: chain.height(),
        prev: [9u8; 32],
        nonce: 0,
        c: vec![0u64; p.n * p.n],
        score: [0u8; 32],
        id: [0u8; 32],
    };
    assert!(!chain.append_checked(bad));
}

#[test]
fn block_id_and_score_are_deterministic() {
    let (_, ph, c, sc) = mine_one();
    assert_eq!(score(&ph, &c), sc);
    assert_eq!(block_id(&ph, &c), block_id(&ph, &c));
}
