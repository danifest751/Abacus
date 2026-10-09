//! Abacus verifier self-test. Prints a one-line summary and exits non-zero on failure.
//!
//! This is a smoke test of the verifier only; it makes no claim about PoW, difficulty or security.

use abacus_verifier::{freivalds_verify_multi, matmul, Rng};

fn main() {
    let n = 64;
    let mut rng = Rng::new(0xABAC05);
    let a = rng.matrix(n);
    let b = rng.matrix(n);
    let c = matmul(&a, &b, n);

    let rs: Vec<Vec<u64>> = (0..4).map(|_| rng.vector(n)).collect();
    let ok_true = freivalds_verify_multi(&a, &b, &c, n, &rs);

    let mut bad = c.clone();
    bad[n * n - 1] = (bad[n * n - 1] + 1) % abacus_verifier::P;
    let ok_false = !freivalds_verify_multi(&a, &b, &bad, n, &rs);

    let ntt_ok = {
        let a: Vec<u64> = (0..16u64).map(|i| i * 7 + 1).collect();
        abacus_verifier::ntt::inverse(&abacus_verifier::ntt::forward(&a)) == a
    };
    let sc_ok = {
        let table: Vec<u64> = (0..64u64).map(|i| (i * i + 3) % abacus_verifier::goldilocks::P).collect();
        let (claimed, rounds) = abacus_verifier::sumcheck::prove(&table, 6);
        abacus_verifier::sumcheck::verify(&table, 6, claimed, &rounds)
    };

    if ok_true && ok_false && ntt_ok && sc_ok {
        println!("abacus-verifier self-test OK (Freivalds + NTT + sumcheck, n={n})");
    } else {
        eprintln!(
            "abacus-verifier self-test FAILED (freivalds_accept={ok_true}, freivalds_reject={ok_false}, ntt={ntt_ok}, sumcheck={sc_ok})"
        );
        std::process::exit(1);
    }
}
