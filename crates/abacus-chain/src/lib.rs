//! Minimal local prototype of candidate A: header-bound Freivalds matmul PoW.
//!
//! A block commits a preheader (chain id, version, prev id, height, timestamp, difficulty bits,
//! nonce) and the product `C`. The instance `A, B` and the score hash derive from the preheader; the
//! product is verified with `k` Fiat-Shamir challenges bound to `(preheader, C)` (ADR 0004, 0009).
//! Difficulty is a count of required leading zero bits of the score; it is **ancestor-derived**
//! (`next_bits`) and enforced on every appended block, retargeted every window from in-block
//! timestamps, which must exceed the median of the previous `MTP_WINDOW` blocks. Fork choice selects
//! the greatest cumulative work, counted from the enforced target, not from the achieved score.
//! No mempool, transactions or monetary system.

use abacus_verifier::{freivalds_fs::verify_fs, matmul, sha256::sha256, P};
use std::sync::Arc;

pub mod p2p;
pub mod tnet;

pub const DOM_INSTANCE: &[u8] = b"abacus/instance";
pub const DOM_SCORE: &[u8] = b"abacus/score";
pub const DOM_BLOCK: &[u8] = b"abacus/block";
pub const DOM_EXPAND: &[u8] = b"abacus/expand";
pub const DOM_PH: &[u8] = b"abacus/ph";

pub const RETARGET_WINDOW: u64 = 16;
pub const TARGET_SPACING: u64 = 10; // seconds per block (prototype)
/// Timestamps must be strictly greater than the median of the last `MTP_WINDOW` blocks.
pub const MTP_WINDOW: usize = 11;
/// Upper bound on difficulty bits. Cumulative work `sum 2^bits` is exact in a `u128` for up to `2^8`
/// blocks at the maximum and saturates beyond (never overflows).
pub const MAX_BITS: u32 = 120;

/// `C` has exactly `n^2` entries, each a canonical residue `< P`. Checked before any hashing.
pub fn c_is_canonical(c: &[u64], n: usize) -> bool {
    n.checked_mul(n) == Some(c.len()) && c.iter().all(|&x| x < P)
}

#[derive(Clone, Copy)]
pub struct Profile {
    pub n: usize,
    pub k: usize,
    pub bits: u32, // required leading zero bits of the score (genesis difficulty for a chain)
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

/// Preheader encoding (v2): the difficulty `bits` is committed, so it cannot be changed without
/// changing the instance, the score and the block id.
pub fn preheader(
    chain_id: &[u8; 32],
    version: u32,
    height: u64,
    prev: &[u8; 32],
    timestamp: u64,
    bits: u32,
    nonce: u64,
) -> Vec<u8> {
    let mut v = Vec::with_capacity(DOM_PH.len() + 32 + 4 + 32 + 8 + 8 + 4 + 8);
    v.extend_from_slice(DOM_PH);
    v.extend_from_slice(chain_id);
    v.extend_from_slice(&version.to_le_bytes());
    v.extend_from_slice(prev);
    v.extend_from_slice(&height.to_le_bytes());
    v.extend_from_slice(&timestamp.to_le_bytes());
    v.extend_from_slice(&bits.to_le_bytes());
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

// ---- candidate A8: int8 instance, exact int32 product (spec/05, ADR 0012) ----

pub const DOM_INSTANCE_I8: &[u8] = b"abacus/instance-i8";
pub const DOM_SCORE_I8: &[u8] = b"abacus/score-i8";

/// SHA-256 counter-mode byte stream: `SHA256("abacus/expand" || seed || LE32(i))`, 32 bytes per hash.
pub fn expand_bytes(seed: &[u8], count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(count + 32);
    let mut c: u32 = 0;
    while out.len() < count {
        let mut m = Vec::with_capacity(DOM_EXPAND.len() + seed.len() + 4);
        m.extend_from_slice(DOM_EXPAND);
        m.extend_from_slice(seed);
        m.extend_from_slice(&c.to_le_bytes());
        out.extend_from_slice(&sha256(&m));
        c += 1;
    }
    out.truncate(count);
    out
}

/// int8 instance: `seed = SHA256("abacus/instance-i8" || preheader)`; `2 n^2` bytes from `expand_bytes`
/// read as two's-complement `i8`, the first `n^2` are `A`, the next `n^2` are `B` (row-major).
pub fn instance_i8(ph: &[u8], n: usize) -> (Vec<i8>, Vec<i8>) {
    let mut m = Vec::with_capacity(DOM_INSTANCE_I8.len() + ph.len());
    m.extend_from_slice(DOM_INSTANCE_I8);
    m.extend_from_slice(ph);
    let seed = sha256(&m);
    let bytes = expand_bytes(&seed, 2 * n * n);
    let v: Vec<i8> = bytes.into_iter().map(|x| x as i8).collect();
    (v[..n * n].to_vec(), v[n * n..].to_vec())
}

/// `SHA256("abacus/score-i8" || preheader || C)` with `C` as little-endian `i32`.
pub fn score_i8(ph: &[u8], c: &[i32]) -> [u8; 32] {
    let mut m = Vec::with_capacity(DOM_SCORE_I8.len() + ph.len() + c.len() * 4);
    m.extend_from_slice(DOM_SCORE_I8);
    m.extend_from_slice(ph);
    m.extend_from_slice(&abacus_verifier::int8::encode_c_i32(c));
    sha256(&m)
}

/// Stateless PoW check of an A8 block: shape, target, score and Fiat–Shamir Freivalds over `F_P`.
pub fn verify_i8(profile: &Profile, ph: &[u8], c: &[i32], sc: &[u8; 32]) -> bool {
    use abacus_verifier::int8::{verify_fs_i8, MAX_N};
    if profile.bits > MAX_BITS || profile.n > MAX_N || profile.n.checked_mul(profile.n) != Some(c.len()) {
        return false;
    }
    if !accept(sc, profile.bits) || &score_i8(ph, c) != sc {
        return false;
    }
    let (a, b) = instance_i8(ph, profile.n);
    verify_fs_i8(ph, &a, &b, c, profile.n, profile.k)
}

// ---- candidate A': memory-hard epoch dataset and gathered instance (spec/04) ----

pub const DOM_DS: &[u8] = b"abacus/ds";

/// Data-dependent reference of block `u >= 1`: `LE64(blocks[u-1][0..8]) mod u` (Argon2d-style).
pub fn dataset_ref(prev_block: &[u8; 32], u: usize) -> usize {
    let mut e = [0u8; 8];
    e.copy_from_slice(&prev_block[0..8]);
    (u64::from_le_bytes(e) % u as u64) as usize
}

/// Sequential, data-dependent dataset: block `u` depends on `u-1` and on an earlier block whose
/// index is read from `u-1`'s content, so the access pattern is not known before the data is.
/// Only 8 of the 32 bytes of a block are consumed by the gather (`field_from_block`), so a miner
/// needs `8 * nblocks` bytes of storage, not `32 * nblocks` (spec/04 §2).
pub fn build_dataset(epoch_seed: &[u8; 32], nblocks: usize) -> Vec<[u8; 32]> {
    let mut blocks: Vec<[u8; 32]> = Vec::with_capacity(nblocks);
    if nblocks == 0 {
        return blocks;
    }
    let mut m0 = Vec::new();
    m0.extend_from_slice(DOM_DS);
    m0.extend_from_slice(epoch_seed);
    m0.extend_from_slice(&0u64.to_le_bytes());
    blocks.push(sha256(&m0));
    for u in 1..nblocks {
        let r = dataset_ref(&blocks[u - 1], u);
        let mut m = Vec::with_capacity(DOM_DS.len() + 32 + 8 + 64);
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
    expand(seed, count).into_iter().map(|x| (x % nblocks as u64) as usize).collect()
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
        Some(ds) if !ds.is_empty() => instance_hard(ph, n, ds),
        _ => instance(ph, n),
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

/// Search nonces `start .. start + max_attempts` (wrapping is not allowed) at `profile.bits`.
#[allow(clippy::too_many_arguments)]
pub fn mine_from(
    profile: &Profile,
    chain_id: &[u8; 32],
    version: u32,
    height: u64,
    prev: &[u8; 32],
    timestamp: u64,
    start: u64,
    max_attempts: u64,
    dataset: Option<&[[u8; 32]]>,
) -> Option<(u64, Vec<u64>, [u8; 32])> {
    let end = start.checked_add(max_attempts)?;
    for nonce in start..end {
        let ph = preheader(chain_id, version, height, prev, timestamp, profile.bits, nonce);
        let (a, b) = instance_with(&ph, profile.n, dataset);
        let c = matmul(&a, &b, profile.n);
        let sc = score(&ph, &c);
        if accept(&sc, profile.bits) {
            return Some((nonce, c, sc));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
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
    mine_from(profile, chain_id, version, height, prev, timestamp, 0, max_attempts, dataset)
}

/// Stateless PoW check of one block: shape and canonical encoding of `C`, target, score, and the
/// Fiat–Shamir Freivalds check. `profile.bits` is the **required** difficulty. Never panics on
/// malformed input.
pub fn verify(profile: &Profile, ph: &[u8], c: &[u64], sc: &[u8; 32], dataset: Option<&[[u8; 32]]>) -> bool {
    if profile.bits > MAX_BITS || !c_is_canonical(c, profile.n) {
        return false;
    }
    if !accept(sc, profile.bits) {
        return false;
    }
    if &score(ph, c) != sc {
        return false;
    }
    let (a, b) = instance_with(ph, profile.n, dataset);
    verify_fs(ph, &a, &b, c, profile.n, profile.k)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub height: u64,
    pub prev: [u8; 32],
    pub timestamp: u64,
    pub bits: u32,
    pub nonce: u64,
    pub c: Vec<u64>,
    pub score: [u8; 32],
    pub id: [u8; 32],
}

/// Why `append_checked` refused a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Height,
    Prev,
    Bits,
    Timestamp,
    Id,
    Pow,
}

/// What a miner needs to build the next block.
#[derive(Clone)]
pub struct Template {
    pub height: u64,
    pub prev: [u8; 32],
    pub bits: u32,
    /// Smallest timestamp the next block may carry (`median_time_past + 1`).
    pub min_timestamp: u64,
}

#[derive(Clone)]
pub struct Chain {
    pub profile: Profile,
    pub chain_id: [u8; 32],
    pub version: u32,
    pub blocks: Vec<Block>,
    /// Shared, immutable epoch dataset (candidate A'); cloning a chain does not copy it.
    pub dataset: Option<Arc<Vec<[u8; 32]>>>,
}

impl Chain {
    pub fn new(profile: Profile, chain_id: [u8; 32], version: u32) -> Self {
        Chain { profile, chain_id, version, blocks: Vec::new(), dataset: None }
    }

    /// Attach a memory-hard epoch dataset (candidate A'); the instance becomes gathered.
    pub fn with_dataset(mut self, dataset: Vec<[u8; 32]>) -> Self {
        self.dataset = Some(Arc::new(dataset));
        self
    }

    /// An empty chain with the same parameters and dataset (used to validate a peer's chain).
    pub fn empty_like(&self) -> Self {
        Chain {
            profile: self.profile,
            chain_id: self.chain_id,
            version: self.version,
            blocks: Vec::new(),
            dataset: self.dataset.clone(),
        }
    }

    pub fn dataset_slice(&self) -> Option<&[[u8; 32]]> {
        self.dataset.as_deref().map(|v| v.as_slice())
    }

    pub fn tip(&self) -> [u8; 32] {
        self.blocks.last().map(|b| b.id).unwrap_or([0u8; 32])
    }

    pub fn height(&self) -> u64 {
        self.blocks.len() as u64
    }

    /// Median timestamp of the last `MTP_WINDOW` blocks (0 for an empty chain).
    pub fn median_time_past(&self) -> u64 {
        if self.blocks.is_empty() {
            return 0;
        }
        let from = self.blocks.len().saturating_sub(MTP_WINDOW);
        let mut ts: Vec<u64> = self.blocks[from..].iter().map(|b| b.timestamp).collect();
        ts.sort_unstable();
        ts[ts.len() / 2]
    }

    /// Difficulty (bits) required for the next block. Every `RETARGET_WINDOW` blocks, the time between
    /// the first and the last block of the window (`RETARGET_WINDOW - 1` intervals) is compared with
    /// `(RETARGET_WINDOW - 1) * TARGET_SPACING`: +1 bit below half of it, -1 bit above twice.
    pub fn next_bits(&self) -> u32 {
        let h = self.height();
        let cur = self.blocks.last().map(|b| b.bits).unwrap_or(self.profile.bits);
        if h == 0 || !h.is_multiple_of(RETARGET_WINDOW) {
            return cur;
        }
        let first = self.blocks[self.blocks.len() - RETARGET_WINDOW as usize].timestamp;
        let last = self.blocks.last().unwrap().timestamp;
        let elapsed = last.saturating_sub(first).max(1);
        let expected = (RETARGET_WINDOW - 1) * TARGET_SPACING;
        let mut bits = cur;
        if elapsed < expected / 2 {
            bits = bits.saturating_add(1); // too fast -> harder
        } else if elapsed > expected * 2 {
            bits = bits.saturating_sub(1); // too slow -> easier
        }
        bits.clamp(1, MAX_BITS)
    }

    pub fn template(&self) -> Template {
        Template {
            height: self.height(),
            prev: self.tip(),
            bits: self.next_bits(),
            min_timestamp: if self.blocks.is_empty() { 0 } else { self.median_time_past() + 1 },
        }
    }

    /// Cumulative work = sum of `2^bits` over the **required** difficulty of each block.
    pub fn cumulative_work(&self) -> u128 {
        self.blocks.iter().fold(0u128, |acc, b| acc.saturating_add(1u128 << b.bits.min(MAX_BITS)))
    }

    /// Mine the next block locally at the required difficulty. `timestamp` is raised to the
    /// minimum allowed value if needed. The block goes through `append_checked`.
    pub fn mine_next(&mut self, timestamp: u64, max_attempts: u64) -> Option<Block> {
        let t = self.template();
        let timestamp = timestamp.max(t.min_timestamp);
        let p = Profile { n: self.profile.n, k: self.profile.k, bits: t.bits };
        let (nonce, c, sc) =
            mine(&p, &self.chain_id, self.version, t.height, &t.prev, timestamp, max_attempts, self.dataset_slice())?;
        let ph = preheader(&self.chain_id, self.version, t.height, &t.prev, timestamp, t.bits, nonce);
        let id = block_id(&ph, &c);
        let block = Block { height: t.height, prev: t.prev, timestamp, bits: t.bits, nonce, c, score: sc, id };
        self.try_append(block.clone()).ok()?;
        Some(block)
    }

    /// Validate `block` as the next block and append it.
    pub fn try_append(&mut self, block: Block) -> Result<(), Reject> {
        if block.height != self.height() {
            return Err(Reject::Height);
        }
        if block.prev != self.tip() {
            return Err(Reject::Prev);
        }
        if block.bits != self.next_bits() {
            return Err(Reject::Bits);
        }
        if !self.blocks.is_empty() && block.timestamp <= self.median_time_past() {
            return Err(Reject::Timestamp);
        }
        let ph = preheader(
            &self.chain_id,
            self.version,
            block.height,
            &block.prev,
            block.timestamp,
            block.bits,
            block.nonce,
        );
        let p = Profile { n: self.profile.n, k: self.profile.k, bits: block.bits };
        // Shape and canonical encoding of a peer-supplied C before anything is hashed.
        if !c_is_canonical(&block.c, p.n) {
            return Err(Reject::Pow);
        }
        if block_id(&ph, &block.c) != block.id {
            return Err(Reject::Id);
        }
        if !verify(&p, &ph, &block.c, &block.score, self.dataset_slice()) {
            return Err(Reject::Pow);
        }
        self.blocks.push(block);
        Ok(())
    }

    pub fn append_checked(&mut self, block: Block) -> bool {
        self.try_append(block).is_ok()
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
