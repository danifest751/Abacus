//! Number-theoretic transform over the Goldilocks field (iterative Cooley-Tukey).

use crate::goldilocks as gl;

fn bit_reverse(a: &mut [u64]) {
    let n = a.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            a.swap(i, j);
        }
    }
}

fn transform(a: &mut [u64], inverse: bool) {
    let n = a.len();
    assert!(n > 0 && n.is_power_of_two(), "length must be a power of two");
    let k = n.trailing_zeros();
    assert!(k <= gl::TWO_ADICITY, "length exceeds 2-adicity");
    bit_reverse(a);
    let mut length = 2usize;
    while length <= n {
        let mut wlen = gl::pow_mod(gl::GENERATOR, (gl::P - 1) / length as u64);
        if inverse {
            wlen = gl::inv(wlen).expect("nonzero root");
        }
        let half = length >> 1;
        let mut start = 0usize;
        while start < n {
            let mut w = 1u64;
            for j in start..start + half {
                let u = a[j];
                let v = gl::mul(a[j + half], w);
                a[j] = gl::add(u, v);
                a[j + half] = gl::sub(u, v);
                w = gl::mul(w, wlen);
            }
            start += length;
        }
        length <<= 1;
    }
    if inverse {
        let n_inv = gl::inv(n as u64 % gl::P).expect("nonzero n");
        for x in a.iter_mut() {
            *x = gl::mul(*x, n_inv);
        }
    }
}

pub fn forward(a: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = a.iter().map(|&x| x % gl::P).collect();
    transform(&mut out, false);
    out
}

pub fn inverse(a: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = a.iter().map(|&x| x % gl::P).collect();
    transform(&mut out, true);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rng;

    #[test]
    fn round_trip() {
        let mut rng = Rng::new(3);
        for k in 1..=10u32 {
            let n = 1usize << k;
            let a: Vec<u64> = (0..n).map(|_| rng.residue()).collect();
            assert_eq!(inverse(&forward(&a)), a);
        }
    }

    #[test]
    fn is_linear() {
        let mut rng = Rng::new(5);
        let n = 16usize;
        let a: Vec<u64> = (0..n).map(|_| rng.residue()).collect();
        let b: Vec<u64> = (0..n).map(|_| rng.residue()).collect();
        let fa = forward(&a);
        let fb = forward(&b);
        let sum: Vec<u64> = a.iter().zip(b.iter()).map(|(x, y)| gl::add(*x, *y)).collect();
        let fsum = forward(&sum);
        for i in 0..n {
            assert_eq!(fsum[i], gl::add(fa[i], fb[i]));
        }
    }
}
