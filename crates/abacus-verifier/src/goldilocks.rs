//! Goldilocks field: arithmetic modulo `P = 2**64 - 2**32 + 1`.
//!
//! Field used by modern STARK/FRI provers. 2-adicity is 32, so NTT sizes up to `2**32` are
//! supported. Generator `7`; `P - 1 = 2**32 * (2**32 - 1)`, so `7 ** ((P - 1) >> k)` is a
//! primitive `2**k`-th root of unity.

/// Field modulus: `2**64 - 2**32 + 1`.
pub const P: u64 = 0xFFFF_FFFF_0000_0001;
/// Two-adicity of `P - 1`.
pub const TWO_ADICITY: u32 = 32;
/// Multiplicative-group generator.
pub const GENERATOR: u64 = 7;

#[inline]
pub fn add(a: u64, b: u64) -> u64 {
    ((a as u128 + b as u128) % P as u128) as u64
}

#[inline]
pub fn sub(a: u64, b: u64) -> u64 {
    ((a as u128 + P as u128 - b as u128) % P as u128) as u64
}

#[inline]
pub fn mul(a: u64, b: u64) -> u64 {
    ((a as u128 * b as u128) % P as u128) as u64
}

/// Modular exponentiation.
pub fn pow_mod(a: u64, mut e: u64) -> u64 {
    let mut result = 1u64;
    let mut base = a % P;
    while e > 0 {
        if e & 1 == 1 {
            result = mul(result, base);
        }
        base = mul(base, base);
        e >>= 1;
    }
    result
}

/// Modular inverse by Fermat's little theorem; `None` for zero.
pub fn inv(a: u64) -> Option<u64> {
    if a.is_multiple_of(P) {
        None
    } else {
        Some(pow_mod(a, P - 2))
    }
}

/// A primitive `2**k`-th root of unity (`k <= 32`).
pub fn root_of_unity(k: u32) -> u64 {
    assert!(k <= TWO_ADICITY, "k out of range");
    pow_mod(GENERATOR, (P - 1) >> k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges() {
        assert_eq!(mul(P - 1, P - 1), 1);
        assert_eq!(add(P - 1, 1), 0);
        assert_eq!(sub(0, 1), P - 1);
    }

    #[test]
    fn roots_are_primitive() {
        for k in 1..=20u32 {
            let root = root_of_unity(k);
            let n = 1u64 << k;
            assert_eq!(pow_mod(root, n), 1);
            assert_eq!(pow_mod(root, n >> 1), P - 1);
        }
    }
}
