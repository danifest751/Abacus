//! Abacus research laboratory — independent verifier.
//!
//! Field: arithmetic modulo Goldilocks `P = 2^64 - 2^32 + 1` (a prime). All products use `u128` so there is no
//! silent overflow; every public entry point works on row-major `n x n` matrices of residues.
//!
//! The verifier implements **Freivalds' algorithm**: given a claimed product `C` of `A` and `B`,
//! it checks `A * (B * r) == C * r` for a random vector `r`. This is `O(n^2)` rather than `O(n^3)`
//! and rejects an incorrect `C` with probability at least `1/2` per vector (amplified by using
//! several vectors). It tests the *result*, not that a miner did the work — that gap is exactly
//! what the research studies.

pub mod freivalds_fs;
pub mod goldilocks;
pub mod ntt;
pub mod sha256;
pub mod sumcheck;

/// The field modulus of the matmul laboratory: Goldilocks `2^64 - 2^32 + 1` (shared with the NTT
/// candidate and with the CPPminer Quantus arithmetic for reuse). Prime.
pub const P: u64 = goldilocks::P;

/// Reduce a `u128` modulo `P`.
#[inline]
pub fn reduce(x: u128) -> u64 {
    (x % P as u128) as u64
}

/// `(a * b) mod P` without overflow.
#[inline]
pub fn mul_mod(a: u64, b: u64) -> u64 {
    debug_assert!(a < P && b < P);
    reduce(a as u128 * b as u128)
}

/// `(a + b) mod P` without overflow.
#[inline]
pub fn add_mod(a: u64, b: u64) -> u64 {
    debug_assert!(a < P && b < P);
    reduce(a as u128 + b as u128)
}

/// Row-major `n x n` matrix product modulo `P`.
pub fn matmul(a: &[u64], b: &[u64], n: usize) -> Vec<u64> {
    assert_eq!(a.len(), n * n, "A shape");
    assert_eq!(b.len(), n * n, "B shape");
    let mut c = vec![0u64; n * n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i * n + k];
            if aik == 0 {
                continue;
            }
            for j in 0..n {
                let idx = i * n + j;
                c[idx] = add_mod(c[idx], mul_mod(aik, b[k * n + j]));
            }
        }
    }
    c
}

/// Matrix-vector product `m * v` modulo `P` (`m` is row-major `n x n`).
pub fn matvec(m: &[u64], v: &[u64], n: usize) -> Vec<u64> {
    assert_eq!(m.len(), n * n, "matrix shape");
    assert_eq!(v.len(), n, "vector shape");
    let mut out = vec![0u64; n];
    for i in 0..n {
        let mut acc = 0u64;
        for j in 0..n {
            acc = add_mod(acc, mul_mod(m[i * n + j], v[j]));
        }
        out[i] = acc;
    }
    out
}

/// Freivalds check: does `C == A * B`? `r` is one random vector of length `n`.
///
/// Returns `true` when `A * (B * r) == C * r`. For `r` uniform over `F_P^n` an incorrect `C`
/// passes with probability at most `1/P` (Schwartz–Zippel; the classical `1/2` bound is for
/// `r in {0,1}^n`, see ADR 0009). Returns `false` on a shape mismatch instead of panicking.
pub fn freivalds_verify(a: &[u64], b: &[u64], c: &[u64], n: usize, r: &[u64]) -> bool {
    let nn = match n.checked_mul(n) {
        Some(v) => v,
        None => return false,
    };
    if a.len() != nn || b.len() != nn || c.len() != nn || r.len() != n {
        return false;
    }
    let br = matvec(b, r, n);
    let abr = matvec(a, &br, n);
    let cr = matvec(c, r, n);
    abr == cr
}

/// Freivalds with several challenge vectors; returns `true` only if every vector agrees.
pub fn freivalds_verify_multi(a: &[u64], b: &[u64], c: &[u64], n: usize, rs: &[Vec<u64>]) -> bool {
    !rs.is_empty() && rs.iter().all(|r| freivalds_verify(a, b, c, n, r))
}

/// Deterministic xorshift64* generator for reproducible tests and probes.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// A residue in `[0, P)`.
    pub fn residue(&mut self) -> u64 {
        self.next_u64() % P
    }

    pub fn matrix(&mut self, n: usize) -> Vec<u64> {
        (0..n * n).map(|_| self.residue()).collect()
    }

    pub fn vector(&mut self, n: usize) -> Vec<u64> {
        (0..n).map(|_| self.residue()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_true_product() {
        let mut rng = Rng::new(42);
        let n = 8;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let c = matmul(&a, &b, n);
        for _ in 0..8 {
            let r = rng.vector(n);
            assert!(freivalds_verify(&a, &b, &c, n, &r));
        }
    }

    #[test]
    fn rejects_a_tampered_product() {
        let mut rng = Rng::new(7);
        let n = 16;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let mut c = matmul(&a, &b, n);
        c[0] = (c[0] + 1) % P;
        // Any single non-degenerate r rejects; use a few to be safe.
        let rs: Vec<Vec<u64>> = (0..8).map(|_| rng.vector(n)).collect();
        assert!(!freivalds_verify_multi(&a, &b, &c, n, &rs));
    }

    #[test]
    fn mul_add_agree_with_reduce() {
        assert_eq!(mul_mod(P - 1, P - 1), 1); // (-1)*(-1) = 1
        assert_eq!(add_mod(P - 1, 1), 0);
    }

    #[test]
    fn shape_mismatch_rejects_without_panicking() {
        let mut rng = Rng::new(9);
        let n = 4;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let c = matmul(&a, &b, n);
        let r = rng.vector(n);
        assert!(!freivalds_verify(&a, &b, &c[..15], n, &r));
        assert!(!freivalds_verify(&a, &b, &c, n, &r[..3]));
    }
}
