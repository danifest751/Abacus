//! Minimal local prototype of candidate A: header-bound Freivalds matmul PoW.
//!
//! A block commits a preheader (chain id, version, prev id, height, nonce, timestamp) and the product
//! `C`. The instance `A, B` and the score hash derive from the preheader; the product is verified with
//! `k` Fiat-Shamir-bound Freivalds challenges (ADR 0004). Difficulty is a count of required leading
//! zero bits of the score; it is retargeted every window from in-block timestamps (bounded). Fork
//! choice selects the greatest cumulative work. No networking, mempool or monetary system.

use abacus_verifier::{freivalds_fs::verify_fs, matmul, sha256::sha256, P};

pub mod p2p;

pub const DOM_INSTANCE: &[u8] = b"abacus/instance";
pub const DOM_SCORE: &[u8] = b"abacus/score";
pub const DOM_BLOCK: &[u8] = b"abacus/block";
pub const DOM_EXPAND: &[u8] = b"abacus/expand";
pub const DOM_PH: &[u8] = b"abacus/ph";

pub const RETARGET_WINDOW: u64 = 16;
pub const TARGET_SPACING: u64 = 10; // seconds per block (prototype)

#[derive(Clone, Copy)]
pub struct Profile {
    pub n: usize,
    pub k: usize,
    pub bits: u32, // required leading zero bits of the score
}

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

pub fn preheader(
    chain_id: &[u8; 32],
    version: u32,
    height: u64,
    prev: &[u8; 32],
    timestamp: u64,
    nonce: u64,
) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(DOM_PH);
    v.extend_from_slice(chain_id);
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(prev);
    v.extend_from_slice(&height.to_le_bytes());
    v.extend_from_slice(&timestamp.to_le_bytes());
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

// ---- candidate A': memory-hard epoch dataset and gathered instance (spec/04) ----

pub const DOM_DS: &[u8] = b"abacus/ds";
pub const DOM_REF: &[u8] = b"abacus/ref";

/// Sequential, data-dependent dataset: block `u` depends on `u-1` and a data-dependent earlier block,
/// so a random block is not cheaply recomputable (a miner must store the dataset).
pub fn build_dataset(epoch_seed: &[u8; 32], nblocks: usize) -> Vec<[u8; 32]> {
    let mut blocks: Vec<[u8; 32]> = Vec::with_capacity(nblocks);
    let mut m0 = Vec::new();
    m0.extend_from_slice(DOM_DS);
    m0.extend_from_slice(epoch_seed);
    m0.extend_from_slice(&0u64.to_le_bytes());
    blocks.push(sha256(&m0));
    for u in 1..nblocks {
        let mut mr = Vec::new();
        mr.extend_from_slice(DOM_REF);
        mr.extend_from_slice(epoch_seed);
        mr.extend_from_slice(&(u as u64).to_le_bytes());
        let mut e = [0u8; 8];
        e.copy_from_slice(&sha256(&mr)[0..8]);
        let r = (u64::from_le_bytes(e) % u as u64) as usize;
        let mut m = Vec::new();
        m.extend_from_slice(DOM_DS);
        m.extend_from_slice(epoch_seed);
        m.extend_from_slice(&(u as u64).to_le_bytes());
        m.extend_from_slice(&blocks[u - 1]);
        m.extend_from_slice(&blocks[r]);
        blocks.push(sha256(&m));
    }
    blocks
}

pub fn field_from_block(block: &[u8; 32]) -> u64 {
    let mut e = [0u8; 8];
    e.copy_from_slice(&block[0..8]);
    u64::from_le_bytes(e) % P
}

pub fn expand_indices(seed: &[u8], count: usize, nblocks: usize) -> Vec<usize> {
    expand(seed, count).into_iter().map(|x| (x as usize) % nblocks).collect()
}

/// Gathered instance: `A` and `B` are assembled from header-random dataset blocks.
pub fn instance_hard(ph: &[u8], n: usize, dataset: &[[u8; 32]]) -> (Vec<u64>, Vec<u64>) {
    let mut m = Vec::new();
    m.extend_from_slice(DOM_INSTANCE);
    m.extend_from_slice(ph);
    let seed = sha256(&m);
    let total = n * n;
    let idx = expand_indices(&seed, 2 * total, dataset.len());
    let a = (0..total).map(|i| field_from_block(&dataset[idx[i]])).collect();
    let b = (total..2 * total).map(|i| field_from_block(&dataset[idx[i]])).collect();
    (a, b)
}

/// Instance used by the PoW: gathered when a dataset is present, else expanded from the seed.
pub fn instance_with(ph: &[u8], n: usize, dataset: Option<&[[u8; 32]]>) -> (Vec<u64>, Vec<u64>) {
    match dataset {
        Some(ds) => instance_hard(ph, n, ds),
        None => instance(ph, n),
    }
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

/// Number of leading zero bits of a 32-byte (big-endian) score.
pub fn score_lead(score: &[u8; 32]) -> u32 {
    let mut lead = 0u32;
    for &b in score.iter() {
        if b == 0 {
            lead += 8;
        } else {
            lead += b.leading_zeros();
            break;
        }
    }
    lead
}

pub fn accept(score: &[u8; 32], bits: u32) -> bool {
    score_lead(score) >= bits
}

pub fn mine(
    profile: &Profile,
    chain_id: &[u8; 32],
    version: u32,
    height: u64,
    prev: &[u8; 32],
    timestamp: u64,
    max_attempts: u64,
    dataset: Option<&[[u8; 32]]>,
) -> Option<(u64, Vec<u64>, [u8; 32])> {
    for nonce in 0..max_attempts {
        let ph = preheader(chain_id, version, height, prev, timestamp, nonce);
        let (a, b) = instance_with(&ph, profile.n, dataset);
        let c = matmul(&a, &b, profile.n);
        let sc = score(&ph, &c);
        if accept(&sc, profile.bits) {
            return Some((nonce, c, sc));
        }
    }
    None
}

pub fn verify(profile: &Profile, ph: &[u8], c: &[u64], sc: &[u8; 32], dataset: Option<&[[u8; 32]]>) -> bool {
    if !accept(sc, profile.bits) {
        return false;
    }
    if &score(ph, c) != sc {
        return false;
    }
    let (a, b) = instance_with(ph, profile.n, dataset);
    verify_fs(&a, &b, c, profile.n, profile.k)
}

#[derive(Clone)]
pub struct Block {
    pub height: u64,
    pub prev: [u8; 32],
    pub timestamp: u64,
    pub nonce: u64,
    pub c: Vec<u64>,
    pub score: [u8; 32],
    pub id: [u8; 32],
    pub bits: u32,
}

#[derive(Clone)]
pub struct Chain {
    pub profile: Profile,
    pub chain_id: [u8; 32],
    pub version: u32,
    pub blocks: Vec<Block>,
    pub dataset: Option<Vec<[u8; 32]>>,
}

impl Chain {
    pub fn new(profile: Profile, chain_id: [u8; 32], version: u32) -> Self {
        Chain { profile, chain_id, version, blocks: Vec::new(), dataset: None }
    }

    /// Attach a memory-hard epoch dataset (candidate A'); the instance becomes gathered.
    pub fn with_dataset(mut self, dataset: Vec<[u8; 32]>) -> Self {
        self.dataset = Some(dataset);
        self
    }

    pub fn tip(&self) -> [u8; 32] {
        self.blocks.last().map(|b| b.id).unwrap_or([0u8; 32])
    }

    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }

    /// Difficulty (bits) to use for the next block: retarget every window from timestamps.
    pub fn next_bits(&self) -> u32 {
        let h = self.height();
        let cur = self.blocks.last().map(|b| b.bits).unwrap_or(self.profile.bits);
        if h == 0 || h % RETARGET_WINDOW != 0 {
            return cur;
        }
        let first = self.blocks[self.blocks.len() - RETARGET_WINDOW as usize].timestamp;
        let last = self.blocks.last().unwrap().timestamp;
        let elapsed = last.saturating_sub(first).max(1);
        let expected = RETARGET_WINDOW * TARGET_SPACING;
        let mut bits = cur;
        if elapsed < expected / 2 {
            bits = bits.saturating_add(1); // too fast -> harder
        } else if elapsed > expected * 2 {
            bits = bits.saturating_sub(1); // too slow -> easier
        }
        bits.clamp(1, 255)
    }

    /// Cumulative work = sum of 2^bits (expected attempts), as f64 (bits <= 255).
    pub fn cumulative_work(&self) -> f64 {
        self.blocks.iter().map(|b| 2f64.powi(b.bits as i32)).sum()
    }

    pub fn mine_next(&mut self, timestamp: u64, max_attempts: u64) -> Option<Block> {
        let bits = self.next_bits();
        let height = self.height();
        let prev = self.tip();
        let p = Profile { n: self.profile.n, k: self.profile.k, bits };
        let (nonce, c, sc) = mine(&p, &self.chain_id, self.version, height, &prev, timestamp, max_attempts, self.dataset.as_deref())?;
        let ph = preheader(&self.chain_id, self.version, height, &prev, timestamp, nonce);
        let id = block_id(&ph, &c);
        let block = Block { height, prev, timestamp, nonce, c, score: sc, id, bits };
        self.blocks.push(block.clone());
        Some(block)
    }

    pub fn append_checked(&mut self, block: Block) -> bool {
        if block.height != self.height() || block.prev != self.tip() {
            return false;
        }
        let ph = preheader(&self.chain_id, self.version, block.height, &block.prev, block.timestamp, block.nonce);
        if block_id(&ph, &block.c) != block.id {
            return false;
        }
        let p = Profile { n: self.profile.n, k: self.profile.k, bits: block.bits };
        if !verify(&p, &ph, &block.c, &block.score, self.dataset.as_deref()) {
            return false;
        }
        self.blocks.push(block);
        true
    }
}

/// Fork choice: return the chain with strictly greater cumulative work (ties -> `a`).
pub fn best_chain<'a>(a: &'a Chain, b: &'a Chain) -> &'a Chain {
    if b.cumulative_work() > a.cumulative_work() {
        b
    } else {
        a
    }
}
