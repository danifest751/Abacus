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
    // Hash C once to a root, then expand k*n field elements (ADR 0006).
    let mut root_in = Vec::with_capacity(12 + cb.len());
    root_in.extend_from_slice(b"abacus/check");
    root_in.extend_from_slice(&cb);
    let root = sha256(&root_in);

    let need = k * n;
    let mut vals: Vec<u64> = Vec::with_capacity(need);
    let mut counter: u32 = 0;
    while vals.len() < need {
        let mut buf = Vec::with_capacity(36);
        buf.extend_from_slice(&root);
        buf.extend_from_slice(&counter.to_le_bytes());
        let h = sha256(&buf);
        for off in [0usize, 8, 16, 24] {
            let mut e = [0u8; 8];
            e.copy_from_slice(&h[off..off + 8]);
            vals.push(u64::from_le_bytes(e) % P);
        }
        counter += 1;
    }
    (0..k).map(|i| vals[i * n..(i + 1) * n].to_vec()).collect()
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
