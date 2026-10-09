//! Sumcheck protocol over the Goldilocks field, non-interactive (Fiat–Shamir).
//!
//! A multilinear polynomial `f(x_1..x_n)` is given by its Boolean-hypercube evaluations,
//! `table[i] = f(bits of i)` with bit `j` of `i` equal to `x_{j+1}` (LSB = variable 1). The
//! protocol proves `S = sum_{b in {0,1}^n} f(b)` with `O(n)` field elements. Challenges are derived
//! by the **verifier** from a running transcript hash (table, `n`, claimed sum, every round so far),
//! never supplied by the prover: a verifier that accepts prover-chosen challenges is forgeable
//! (any claimed sum passes with `r_j = 0`). The verifier here recomputes `f(r)` from the full table
//! via the multilinear extension, so it is `O(2^n)` — a building block, not a succinct verifier.

use crate::goldilocks as gl;
use crate::sha256::sha256;

pub const DOM_SUMCHECK: &[u8] = b"abacus/sumcheck";

pub fn full_sum(table: &[u64]) -> u64 {
    let mut acc: u128 = 0;
    for &x in table {
        acc += x as u128;
    }
    (acc % gl::P as u128) as u64
}

fn fold_first(table: &[u64], r: u64) -> Vec<u64> {
    let one_minus_r = gl::sub(1, r);
    (0..table.len() / 2).map(|i| gl::add(gl::mul(table[2 * i], one_minus_r), gl::mul(table[2 * i + 1], r))).collect()
}

pub fn eval_mle(table: &[u64], n: usize, rs: &[u64]) -> u64 {
    assert_eq!(table.len(), 1usize << n, "table shape");
    assert_eq!(rs.len(), n, "challenge shape");
    let mut t = table.to_vec();
    for &r in rs {
        t = fold_first(&t, r);
    }
    t[0]
}

/// Initial transcript state: `SHA256(domain || n || len || table || claimed)` (all LE u64).
pub fn transcript_init(table: &[u64], n: usize, claimed: u64) -> [u8; 32] {
    let mut m = Vec::with_capacity(DOM_SUMCHECK.len() + 24 + table.len() * 8);
    m.extend_from_slice(DOM_SUMCHECK);
    m.extend_from_slice(&(n as u64).to_le_bytes());
    m.extend_from_slice(&(table.len() as u64).to_le_bytes());
    for &x in table {
        m.extend_from_slice(&x.to_le_bytes());
    }
    m.extend_from_slice(&claimed.to_le_bytes());
    sha256(&m)
}

/// Absorb one round `(g0, g1)`; returns the new state and the round challenge.
pub fn transcript_round(state: &[u8; 32], g0: u64, g1: u64) -> ([u8; 32], u64) {
    let mut m = Vec::with_capacity(48);
    m.extend_from_slice(state);
    m.extend_from_slice(&g0.to_le_bytes());
    m.extend_from_slice(&g1.to_le_bytes());
    let next = sha256(&m);
    let mut e = [0u8; 8];
    e.copy_from_slice(&next[0..8]);
    (next, u64::from_le_bytes(e) % gl::P)
}

/// Fiat–Shamir challenges for a transcript (what the verifier recomputes).
pub fn challenges(table: &[u64], n: usize, claimed: u64, rounds: &[(u64, u64)]) -> Vec<u64> {
    let mut state = transcript_init(table, n, claimed);
    rounds
        .iter()
        .map(|&(g0, g1)| {
            let (s, r) = transcript_round(&state, g0, g1);
            state = s;
            r
        })
        .collect()
}

/// Honest prover. Returns `(claimed_sum, rounds)` where `rounds[j] = (g_j(0), g_j(1))`.
pub fn prove(table: &[u64], n: usize) -> (u64, Vec<(u64, u64)>) {
    assert_eq!(table.len(), 1usize << n, "table shape");
    let mut t = table.to_vec();
    let claimed = full_sum(&t);
    let mut state = transcript_init(table, n, claimed);
    let mut rounds = Vec::with_capacity(n);
    for _ in 0..n {
        let mut g0: u128 = 0;
        let mut g1: u128 = 0;
        for (i, &x) in t.iter().enumerate() {
            if i % 2 == 0 {
                g0 += x as u128;
            } else {
                g1 += x as u128;
            }
        }
        let (g0, g1) = ((g0 % gl::P as u128) as u64, (g1 % gl::P as u128) as u64);
        rounds.push((g0, g1));
        let (s, r) = transcript_round(&state, g0, g1);
        state = s;
        t = fold_first(&t, r);
    }
    (claimed, rounds)
}

/// Verify a transcript against the table; the challenges are recomputed, not taken from the prover.
pub fn verify(table: &[u64], n: usize, claimed_sum: u64, rounds: &[(u64, u64)]) -> bool {
    if n >= usize::BITS as usize || table.len() != 1usize << n || rounds.len() != n || claimed_sum >= gl::P {
        return false;
    }
    if rounds.iter().any(|&(g0, g1)| g0 >= gl::P || g1 >= gl::P) {
        return false;
    }
    let rs = challenges(table, n, claimed_sum, rounds);
    let mut cur = claimed_sum;
    for (&(g0, g1), &r) in rounds.iter().zip(rs.iter()) {
        if gl::add(g0, g1) != cur {
            return false;
        }
        cur = gl::add(gl::mul(g0, gl::sub(1, r)), gl::mul(g1, r));
    }
    eval_mle(table, n, &rs) == cur
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Rng;

    fn rand_table(rng: &mut Rng, n: usize) -> Vec<u64> {
        (0..1usize << n).map(|_| rng.next_u64() % gl::P).collect()
    }

    #[test]
    fn completeness() {
        let mut rng = Rng::new(11);
        for n in 1..=8usize {
            let table = rand_table(&mut rng, n);
            let (claimed, rounds) = prove(&table, n);
            assert!(verify(&table, n, claimed, &rounds));
        }
    }

    #[test]
    fn rejects_wrong_sum() {
        let mut rng = Rng::new(12);
        let n = 6usize;
        let table = rand_table(&mut rng, n);
        let (claimed, rounds) = prove(&table, n);
        assert!(!verify(&table, n, gl::add(claimed, 1), &rounds));
    }

    #[test]
    fn rejects_tampered_round() {
        let mut rng = Rng::new(13);
        let n = 6usize;
        let table = rand_table(&mut rng, n);
        let (claimed, rounds) = prove(&table, n);
        for j in 0..n {
            let mut bad = rounds.clone();
            bad[j].0 = gl::add(bad[j].0, 1);
            assert!(!verify(&table, n, claimed, &bad));
        }
    }

    #[test]
    fn rejects_zero_challenge_forgery() {
        // With prover-chosen challenges r_j = 0 any claimed sum can be made to pass; the verifier
        // must recompute the challenges, so this transcript is rejected.
        let mut rng = Rng::new(14);
        let n = 4usize;
        let table = rand_table(&mut rng, n);
        let fake = gl::add(full_sum(&table), 12345);
        let mut cur = fake;
        let mut t = table.clone();
        let mut rounds = Vec::new();
        for _ in 0..n {
            let g0 = full_sum(&t.iter().step_by(2).copied().collect::<Vec<_>>());
            rounds.push((g0, gl::sub(cur, g0)));
            cur = g0;
            t = t.iter().step_by(2).copied().collect();
        }
        assert!(!verify(&table, n, fake, &rounds));
    }

    #[test]
    fn eval_mle_bilinear() {
        let table = [0u64, 0, 0, 1]; // x1 * x2, x1 as LSB
        assert_eq!(eval_mle(&table, 2, &[2, 3]), 6);
        assert_eq!(full_sum(&table), 1);
    }
}
