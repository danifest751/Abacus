//! Interactive proof of tensor throughput (spec/06, ADR 0014).
//!
//! A verifier challenges a prover to compute `m` exact int8 products `C_j = A_j * B_j` (`n x n`, int32
//! result) from a fresh seed within a deadline, and to commit to every row of every `C_j` with a Merkle
//! root. Only after the root arrives does the verifier pick `k` random rows; the prover opens them and
//! the verifier checks each opened row with a secret Freivalds vector in `O(n)`:
//!
//! ```text
//! C_j[i, :] . r  ==  A_j[i, :] . (B_j r)      (mod P, Goldilocks)
//! ```
//!
//! `v_j = B_j r` is precomputed by the verifier before the challenge (`O(n^2)` per product), so online
//! verification costs `O(k n)` plus `k` Merkle paths. Because the root is fixed before the rows are
//! chosen, there is no grinding: a prover that commits garbage in a fraction `f` of the rows is caught
//! with probability `1 - (1 - f)^k`, and a wrong opened row passes the check with probability at most
//! `2^-63` (the integer error is below `2^32 < P`, spec/05). The deadline turns a passing response into
//! a lower bound on throughput: at least `(1 - f) m n^3` multiply-adds in the measured time.
//!
//! What it does not show: where the work ran (a prover can forward the challenge), or anything about
//! floating-point workloads.

use abacus_verifier::int8::to_field;
use abacus_verifier::sha256::sha256;
use abacus_verifier::P;

pub mod net;

pub const DOM_SEED: &[u8] = b"abacus/attest-seed";
pub const DOM_EXPAND: &[u8] = b"abacus/expand";
pub const DOM_EMPTY: &[u8] = b"abacus/attest-empty";
pub const DOM_SAMPLE: &[u8] = b"abacus/attest-sample";
pub const MAX_N: usize = abacus_verifier::int8::MAX_N;

/// Seed of product `j`: `SHA256("abacus/attest-seed" || seed || LE32(j))`.
pub fn product_seed(seed: &[u8; 32], j: u32) -> [u8; 32] {
    let mut m = Vec::with_capacity(DOM_SEED.len() + 36);
    m.extend_from_slice(DOM_SEED);
    m.extend_from_slice(seed);
    m.extend_from_slice(&j.to_le_bytes());
    sha256(&m)
}

/// Bytes `[start, start + len)` of the stream `SHA256("abacus/expand" || seed || LE32(c))`, c = 0, 1, ...
pub fn expand_range(seed: &[u8; 32], start: usize, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 64);
    let mut c = start / 32;
    let skip = start % 32;
    let mut m = Vec::with_capacity(DOM_EXPAND.len() + 36);
    while out.len() < skip + len {
        m.clear();
        m.extend_from_slice(DOM_EXPAND);
        m.extend_from_slice(seed);
        m.extend_from_slice(&(c as u32).to_le_bytes());
        out.extend_from_slice(&sha256(&m));
        c += 1;
    }
    out[skip..skip + len].to_vec()
}

fn as_i8(bytes: Vec<u8>) -> Vec<i8> {
    bytes.into_iter().map(|b| b as i8).collect()
}

/// Row `i` of `A_j` (bytes `[i n, (i + 1) n)` of the product stream).
pub fn a_row(seed: &[u8; 32], j: u32, n: usize, i: usize) -> Vec<i8> {
    as_i8(expand_range(&product_seed(seed, j), i * n, n))
}

/// `B_j`, row-major (bytes `[n^2, 2 n^2)` of the product stream).
pub fn b_matrix(seed: &[u8; 32], j: u32, n: usize) -> Vec<i8> {
    as_i8(expand_range(&product_seed(seed, j), n * n, n * n))
}

/// Exact row `i` of `C_j = A_j * B_j`.
pub fn c_row(a_row: &[i8], b: &[i8], n: usize) -> Vec<i32> {
    let mut row = vec![0i32; n];
    for (k, &a) in a_row.iter().enumerate() {
        let a = a as i32;
        for (c, &bk) in row.iter_mut().zip(&b[k * n..(k + 1) * n]) {
            *c += a * bk as i32;
        }
    }
    row
}

/// Leaf: `SHA256(row as LE int32 || 0x00 || LE32(j) || LE32(i))`. The row comes first so that, for
/// `n` a multiple of 16, it fills whole 64-byte SHA-256 blocks (aligned loads on the GPU); the 9-byte
/// suffix binds the position. Leaves (`4 n + 9` bytes) and nodes (65 bytes) never have equal length.
pub fn leaf_hash(j: u32, i: u32, row: &[i32]) -> [u8; 32] {
    let mut m = Vec::with_capacity(9 + row.len() * 4);
    for &x in row {
        m.extend_from_slice(&x.to_le_bytes());
    }
    m.push(0);
    m.extend_from_slice(&j.to_le_bytes());
    m.extend_from_slice(&i.to_le_bytes());
    sha256(&m)
}

fn node(l: &[u8; 32], r: &[u8; 32]) -> [u8; 32] {
    let mut m = [0u8; 65];
    m[0] = 1;
    m[1..33].copy_from_slice(l);
    m[33..].copy_from_slice(r);
    sha256(&m)
}

pub fn empty_leaf() -> [u8; 32] {
    sha256(DOM_EMPTY)
}

/// All levels of the Merkle tree over `leaves` (padded with `empty_leaf` to a power of two);
/// `levels[0]` are the leaves, the last level is `[root]`. Leaf order: `j * n + i`.
pub fn merkle_levels(leaves: &[[u8; 32]]) -> Vec<Vec<[u8; 32]>> {
    let width = leaves.len().max(1).next_power_of_two();
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    level.resize(width, empty_leaf());
    let mut levels = vec![level];
    while levels.last().unwrap().len() > 1 {
        let prev = levels.last().unwrap();
        levels.push(prev.chunks(2).map(|p| node(&p[0], &p[1])).collect());
    }
    levels
}

pub fn merkle_path(levels: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<[u8; 32]> {
    let mut path = Vec::with_capacity(levels.len());
    for level in &levels[..levels.len() - 1] {
        path.push(level[idx ^ 1]);
        idx >>= 1;
    }
    path
}

pub fn verify_path(root: &[u8; 32], leaf: &[u8; 32], mut idx: usize, path: &[[u8; 32]]) -> bool {
    let mut h = *leaf;
    for sib in path {
        h = if idx & 1 == 0 { node(&h, sib) } else { node(sib, &h) };
        idx >>= 1;
    }
    idx == 0 && &h == root
}

/// Depth of the tree over `count` leaves.
pub fn tree_depth(count: usize) -> usize {
    count.max(1).next_power_of_two().trailing_zeros() as usize
}

/// Verifier state prepared before the challenge: the secret Freivalds vector `r` and `v_j = B_j r`.
pub struct Precomputed {
    pub n: usize,
    pub m: usize,
    pub r: Vec<u64>,
    pub v: Vec<Vec<u64>>,
}

/// `O(m n^2)`; run before the challenge is sent. `secret` must be unknown to the prover.
pub fn precompute(seed: &[u8; 32], n: usize, m: usize, secret: &[u8; 32]) -> Precomputed {
    let rbytes = expand_range(secret, 0, 8 * n);
    let r: Vec<u64> = rbytes.chunks(8).map(|c| u64::from_le_bytes(c.try_into().unwrap()) % P).collect();
    let v = (0..m as u32)
        .map(|j| {
            let b = b_matrix(seed, j, n);
            (0..n)
                .map(|k| {
                    let row = &b[k * n..(k + 1) * n];
                    let acc = row
                        .iter()
                        .zip(&r)
                        .fold(0u128, |acc, (&x, &rv)| (acc + to_field(x as i64) as u128 * rv as u128) % P as u128);
                    acc as u64
                })
                .collect()
        })
        .collect();
    Precomputed { n, m, r, v }
}

/// Check one opened row in `O(n)`: `row . r == A_j[i, :] . v_j (mod P)`.
pub fn check_row(pre: &Precomputed, seed: &[u8; 32], j: u32, i: usize, row: &[i32]) -> bool {
    if row.len() != pre.n || j as usize >= pre.m || i >= pre.n {
        return false;
    }
    let p = P as u128;
    let lhs = row.iter().zip(&pre.r).fold(0u128, |acc, (&c, &rv)| (acc + to_field(c as i64) as u128 * rv as u128) % p);
    let a = a_row(seed, j, pre.n, i);
    let rhs = a
        .iter()
        .zip(&pre.v[j as usize])
        .fold(0u128, |acc, (&x, &vv)| (acc + to_field(x as i64) as u128 * vv as u128) % p);
    lhs == rhs
}

/// `k` distinct row indices `(j, i)` chosen from verifier-private randomness and the committed root.
pub fn sample(secret: &[u8; 32], root: &[u8; 32], n: usize, m: usize, k: usize) -> Vec<(u32, u32)> {
    let total = n * m;
    let k = k.min(total);
    let mut out: Vec<(u32, u32)> = Vec::with_capacity(k);
    let mut ctr: u64 = 0;
    while out.len() < k {
        let mut msg = Vec::with_capacity(DOM_SAMPLE.len() + 72);
        msg.extend_from_slice(DOM_SAMPLE);
        msg.extend_from_slice(secret);
        msg.extend_from_slice(root);
        msg.extend_from_slice(&ctr.to_le_bytes());
        let h = sha256(&msg);
        let x = (u64::from_le_bytes(h[..8].try_into().unwrap()) % total as u64) as usize;
        let pick = ((x / n) as u32, (x % n) as u32);
        if !out.contains(&pick) {
            out.push(pick);
        }
        ctr += 1;
    }
    out
}

/// Probability that a prover with a fraction `f` of wrong rows passes `k` distinct random openings
/// (sampling without replacement from `total` rows).
pub fn pass_probability(f: f64, total: usize, k: usize) -> f64 {
    let bad = (f * total as f64).round() as usize;
    let good = total - bad;
    (0..k).fold(1.0, |p, t| if t >= good { 0.0 } else { p * (good - t) as f64 / (total - t) as f64 })
}

// ---------------- reference CPU prover (tests, small n) ----------------

/// All rows of all products, computed honestly on the CPU. `cheat_rows` leaves the last rows of every
/// product as zeros (a prover skipping that work).
pub fn cpu_prove(seed: &[u8; 32], n: usize, m: usize, cheat_rows: usize) -> Vec<Vec<Vec<i32>>> {
    (0..m as u32)
        .map(|j| {
            let ps = product_seed(seed, j);
            let a = as_i8(expand_range(&ps, 0, n * n));
            let b = as_i8(expand_range(&ps, n * n, n * n));
            (0..n)
                .map(|i| if i >= n - cheat_rows { vec![0i32; n] } else { c_row(&a[i * n..(i + 1) * n], &b, n) })
                .collect()
        })
        .collect()
}

pub fn leaves_of(rows: &[Vec<Vec<i32>>]) -> Vec<[u8; 32]> {
    rows.iter()
        .enumerate()
        .flat_map(|(j, prod)| prod.iter().enumerate().map(move |(i, row)| leaf_hash(j as u32, i as u32, row)))
        .collect()
}

// ---------------- wire format helpers ----------------

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

pub fn unhex32(s: &str) -> Option<[u8; 32]> {
    unhex(s)?.try_into().ok()
}

/// `ROW <j> <i> <row hex, LE int32> <path hex, concatenated 32-byte siblings>`
pub fn encode_row_line(j: u32, i: u32, row: &[i32], path: &[[u8; 32]]) -> String {
    let rb: Vec<u8> = row.iter().flat_map(|x| x.to_le_bytes()).collect();
    let pb: Vec<u8> = path.iter().flatten().copied().collect();
    format!("ROW {j} {i} {} {}", hex(&rb), hex(&pb))
}

pub type OpenedRow = (u32, u32, Vec<i32>, Vec<[u8; 32]>);

pub fn decode_row_line(line: &str, n: usize, depth: usize) -> Option<OpenedRow> {
    let mut it = line.strip_prefix("ROW ")?.split(' ');
    let j: u32 = it.next()?.parse().ok()?;
    let i: u32 = it.next()?.parse().ok()?;
    let rb = unhex(it.next()?)?;
    let pb = unhex(it.next().unwrap_or(""))?;
    if rb.len() != 4 * n || pb.len() != 32 * depth || it.next().is_some() {
        return None;
    }
    let row = rb.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect();
    let path = pb.chunks(32).map(|c| c.try_into().unwrap()).collect();
    Some((j, i, row, path))
}

/// Verifier randomness from the OS: `RandomState` keys are drawn from the operating system's generator;
/// hashed together with the clock and a counter through SHA-256. Research use; see spec/06.
pub fn os_random32(tag: u64) -> [u8; 32] {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut m = Vec::new();
    for t in 0..4u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(tag ^ t);
        m.extend_from_slice(&h.finish().to_le_bytes());
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    m.extend_from_slice(&nanos.to_le_bytes());
    sha256(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_match_full_product_and_ranges_match_stream() {
        let seed = [3u8; 32];
        let n = 16;
        let ps = product_seed(&seed, 1);
        let full = expand_range(&ps, 0, 2 * n * n);
        assert_eq!(expand_range(&ps, 37, 50), full[37..87].to_vec());
        let a = as_i8(full[..n * n].to_vec());
        let b = as_i8(full[n * n..].to_vec());
        let c = abacus_verifier::int8::matmul_i8(&a, &b, n);
        for i in 0..n {
            assert_eq!(c_row(&a_row(&seed, 1, n, i), &b_matrix(&seed, 1, n), n), c[i * n..(i + 1) * n].to_vec());
        }
    }

    #[test]
    fn honest_rows_pass_and_tampered_rows_fail() {
        let (seed, secret) = ([5u8; 32], [6u8; 32]);
        let (n, m) = (16, 2);
        let pre = precompute(&seed, n, m, &secret);
        let rows = cpu_prove(&seed, n, m, 0);
        for (j, product) in rows.iter().enumerate() {
            for (i, row) in product.iter().enumerate() {
                assert!(check_row(&pre, &seed, j as u32, i, row));
                let mut bad = row.clone();
                bad[i % n] = bad[i % n].wrapping_add(1);
                assert!(!check_row(&pre, &seed, j as u32, i, &bad));
            }
        }
    }

    #[test]
    fn merkle_paths_verify_and_bind_position() {
        let rows = cpu_prove(&[7u8; 32], 8, 3, 0);
        let leaves = leaves_of(&rows);
        let levels = merkle_levels(&leaves);
        let root = levels.last().unwrap()[0];
        assert_eq!(tree_depth(leaves.len()), levels.len() - 1);
        for (idx, leaf) in leaves.iter().enumerate() {
            let path = merkle_path(&levels, idx);
            assert!(verify_path(&root, leaf, idx, &path));
            assert!(!verify_path(&root, leaf, idx ^ 1, &path));
        }
    }

    #[test]
    fn samples_are_distinct_and_in_range() {
        let s = sample(&[1u8; 32], &[2u8; 32], 16, 4, 20);
        assert_eq!(s.len(), 20);
        for (t, &(j, i)) in s.iter().enumerate() {
            assert!(j < 4 && i < 16);
            assert!(!s[..t].contains(&(j, i)));
        }
    }

    #[test]
    fn pass_probability_matches_formula() {
        assert_eq!(pass_probability(0.0, 1000, 10), 1.0);
        assert!((pass_probability(0.1, 100_000, 30) - 0.9f64.powi(30)).abs() < 1e-3);
        assert_eq!(pass_probability(1.0, 100, 1), 0.0);
    }

    #[test]
    fn row_line_round_trip() {
        let row = vec![1, -2, i32::MAX, i32::MIN];
        let path = vec![[9u8; 32]; 3];
        let line = encode_row_line(2, 5, &row, &path);
        assert_eq!(decode_row_line(&line, 4, 3), Some((2, 5, row, path)));
        assert_eq!(decode_row_line(&line, 5, 3), None);
    }
}
