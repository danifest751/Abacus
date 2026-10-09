//! Candidate T (ADR 0015, spec/07): deep requantized int8 network PoW with row-ticket lottery.
//!
//! Per epoch, `L` weight matrices `W_l` (`n x n`, int8) are derived from an epoch seed. Per attempt, an
//! input `X_0` (`b x n`, int8) is derived from the header digest and a nonce, and the miner runs
//!
//! ```text
//! X_l = requant(X_{l-1} * W_l),   requant(y) = clamp((y * M + 2^23) >> 24, -128, 127),   l = 1..L
//! ```
//!
//! The lottery tickets are the `w`-byte pieces of the rows of `X_L`: ticket `(i, c)` scores
//! `SHA256(X_L[i, c w .. (c + 1) w] || 0x54 || header_digest || LE64(nonce) || LE32(i) || LE32(c))`.
//! A block carries only `(nonce, i, c)`; a verifier recomputes row `i` through the `L` layers
//! (`L n^2` multiply-adds) and the hash. Every ticket costs the same `L n w` multiply-adds whether the
//! miner batches rows (GEMM) or computes one row (GEMV); the requantization between layers blocks
//! linear shortcuts such as collapsing the layers or scoring `X r` without the product.

use abacus_verifier::sha256::sha256;

use crate::expand_bytes;

pub const DOM_W: &[u8] = b"abacus/tnet-w";
pub const DOM_X0: &[u8] = b"abacus/tnet-x0";
pub const TICKET_TAG: u8 = 0x54;

#[derive(Clone, Copy, Debug)]
pub struct TnetParams {
    /// Width of every layer.
    pub n: usize,
    /// Rows of `X_0` per attempt (batch).
    pub b: usize,
    /// Number of layers.
    pub layers: usize,
    /// Ticket width in bytes (a divisor of `n`).
    pub w: usize,
    /// Requantization multiplier `M` (fixed point, `REQ_SHIFT` fractional bits).
    pub mult: i32,
}

impl TnetParams {
    /// Tickets per attempt.
    pub fn tickets(&self) -> usize {
        self.b * (self.n / self.w)
    }

    /// Multiply-adds per attempt.
    pub fn macs(&self) -> f64 {
        self.layers as f64 * self.b as f64 * (self.n as f64).powi(2)
    }
}

/// Fractional bits of the requantization multiplier.
pub const REQ_SHIFT: u32 = 24;

/// Multiplier that keeps the activations' spread constant across layers for uniform int8 weights:
/// `round(2^24 / (74 sqrt(n)))` (74 ~ standard deviation of a uniform int8). A power-of-two scale is
/// too coarse: it makes the activations grow into saturation or decay towards a few bits.
pub fn default_mult(n: usize) -> i32 {
    ((1u64 << REQ_SHIFT) as f64 / (74.0 * (n as f64).sqrt())).round() as i32
}

fn i8s(bytes: Vec<u8>) -> Vec<i8> {
    bytes.into_iter().map(|x| x as i8).collect()
}

/// `W_l`, row-major (`W[k][j]` at `k n + j`): `expand(SHA256("abacus/tnet-w" || epoch || LE32(l)), n^2)`.
pub fn epoch_weights(epoch_seed: &[u8; 32], p: &TnetParams) -> Vec<Vec<i8>> {
    (0..p.layers as u32)
        .map(|l| {
            let mut m = Vec::with_capacity(DOM_W.len() + 36);
            m.extend_from_slice(DOM_W);
            m.extend_from_slice(epoch_seed);
            m.extend_from_slice(&l.to_le_bytes());
            i8s(expand_bytes(&sha256(&m), p.n * p.n))
        })
        .collect()
}

/// `SHA256("abacus/tnet-x0" || header_digest || LE64(nonce))`.
pub fn x0_seed(header_digest: &[u8; 32], nonce: u64) -> [u8; 32] {
    let mut m = Vec::with_capacity(DOM_X0.len() + 40);
    m.extend_from_slice(DOM_X0);
    m.extend_from_slice(header_digest);
    m.extend_from_slice(&nonce.to_le_bytes());
    sha256(&m)
}

/// Row `i` of `X_0`: bytes `[i n, (i + 1) n)` of `expand(x0_seed)`.
pub fn x0_row(seed: &[u8; 32], n: usize, i: usize) -> Vec<i8> {
    let start = i * n;
    let skip = start % 32;
    let first = start / 32;
    // expand_bytes produces from counter 0; compute just the needed blocks.
    let mut out = Vec::with_capacity(n + 64);
    let mut c = first as u32;
    while out.len() < skip + n {
        let mut m = Vec::with_capacity(crate::DOM_EXPAND.len() + 36);
        m.extend_from_slice(crate::DOM_EXPAND);
        m.extend_from_slice(seed);
        m.extend_from_slice(&c.to_le_bytes());
        out.extend_from_slice(&sha256(&m));
        c += 1;
    }
    i8s(out[skip..skip + n].to_vec())
}

#[inline]
pub fn requant(y: i32, mult: i32) -> i8 {
    let v = (y as i64 * mult as i64 + (1i64 << (REQ_SHIFT - 1))) >> REQ_SHIFT;
    v.clamp(-128, 127) as i8
}

/// One layer for one row: `requant(x * W)` with `W` row-major.
pub fn layer_row(x: &[i8], w: &[i8], n: usize, mult: i32) -> Vec<i8> {
    let mut acc = vec![0i32; n];
    for (k, &xk) in x.iter().enumerate() {
        let xk = xk as i32;
        if xk == 0 {
            continue;
        }
        for (a, &wkj) in acc.iter_mut().zip(&w[k * n..(k + 1) * n]) {
            *a += xk * wkj as i32;
        }
    }
    acc.into_iter().map(|y| requant(y, mult)).collect()
}

/// Same as `layer_row`, splitting the output columns over `threads` threads.
pub fn layer_row_par(x: &[i8], w: &[i8], n: usize, mult: i32, threads: usize) -> Vec<i8> {
    let threads = threads.max(1).min(n);
    let chunk = n.div_ceil(threads);
    let mut out = vec![0i8; n];
    std::thread::scope(|s| {
        for (t, dst) in out.chunks_mut(chunk).enumerate() {
            let j0 = t * chunk;
            s.spawn(move || {
                let width = dst.len();
                let mut acc = vec![0i32; width];
                for (k, &xk) in x.iter().enumerate() {
                    let xk = xk as i32;
                    if xk == 0 {
                        continue;
                    }
                    let wrow = &w[k * n + j0..k * n + j0 + width];
                    for (a, &wkj) in acc.iter_mut().zip(wrow) {
                        *a += xk * wkj as i32;
                    }
                }
                for (d, y) in dst.iter_mut().zip(acc) {
                    *d = requant(y, mult);
                }
            });
        }
    });
    out
}

/// Row `i` of `X_L` (the verifier's computation: `L n^2` multiply-adds).
pub fn forward_row(weights: &[Vec<i8>], p: &TnetParams, seed: &[u8; 32], i: usize, threads: usize) -> Vec<i8> {
    let mut x = x0_row(seed, p.n, i);
    for w in weights {
        x = if threads > 1 { layer_row_par(&x, w, p.n, p.mult, threads) } else { layer_row(&x, w, p.n, p.mult) };
    }
    x
}

/// `SHA256(piece || 0x54 || header_digest || LE64(nonce) || LE32(i) || LE32(c))`.
pub fn ticket_hash(piece: &[i8], header_digest: &[u8; 32], nonce: u64, i: u32, c: u32) -> [u8; 32] {
    let mut m = Vec::with_capacity(piece.len() + 49);
    m.extend(piece.iter().map(|&x| x as u8));
    m.push(TICKET_TAG);
    m.extend_from_slice(header_digest);
    m.extend_from_slice(&nonce.to_le_bytes());
    m.extend_from_slice(&i.to_le_bytes());
    m.extend_from_slice(&c.to_le_bytes());
    sha256(&m)
}

/// Verify a ticket: recompute row `i`, hash piece `c`, compare leading zero bits with `bits`.
#[allow(clippy::too_many_arguments)]
pub fn verify_ticket(
    weights: &[Vec<i8>],
    p: &TnetParams,
    header_digest: &[u8; 32],
    nonce: u64,
    i: usize,
    c: usize,
    bits: u32,
    threads: usize,
) -> bool {
    if i >= p.b || c >= p.n / p.w || weights.len() != p.layers {
        return false;
    }
    let row = forward_row(weights, p, &x0_seed(header_digest, nonce), i, threads);
    let h = ticket_hash(&row[c * p.w..(c + 1) * p.w], header_digest, nonce, i as u32, c as u32);
    crate::score_lead(&h) >= bits
}

/// Reference full attempt for small instances: all rows of `X_L`.
pub fn forward_all(weights: &[Vec<i8>], p: &TnetParams, seed: &[u8; 32]) -> Vec<Vec<i8>> {
    (0..p.b).map(|i| forward_row(weights, p, seed, i, 1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> TnetParams {
        TnetParams { n: 64, b: 16, layers: 4, w: 16, mult: default_mult(64) }
    }

    #[test]
    fn requant_rounds_half_up_and_saturates() {
        let half = 1 << (REQ_SHIFT - 4); // mult = 2^20 -> scale 1/16
        assert_eq!(requant(0, half), 0);
        assert_eq!(requant(8, half), 1);
        assert_eq!(requant(7, half), 0);
        assert_eq!(requant(-8, half), 0);
        assert_eq!(requant(-9, half), -1);
        assert_eq!(requant(1 << 20, half), 127);
        assert_eq!(requant(-(1 << 20), half), -128);
        assert_eq!(default_mult(4096), 3542);
    }

    #[test]
    fn parallel_layer_matches_serial() {
        let p = params();
        let w = epoch_weights(&[1u8; 32], &p);
        let x = x0_row(&x0_seed(&[2u8; 32], 5), p.n, 3);
        for t in [1, 2, 3, 7] {
            assert_eq!(layer_row_par(&x, &w[0], p.n, p.mult, t), layer_row(&x, &w[0], p.n, p.mult));
        }
    }

    #[test]
    fn x0_rows_match_the_stream() {
        let seed = x0_seed(&[3u8; 32], 9);
        let full = expand_bytes(&seed, 64 * 10);
        for i in 0..10 {
            assert_eq!(x0_row(&seed, 64, i), i8s(full[i * 64..(i + 1) * 64].to_vec()));
        }
    }

    #[test]
    fn activations_do_not_collapse() {
        let p = TnetParams { n: 256, b: 8, layers: 8, w: 32, mult: default_mult(256) };
        let w = epoch_weights(&[4u8; 32], &p);
        let rows = forward_all(&w, &p, &x0_seed(&[5u8; 32], 1));
        let all: Vec<i8> = rows.concat();
        let zeros = all.iter().filter(|&&x| x == 0).count() as f64 / all.len() as f64;
        let sat = all.iter().filter(|&&x| x == 127 || x == -128).count() as f64 / all.len() as f64;
        let mean_abs = all.iter().map(|&x| (x as i32).abs() as f64).sum::<f64>() / all.len() as f64;
        assert!(zeros < 0.05 && sat < 0.08 && mean_abs > 25.0, "zeros {zeros}, saturated {sat}, mean |x| {mean_abs}");
    }

    #[test]
    fn found_ticket_verifies_and_tampering_fails() {
        let p = params();
        let w = epoch_weights(&[6u8; 32], &p);
        let hd = [7u8; 32];
        for nonce in 0..64u64 {
            let rows = forward_all(&w, &p, &x0_seed(&hd, nonce));
            for (i, row) in rows.iter().enumerate() {
                for c in 0..p.n / p.w {
                    let h = ticket_hash(&row[c * p.w..(c + 1) * p.w], &hd, nonce, i as u32, c as u32);
                    if crate::score_lead(&h) >= 6 {
                        assert!(verify_ticket(&w, &p, &hd, nonce, i, c, 6, 1));
                        let other = ticket_hash(&row[c * p.w..(c + 1) * p.w], &hd, nonce, i as u32 + 1, c as u32);
                        assert_ne!(other, h, "the position is bound into the ticket");
                        assert!(!verify_ticket(&w, &p, &hd, nonce, i, c, 40, 1));
                        return;
                    }
                }
            }
        }
        panic!("no 6-bit ticket found in 64 attempts");
    }
}
