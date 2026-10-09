//! Fiat–Shamir-bound Freivalds verification (ADR 0004, ADR 0006, ADR 0009).
//!
//! `k` challenge vectors are derived from the preheader and the committed `C` (domain
//! `abacus/check`), so a prover who fixes `C` first cannot construct a null-space forgery. Each
//! challenge is nearly uniform over `F_P`, so a wrong `C` passes one challenge with probability at
//! most `2^-63` (ADR 0009). Matches `reference.chain.fs_challenges` (domain, little-endian encoding,
//! SHA-256, first 8 bytes LE).

use crate::sha256::sha256;
use crate::{freivalds_verify_multi, P};

pub const DOM_CHECK: &[u8] = b"abacus/check";

pub fn fs_challenges(ph: &[u8], c: &[u64], n: usize, k: usize) -> Vec<Vec<u64>> {
    // Hash (preheader, C) once to a root, then expand k*n field elements (ADR 0006).
    let mut root_in = Vec::with_capacity(DOM_CHECK.len() + ph.len() + c.len() * 8);
    root_in.extend_from_slice(DOM_CHECK);
    root_in.extend_from_slice(ph);
    for &x in c {
        root_in.extend_from_slice(&x.to_le_bytes());
    }
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

/// Verify `C == A*B` with `k` Fiat–Shamir challenges bound to `(ph, C)`. Returns `false` (never
/// panics) on a shape mismatch, `k == 0` or a non-canonical entry of `C`.
pub fn verify_fs(ph: &[u8], a: &[u64], b: &[u64], c: &[u64], n: usize, k: usize) -> bool {
    let nn = match n.checked_mul(n) {
        Some(v) => v,
        None => return false,
    };
    if k == 0 || a.len() != nn || b.len() != nn || c.len() != nn || c.iter().any(|&x| x >= P) {
        return false;
    }
    let rs = fs_challenges(ph, c, n, k);
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
        assert!(verify_fs(b"ph", &a, &b, &c, n, 16));
    }

    #[test]
    fn challenges_are_deterministic_shaped_and_bound_to_preheader() {
        let c = (0..16u64).collect::<Vec<u64>>();
        let rs = fs_challenges(b"ph", &c, 4, 5);
        assert_eq!(rs.len(), 5);
        assert_eq!(rs[0].len(), 4);
        assert_eq!(rs, fs_challenges(b"ph", &c, 4, 5));
        assert_ne!(rs, fs_challenges(b"ph2", &c, 4, 5));
    }

    #[test]
    fn rejects_tampered_product() {
        let mut rng = Rng::new(22);
        let n = 8;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let mut c = matmul(&a, &b, n);
        c[0] = (c[0] + 1) % P;
        assert!(!verify_fs(b"ph", &a, &b, &c, n, 2));
    }

    #[test]
    fn rejects_bad_shapes_without_panicking() {
        let mut rng = Rng::new(23);
        let n = 4;
        let a = rng.matrix(n);
        let b = rng.matrix(n);
        let c = matmul(&a, &b, n);
        assert!(!verify_fs(b"", &a, &b, &c[..3], n, 2));
        assert!(!verify_fs(b"", &a[..5], &b, &c, n, 2));
        assert!(!verify_fs(b"", &a, &b, &c, n, 0));
        let mut big = c.clone();
        big[0] = P; // non-canonical encoding of 0
        assert!(!verify_fs(b"", &a, &b, &big, n, 2));
    }
}
