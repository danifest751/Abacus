//! Minimal P2P node for the prototype: listen, optionally sync from a peer, mine, serve.
//!
//! Args: --listen PORT  --peer HOST:PORT  --mine N  --bits B  --serve SEC  --dataset N
//!
//! Run: cargo run --release --quiet --bin abacus-node -- --listen 9101 --mine 3 --bits 6 --serve 8

use abacus_chain::{build_dataset, p2p, Chain, Profile};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = std::env::args().collect();
    for i in 0..a.len() {
        if a[i] == name && i + 1 < a.len() {
            return a[i + 1].clone();
        }
    }
    default.to_string()
}

fn main() {
    let listen: u16 = arg("--listen", "9101").parse().unwrap();
    let peer = arg("--peer", "");
    let mine_n: u64 = arg("--mine", "0").parse().unwrap();
    let bits: u32 = arg("--bits", "6").parse().unwrap();
    let serve_s: u64 = arg("--serve", "0").parse().unwrap();
    let dataset_n: usize = arg("--dataset", "0").parse().unwrap();

    let profile = Profile { n: 8, k: 8, bits };
    let chain_id = [0xABu8; 32];
    let mut chain = Chain::new(profile, chain_id, 1);
    if dataset_n > 0 {
        chain = chain.with_dataset(build_dataset(&[7u8; 32], dataset_n));
    }

    if !peer.is_empty() {
        match p2p::sync_from(&mut chain, &peer) {
            Ok(n) => eprintln!("[node] synced {n} blocks from {peer}"),
            Err(e) => eprintln!("[node] sync error: {e}"),
        }
    }

    let shared = Arc::new(Mutex::new(chain));
    let server = Arc::clone(&shared);
    let listener = TcpListener::bind(("127.0.0.1", listen)).expect("listen");
    std::thread::spawn(move || {
        p2p::serve_loop(listener, move || {
            let c = server.lock().unwrap();
            c.blocks.clone()
        })
    });

    for i in 0..mine_n {
        let ts = (i + 1) * 10;
        let mut c = shared.lock().unwrap();
        match c.mine_next(ts, 10_000_000) {
            Some(b) => eprintln!("[node] mined height {} bits {}", b.height, b.bits),
            None => {
                eprintln!("[node] mining failed");
                break;
            }
        }
    }

    if serve_s > 0 {
        std::thread::sleep(Duration::from_secs(serve_s));
    }

    let c = shared.lock().unwrap();
    let tip = c.tip();
    let tip_hex: String = tip.iter().map(|b| format!("{b:02x}")).collect();
    println!(
        "{{\"listen\": {listen}, \"height\": {}, \"work\": {:.0}, \"tip\": \"{}\"}}",
        c.height(),
        c.cumulative_work(),
        tip_hex
    );
}
