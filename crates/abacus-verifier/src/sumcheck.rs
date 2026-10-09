//! Sumcheck protocol over the Goldilocks field.
//!
//! A multilinear polynomial `f(x_1..x_n)` is given by its Boolean-hypercube evaluations,
//! `table[i] = f(bits of i)` with bit `j` of `i` equal to `x_{j+1}` (LSB = variable 1). The
//! protocol proves `S = sum_{b in {0,1}^n} f(b)` with `O(n)` field elements; the verifier checks
//! the rounds and recomputes `f` at a random point from the table via the multilinear extension.

use crate::goldilocks as gl;
use crate::Rng;

pub fn full_sum(table: &[u64]) -> u64 {
    let mut acc: u128 = 0;
    for &x in table {
        acc += x as u128;
    }
    (acc % gl::P as u128) as u64
}

fn fold_first(table: &[u64], r: u64) -> Vec<u64> {
    let one_minus_r = gl::sub(1, r);
    (0..table.len() / 2)
        .map(|i| gl::add(gl::mul(table[2 * i], one_minus_r), gl::mul(table[2 * i + 1], r)))
        .collect()
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

/// Honest prover. Returns `(claimed_sum, rounds, challenges)` where `rounds[j] = (g_j(0), g_j(1))`.
pub fn prove(table: &[u64], n: usize, rng: &mut Rng) -> (u64, Vec<(u64, u64)>, Vec<u64>) {
    assert_eq!(table.len(), 1usize << n, "table shape");
    let mut t = table.to_vec();
    let claimed = full_sum(&t);
    let mut rounds = Vec::with_capacity(n);
    let mut rs = Vec::with_capacity(n);
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
        let r = rng.next_u64() % gl::P;
        rs.push(r);
        t = fold_first(&t, r);
    }
    (claimed, rounds, rs)
}

pub fn verify(table: &[u64], n: usize, claimed_sum: u64, rounds: &[(u64, u64)], rs: &[u64]) -> bool {
    if table.len() != 1usize << n || rounds.len() != n || rs.len() != n {
        return false;
    }
    let mut cur = claimed_sum % gl::P;
    for (&(g0, g1), &r) in rounds.iter().zip(rs.iter()) {
        if gl::add(g0, g1) != cur {
            return false;
        }
        cur = gl::add(gl::mul(g0, gl::sub(1, r)), gl::mul(g1, r));
    }
    eval_mle(table, n, rs) == cur
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rand_table(rng: &mut Rng, n: usize) -> Vec<u64> {
        (0..1usize << n).map(|_| rng.next_u64() % gl::P).collect()
    }

    #[test]
    fn completeness() {
        let mut rng = Rng::new(11);
        for n in 1..=8usize {
            let table = rand_table(&mut rng, n);
            let (claimed, rounds, rs) = prove(&table, n, &mut rng);
            assert!(verify(&table, n, claimed, &rounds, &rs));
        }
    }

    #[test]
    fn rejects_wrong_sum() {
        let mut rng = Rng::new(12);
        let n = 6usize;
        let table = rand_table(&mut rng, n);
        let (claimed, rounds, rs) = prove(&table, n, &mut rng);
        assert!(!verify(&table, n, gl::add(claimed, 1), &rounds, &rs));
    }

    #[test]
    fn rejects_tampered_round() {
        let mut rng = Rng::new(13);
        let n = 6usize;
        let table = rand_table(&mut rng, n);
        let (claimed, rounds, rs) = prove(&table, n, &mut rng);
        for j in 0..n {
            let mut bad = rounds.clone();
            bad[j].0 = gl::add(bad[j].0, 1);
            assert!(!verify(&table, n, claimed, &bad, &rs));
        }
    }

    #[test]
    fn eval_mle_bilinear() {
        let table = [0u64, 0, 0, 1]; // x1 * x2, x1 as LSB
        assert_eq!(eval_mle(&table, 2, &[2, 3]), 6);
        assert_eq!(full_sum(&table), 1);
    }
}
