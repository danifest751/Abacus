//! Reference CPU prover for the interactive proof of tensor throughput (spec/06). Slow; for tests and
//! protocol checks at small `n`. The GPU prover is `cuda/attest_prover.cu`.
//!
//! Args: --listen PORT (default 9600)  --cheat-rows C (zero the last C rows of every product)
//!       --once (serve one connection and exit)

use abacus_attest::net::serve_cpu;
use std::net::TcpListener;

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned()).unwrap_or_else(|| default.to_string())
}

fn main() {
    let port: u16 = arg("--listen", "9600").parse().expect("--listen");
    let cheat: usize = arg("--cheat-rows", "0").parse().expect("--cheat-rows");
    let once = std::env::args().any(|a| a == "--once");
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    for s in listener.incoming().flatten() {
        let _ = serve_cpu(s, cheat);
        if once {
            break;
        }
    }
}
