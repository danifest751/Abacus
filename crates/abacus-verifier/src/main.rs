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

    if ok_true && ok_false {
        println!("abacus-verifier self-test OK (n={n}, Freivalds accept/reject)");
    } else {
        eprintln!("abacus-verifier self-test FAILED (accept={ok_true}, reject={ok_false})");
        std::process::exit(1);
    }
}
