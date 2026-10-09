use abacus_attest::net::{serve_cpu, verify_round};
use std::net::TcpListener;

fn prover(cheat_rows: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        if let Ok((s, _)) = listener.accept() {
            let _ = serve_cpu(s, cheat_rows);
        }
    });
    addr
}

#[test]
fn honest_prover_is_accepted() {
    let r = verify_round(&prover(0), 32, 3, 16, &[1u8; 32], &[2u8; 32]).unwrap();
    assert!(r.accepted, "{r:?}");
    assert_eq!((r.rows_ok, r.rows_opened), (16, 16));
}

#[test]
fn prover_skipping_half_the_rows_is_caught() {
    // 16 of 32 rows of every product are zeros: with 16 openings the pass probability is ~1e-5.
    let r = verify_round(&prover(16), 32, 3, 16, &[3u8; 32], &[4u8; 32]).unwrap();
    assert!(!r.accepted, "{r:?}");
}

#[test]
fn detection_rate_matches_sampling_bound() {
    // A prover zeroing 4 of 32 rows (f = 1/8) with k = 4 openings passes with probability ~0.57;
    // over 60 independent rounds the empirical pass rate must be close to it.
    let rounds = 60;
    let mut passed = 0;
    for t in 0..rounds as u8 {
        let r = verify_round(&prover(4), 32, 1, 4, &[t; 32], &[t.wrapping_add(100); 32]).unwrap();
        passed += r.accepted as usize;
    }
    let expect = abacus_attest::pass_probability(4.0 / 32.0, 32, 4);
    let rate = passed as f64 / rounds as f64;
    assert!((rate - expect).abs() < 0.2, "pass rate {rate}, expected {expect}");
    assert!(passed < rounds, "a cheating prover must be caught sometimes");
}
