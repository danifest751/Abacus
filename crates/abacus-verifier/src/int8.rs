//! Candidate A8: int8 matrix product with exact int32 result, verified by Fiat–Shamir Freivalds over
//! Goldilocks (ADR 0012, spec/05).
//!
//! `A, B` are `n x n` matrices of `i8`; the work is the exact integer product `C = A * B` in `i32` (the
//! tensor-core int8 GEMM of a GPU). For `n <= MAX_N` every entry of `A * B` satisfies
//! `|c| <= n * 128 * 128 < 2^31`, and any claimed `C` has `|c| < 2^31`, so a wrong claim differs from
//! the true product by an integer matrix `E` with `0 < |e_ij| < 2^32 < P`. Hence `E mod P != 0` and the
//! Freivalds check over `F_P` keeps the per-challenge error `<= 2^-63` of ADR 0009.

use crate::sha256::sha256;
use crate::{freivalds_verify_multi, P};

/// Largest supported dimension: `MAX_N * 128 * 128 < 2^31`.
pub const MAX_N: usize = 1 << 16;
pub const DOM_CHECK_I8: &[u8] = b"abacus/check-i8";

/// Exact integer product `C = A * B` (row-major), accumulated in `i32`.
pub fn matmul_i8(a: &[i8], b: &[i8], n: usize) -> Vec<i32> {
    assert!(n <= MAX_N, "n too large for exact i32 accumulation");
    assert_eq!(a.len(), n * n, "A shape");
    assert_eq!(b.len(), n * n, "B shape");
    let mut c = vec![0i32; n * n];
    for i in 0..n {
        let row = &mut c[i * n..(i + 1) * n];
        for k in 0..n {
            let aik = a[i * n + k] as i32;
            if aik == 0 {
                continue;
            }
            let brow = &b[k * n..(k + 1) * n];
            for (cij, &bkj) in row.iter_mut().zip(brow) {
                *cij += aik * bkj as i32;
            }
        }
    }
    c
}

/// Residue of a signed integer in `[0, P)`.
#[inline]
pub fn to_field(x: i64) -> u64 {
    if x >= 0 {
        x as u64 % P
    } else {
        P - ((-x) as u64 % P)
    }
}

/// Little-endian encoding of `C` (4 bytes per entry, two's complement).
pub fn encode_c_i32(c: &[i32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(c.len() * 4);
    for &x in c {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

/// `k` challenge vectors over `F_P` bound to `(preheader, C)`: commit-then-expand as in ADR 0006.
pub fn fs_challenges_i8(ph: &[u8], c: &[i32], n: usize, k: usize) -> Vec<Vec<u64>> {
    let mut root_in = Vec::with_capacity(DOM_CHECK_I8.len() + ph.len() + c.len() * 4);
    root_in.extend_from_slice(DOM_CHECK_I8);
    root_in.extend_from_slice(ph);
    root_in.extend_from_slice(&encode_c_i32(c));
    let root = sha256(&root_in);
    let need = k * n;
    let mut vals = Vec::with_capacity(need);
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

/// Verify the exact integer product `C == A * B` with `k` Fiat–Shamir challenges over Goldilocks.
/// Returns `false` (never panics) on a shape mismatch, `n > MAX_N` or `k == 0`.
pub fn verify_fs_i8(ph: &[u8], a: &[i8], b: &[i8], c: &[i32], n: usize, k: usize) -> bool {
    let nn = match n.checked_mul(n) {
        Some(v) if n <= MAX_N => v,
        _ => return false,
    };
    if k == 0 || a.len() != nn || b.len() != nn || c.len() != nn {
        return false;
    }
    let af: Vec<u64> = a.iter().map(|&x| to_field(x as i64)).collect();
    let bf: Vec<u64> = b.iter().map(|&x| to_field(x as i64)).collect();
    let cf: Vec<u64> = c.iter().map(|&x| to_field(x as i64)).collect();
    freivalds_verify_multi(&af, &bf, &cf, n, &fs_challenges_i8(ph, c, n, k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rng;

    fn rand_i8(rng: &mut Rng, len: usize) -> Vec<i8> {
        (0..len).map(|_| rng.next_u64() as u8 as i8).collect()
    }

    #[test]
    fn exact_product_and_extreme_values() {
        let n = 4;
        let a = vec![-128i8; n * n];
        let b = vec![-128i8; n * n];
        let c = matmul_i8(&a, &b, n);
        assert!(c.iter().all(|&x| x == 4 * 128 * 128));
        assert!(verify_fs_i8(b"ph", &a, &b, &c, n, 2));
    }

    #[test]
    fn accepts_honest_and_rejects_tampered() {
        let mut rng = Rng::new(31);
        let n = 16;
        let a = rand_i8(&mut rng, n * n);
        let b = rand_i8(&mut rng, n * n);
        let c = matmul_i8(&a, &b, n);
        assert!(verify_fs_i8(b"ph", &a, &b, &c, n, 2));
        for delta in [1i32, -1, i32::MAX, i32::MIN] {
            let mut bad = c.clone();
            bad[5] = bad[5].wrapping_add(delta);
            assert!(!verify_fs_i8(b"ph", &a, &b, &bad, n, 2), "delta {delta}");
        }
    }

    #[test]
    fn integer_error_never_vanishes_mod_p() {
        // The largest possible |true - claimed| is < 2^32 < P, so the residue of a nonzero error is nonzero.
        let worst = (MAX_N as i64) * 128 * 128 + (i32::MAX as i64) + 1;
        assert!(worst < (1i64 << 32) && (worst as u64) < P);
        assert_ne!(to_field(-1), 0);
        assert_eq!((to_field(-5) + 5) % P, 0);
    }

    #[test]
    fn rejects_bad_shapes_without_panicking() {
        let a = vec![0i8; 16];
        let c = vec![0i32; 15];
        assert!(!verify_fs_i8(b"", &a, &a, &c, 4, 2));
        assert!(!verify_fs_i8(b"", &a, &a, &[0i32; 16], 4, 0));
    }
}
