//! Fiat–Shamir-bound Freivalds verification (ADR 0004).
//!
//! `k` challenge vectors are derived from the committed `C` (domain `abacus/check`), so a prover who
//! fixes `C` first cannot construct a null-space forgery. Matches `reference.freivalds` /
//! `scripts/freivalds_forgery_probe.py` (domain, little-endian encoding, SHA-256, first 8 bytes LE).

use crate::sha256::sha256;
use crate::{freivalds_verify_multi, P};

pub fn fs_challenges(c: &[u64], n: usize, k: usize) -> Vec<Vec<u64>> {
    let mut cb = Vec::with_capacity(c.len() * 8);
    for &x in c {
        cb.extend_from_slice(&x.to_le_bytes());
    }
    let mut out = Vec::with_capacity(k);
    for i in 0..k {
        let mut v = Vec::with_capacity(n);
        for j in 0..n {
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(b"abacus/check");
            buf.extend_from_slice(&cb);
            buf.extend_from_slice(&(i as u32).to_le_bytes());
            buf.extend_from_slice(&(j as u32).to_le_bytes());
            let h = sha256(&buf);
            let mut e = [0u8; 8];
            e.copy_from_slice(&h[0..8]);
            v.push(u64::from_le_bytes(e) % P);
        }
        out.push(v);
    }
    out
}

/// Verify `C == A*B` with `k` Fiat–Shamir-bound Freivalds challenges.
pub fn verify_fs(a: &[u64], b: &[u64], c: &[u64], n: usize, k: usize) -> bool {
    let rs = fs_challenges(c, n, k);
    freivalds_verify_multi(a, b, c, n, &rs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{matmul, Rng};

    #[test]
    fn accepts_honest_product() {
        let mut rng = Rng::new(21);
        let n = 8;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let c = matmul(&a, &b, n);
        assert!(verify_fs(&a, &b, &c, n, 16));
    }

    #[test]
    fn challenges_are_deterministic_and_shaped() {
        let c = (0..16u64).collect::<Vec<u64>>();
        let rs = fs_challenges(&c, 4, 5);
        assert_eq!(rs.len(), 5);
        assert_eq!(rs[0].len(), 4);
        assert_eq!(rs, fs_challenges(&c, 4, 5));
    }

    #[test]
    fn rejects_tampered_product() {
        let mut rng = Rng::new(22);
        let n = 8;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let mut c = matmul(&a, &b, n);
        c[0] = (c[0] + 1) % P;
        // k = 16, so a wrong C passes all challenges with probability at most 2^-16.
        assert!(!verify_fs(&a, &b, &c, n, 16));
    }
}
