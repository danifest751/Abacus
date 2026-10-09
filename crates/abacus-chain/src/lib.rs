//! Minimal local prototype of candidate A: header-bound Freivalds matmul PoW.
//!
//! A block commits a preheader (chain id, version, prev id, height, nonce) and the product `C`. The
//! instance `A, B` and the score hash derive from the preheader; the product is verified with `k`
//! Fiat-Shamir-bound Freivalds challenges (ADR 0004). No networking, mempool or monetary system.

use abacus_verifier::{freivalds_fs::verify_fs, matmul, sha256::sha256, P};

pub const DOM_INSTANCE: &[u8] = b"abacus/instance";
pub const DOM_SCORE: &[u8] = b"abacus/score";
pub const DOM_BLOCK: &[u8] = b"abacus/block";
pub const DOM_EXPAND: &[u8] = b"abacus/expand";
pub const DOM_PH: &[u8] = b"abacus/ph";

#[derive(Clone, Copy)]
pub struct Profile {
    pub n: usize,
    pub k: usize,
    pub target: [u8; 32],
}

/// Expand a seed into `count` field elements via SHA-256 counter mode.
pub fn expand(seed: &[u8], count: usize) -> Vec<u64> {
    let mut out = Vec::with_capacity(count);
    let mut c: u32 = 0;
    while out.len() < count {
        let mut m = Vec::new();
        m.extend_from_slice(DOM_EXPAND);
        m.extend_from_slice(seed);
        m.extend_from_slice(&c.to_le_bytes());
        let h = sha256(&m);
        for off in [0usize, 8, 16, 24] {
            let mut e = [0u8; 8];
            e.copy_from_slice(&h[off..off + 8]);
            out.push(u64::from_le_bytes(e) % P);
        }
        c += 1;
    }
    out.truncate(count);
    out
}

pub fn preheader(chain_id: &[u8; 32], version: u32, height: u64, prev: &[u8; 32], nonce: u64) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(DOM_PH);
    v.extend_from_slice(chain_id);
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(prev);
    v.extend_from_slice(&height.to_le_bytes());
    v.extend_from_slice(&nonce.to_le_bytes());
    v
}

pub fn instance(ph: &[u8], n: usize) -> (Vec<u64>, Vec<u64>) {
    let mut m = Vec::new();
    m.extend_from_slice(DOM_INSTANCE);
    m.extend_from_slice(ph);
    let seed = sha256(&m);
    let v = expand(&seed, 2 * n * n);
    (v[0..n * n].to_vec(), v[n * n..2 * n * n].to_vec())
}

pub fn encode_c(c: &[u64]) -> Vec<u8> {
    let mut v = Vec::with_capacity(c.len() * 8);
    for &x in c {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

pub fn score(ph: &[u8], c: &[u64]) -> [u8; 32] {
    let mut m = Vec::new();
    m.extend_from_slice(DOM_SCORE);
    m.extend_from_slice(ph);
    m.extend_from_slice(&encode_c(c));
    sha256(&m)
}

pub fn block_id(ph: &[u8], c: &[u64]) -> [u8; 32] {
    let mut m = Vec::new();
    m.extend_from_slice(DOM_BLOCK);
    m.extend_from_slice(ph);
    m.extend_from_slice(&encode_c(c));
    sha256(&m)
}

/// Lexicographic (== numeric big-endian) comparison.
pub fn le_less(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in 0..32 {
        if a[i] != b[i] {
            return a[i] < b[i];
        }
    }
    false
}

/// Mine one block: find a nonce whose score is below the target.
pub fn mine(
    profile: &Profile,
    chain_id: &[u8; 32],
    version: u32,
    height: u64,
    prev: &[u8; 32],
    nonce_start: u64,
    max_attempts: u64,
) -> Option<(u64, Vec<u64>, [u8; 32])> {
    for nonce in nonce_start..nonce_start.saturating_add(max_attempts) {
        let ph = preheader(chain_id, version, height, prev, nonce);
        let (a, b) = instance(&ph, profile.n);
        let c = matmul(&a, &b, profile.n);
        let sc = score(&ph, &c);
        if le_less(&sc, &profile.target) {
            return Some((nonce, c, sc));
        }
    }
    None
}

/// Verify a block: score below target, score matches, and Freivalds accepts `C`.
pub fn verify(profile: &Profile, ph: &[u8], c: &[u64], sc: &[u8; 32]) -> bool {
    if !le_less(sc, &profile.target) {
        return false;
    }
    if &score(ph, c) != sc {
        return false;
    }
    let (a, b) = instance(ph, profile.n);
    verify_fs(&a, &b, c, profile.n, profile.k)
}

#[derive(Clone)]
pub struct Block {
    pub height: u64,
    pub prev: [u8; 32],
    pub nonce: u64,
    pub c: Vec<u64>,
    pub score: [u8; 32],
    pub id: [u8; 32],
}

pub struct Chain {
    pub profile: Profile,
    pub chain_id: [u8; 32],
    pub blocks: Vec<Block>,
}

impl Chain {
    pub fn new(profile: Profile, chain_id: [u8; 32], _genesis_id: [u8; 32]) -> Self {
        Chain { profile, chain_id, blocks: Vec::new() }
    }

    pub fn tip(&self) -> [u8; 32] {
        self.blocks.last().map(|b| b.id).unwrap_or([0u8; 32])
    }

    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }

    /// Mine and append the next block. Returns the block.
    pub fn mine_next(&mut self, version: u32, max_attempts: u64) -> Option<&Block> {
        let height = self.height();
        let prev = self.tip();
        let (nonce, c, sc) = mine(&self.profile, &self.chain_id, version, height, &prev, 0, max_attempts)?;
        let ph = preheader(&self.chain_id, version, height, &prev, nonce);
        let id = block_id(&ph, &c);
        self.blocks.push(Block { height, prev, nonce, c, score: sc, id });
        self.blocks.last()
    }

    /// Validate that `block` extends the current tip with a valid PoW.
    pub fn append_checked(&mut self, block: Block) -> bool {
        if block.height != self.height() || block.prev != self.tip() {
            return false;
        }
        let ph = preheader(&self.chain_id, 1, block.height, &block.prev, block.nonce);
        if block_id(&ph, &block.c) != block.id {
            return false;
        }
        if !verify(&self.profile, &ph, &block.c, &block.score) {
            return false;
        }
        self.blocks.push(block);
        true
    }

    /// Expected attempts per block from the top byte of the target (approx., for a small target).
    pub fn expected_attempts(&self) -> f64 {
        let t = &self.profile.target;
        let mut lead = 0u64;
        for &b in t.iter() {
            if b == 0 {
                lead += 8;
            } else {
                lead += b.leading_zeros() as u64;
                break;
            }
        }
        2f64.powi(lead as i32)
    }
}
