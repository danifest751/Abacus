//! Minimal P2P node for the prototype: listen, optionally sync from a peer, mine, serve.
//!
//! Args: --listen PORT  --peer HOST:PORT  --mine N  --bits B  --serve SEC  --dataset N  --n N
//!       --resync SEC (re-pull the peer every SEC seconds while serving; 0 = only at start)
//!
//! Run: cargo run --release --quiet --bin abacus-node -- --listen 9101 --mine 3 --bits 6 --serve 8

use abacus_chain::{build_dataset, mine_from, p2p, Chain, Profile};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Wire/encoding version of this prototype (v2: `bits` committed in the preheader, ADR 0010).
const VERSION: u32 = 2;
/// Nonces tried per lock-free mining round before re-reading the tip.
const MINE_BATCH: u64 = 4096;

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = std::env::args().collect();
    for i in 0..a.len() {
        if a[i] == name && i + 1 < a.len() {
            return a[i + 1].clone();
        }
    }
    default.to_string()
}

/// Mine one block without holding the chain lock during the search; returns its height.
fn mine_one(shared: &Mutex<Chain>, max_attempts: u64) -> Option<u64> {
    let mut tried = 0u64;
    let mut next_nonce = p2p::nonce_base(0); // extranonce 0 is reserved for the node's own miner
    while tried < max_attempts {
        let (profile, chain_id, version, tpl, dataset) = {
            let c = shared.lock().unwrap();
            (c.profile, c.chain_id, c.version, c.template(), c.dataset.clone())
        };
        let ts = p2p::now().max(tpl.min_timestamp);
        let p = Profile { n: profile.n, k: profile.k, bits: tpl.bits };
        let ds = dataset.as_deref().map(|v| v.as_slice());
        let batch = MINE_BATCH.min(max_attempts - tried);
        let found = mine_from(&p, &chain_id, version, tpl.height, &tpl.prev, ts, next_nonce, batch, ds);
        tried += batch;
        next_nonce += batch;
        if let Some((nonce, c, sc)) = found {
            let ph = abacus_chain::preheader(&chain_id, version, tpl.height, &tpl.prev, ts, tpl.bits, nonce);
            let block = abacus_chain::Block {
                height: tpl.height,
                prev: tpl.prev,
                timestamp: ts,
                bits: tpl.bits,
                nonce,
                id: abacus_chain::block_id(&ph, &c),
                c,
                score: sc,
            };
            let mut ch = shared.lock().unwrap();
            if ch.append_checked(block) {
                return Some(ch.height() - 1);
            }
            // The tip moved (a miner submitted first); retry on the new tip.
        }
    }
    None
}

fn main() {
    let listen: u16 = arg("--listen", "9101").parse().unwrap();
    let peer = arg("--peer", "");
    let mine_n: u64 = arg("--mine", "0").parse().unwrap();
    let bits: u32 = arg("--bits", "6").parse().unwrap();
    let serve_s: u64 = arg("--serve", "0").parse().unwrap();
    let dataset_n: usize = arg("--dataset", "0").parse().unwrap();
    let n: usize = arg("--n", "8").parse().unwrap();
    let resync_s: u64 = arg("--resync", "0").parse().unwrap();

    let profile = Profile { n, k: 8, bits };
    let chain_id = [0xABu8; 32];
    let mut chain = Chain::new(profile, chain_id, VERSION);
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
    let stats = Arc::new(p2p::PoolStats::default());
    let listener = TcpListener::bind(("127.0.0.1", listen)).expect("listen");
    {
        let (server, st) = (Arc::clone(&shared), Arc::clone(&stats));
        std::thread::spawn(move || p2p::serve_multi(listener, server, st));
    }

    for _ in 0..mine_n {
        match mine_one(&shared, 10_000_000) {
            Some(h) => {
                let bits = shared.lock().unwrap().blocks[h as usize].bits;
                eprintln!("[node] mined height {h} bits {bits}");
            }
            None => {
                eprintln!("[node] mining failed");
                break;
            }
        }
    }

    let deadline = Instant::now() + Duration::from_secs(serve_s);
    while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        let step = if resync_s > 0 { Duration::from_secs(resync_s).min(left) } else { left };
        std::thread::sleep(step);
        if resync_s > 0 && !peer.is_empty() {
            // Fetch and validate without the lock; adopt under the lock.
            let template = shared.lock().unwrap().empty_like();
            match p2p::fetch_chain(&template, &peer) {
                Ok(Some(fresh)) => {
                    let h = p2p::adopt_if_better(&mut shared.lock().unwrap(), fresh);
                    if h > 0 {
                        eprintln!("[node] resync adopted {h} blocks from {peer}");
                    }
                }
                Ok(None) => eprintln!("[node] resync: invalid chain from {peer}"),
                Err(e) => eprintln!("[node] resync error: {e}"),
            }
        }
    }

    let c = shared.lock().unwrap();
    let tip_hex: String = c.tip().iter().map(|b| format!("{b:02x}")).collect();
    let miners: Vec<String> = stats
        .snapshot()
        .iter()
        .map(|m| {
            format!(
                "{{\"extranonce\": {}, \"accepted\": {}, \"stale\": {}, \"rejected\": {}}}",
                m.extranonce, m.accepted, m.stale, m.rejected
            )
        })
        .collect();
    println!(
        "{{\"listen\": {listen}, \"height\": {}, \"work\": {}, \"tip\": \"{}\", \"miners\": [{}]}}",
        c.height(),
        c.cumulative_work(),
        tip_hex,
        miners.join(", ")
    );
}
