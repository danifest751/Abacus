use abacus_chain::{p2p, Chain, Profile};
use std::net::TcpListener;
fn profile() -> Profile {
    Profile { n: 4, k: 4, bits: 5 }
}

fn serve_once(blocks: Vec<abacus_chain::Block>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            // Consume the client's SYNC hello, then serve the snapshot.
            let mut r = std::io::BufReader::new(s.try_clone().unwrap());
            let mut hello = String::new();
            let _ = std::io::BufRead::read_line(&mut r, &mut hello);
            p2p::serve_conn(&mut s, &blocks);
        }
    });
    addr
}

#[test]
fn peer_sync_converges() {
    // A mines 3 blocks.
    let mut a = Chain::new(profile(), [1u8; 32], 1);
    for i in 0..3u64 {
        a.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    let a_id = a.tip();

    // B syncs from A and adopts 3 blocks.
    let addr = serve_once(a.blocks.clone());
    let mut b = Chain::new(profile(), [1u8; 32], 1);
    let n = p2p::sync_from(&mut b, &addr.to_string()).unwrap();
    assert!(n >= 3);
    assert_eq!(b.height(), 3);
    assert_eq!(b.tip(), a_id);

    // B mines 2 more -> 5.
    for i in 3..5u64 {
        b.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    assert_eq!(b.height(), 5);

    // C syncs from B and adopts the longer chain.
    let addr = serve_once(b.blocks.clone());
    let mut c = Chain::new(profile(), [1u8; 32], 1);
    p2p::sync_from(&mut c, &addr.to_string()).unwrap();
    assert_eq!(c.height(), 5);
    assert_eq!(c.tip(), b.tip());
}

#[test]
fn sync_ignores_shorter_chain() {
    let mut a = Chain::new(profile(), [2u8; 32], 1);
    for i in 0..4u64 {
        a.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    // Peer serves only 1 block; A keeps its longer chain.
    let mut short = Chain::new(profile(), [2u8; 32], 1);
    short.mine_next(10, 1_000_000).unwrap();
    let addr = serve_once(short.blocks.clone());
    let adopted = p2p::sync_from(&mut a, &addr.to_string()).unwrap();
    assert_eq!(adopted, 0);
    assert_eq!(a.height(), 4);
}

#[test]
fn reorg_adopts_longer_branch() {
    // Common prefix of 2 blocks, shared by both nodes.
    let mut prefix = Chain::new(profile(), [3u8; 32], 1);
    for i in 0..2u64 {
        prefix.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    let mut a = prefix.clone();
    let mut b = prefix.clone();
    // Diverge: A extends by 2, B by 3 (different timestamps -> different instances).
    for i in 2..4u64 {
        a.mine_next((i + 1) * 10 + 1, 1_000_000).unwrap();
    }
    for i in 2..5u64 {
        b.mine_next((i + 1) * 10 + 2, 1_000_000).unwrap();
    }
    assert!(b.cumulative_work() > a.cumulative_work());

    let addr = serve_once(b.blocks.clone());
    let n = p2p::sync_from(&mut a, &addr.to_string()).unwrap();
    assert!(n >= 5);
    assert_eq!(a.height(), 5);
    assert_eq!(a.tip(), b.tip()); // A rolled back its 2 blocks and adopted B's branch
}

#[test]
fn sync_memory_hard_chain_with_same_dataset() {
    use abacus_chain::build_dataset;
    let p = Profile { n: 4, k: 4, bits: 6 };
    let ds = build_dataset(&[5u8; 32], 512);
    let mut a = Chain::new(p, [4u8; 32], 1).with_dataset(ds.clone());
    for i in 0..2u64 {
        a.mine_next((i + 1) * 10, 1_000_000).unwrap();
    }
    let addr = serve_once(a.blocks.clone());
    let mut b = Chain::new(p, [4u8; 32], 1).with_dataset(ds.clone());
    let n = p2p::sync_from(&mut b, &addr.to_string()).unwrap();
    assert!(n >= 2);
    assert_eq!(b.tip(), a.tip());
}

fn start_pool(p: Profile, id: [u8; 32]) -> (std::net::SocketAddr, std::sync::Arc<std::sync::Mutex<Chain>>, std::sync::Arc<p2p::PoolStats>) {
    use std::sync::{Arc, Mutex};
    let shared = Arc::new(Mutex::new(Chain::new(p, id, 1)));
    let stats = Arc::new(p2p::PoolStats::default());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (s2, st2) = (Arc::clone(&shared), Arc::clone(&stats));
    std::thread::spawn(move || p2p::serve_multi(listener, s2, st2));
    (addr, shared, stats)
}

fn rpc(w: &mut std::net::TcpStream, r: &mut std::io::BufReader<std::net::TcpStream>, line: &str) -> String {
    use std::io::{BufRead, Write};
    writeln!(w, "{line}").unwrap();
    w.flush().unwrap();
    let mut reply = String::new();
    r.read_line(&mut reply).unwrap();
    reply.trim_end().to_string()
}

#[test]
fn solo_job_submit_accepts_and_appends() {
    use abacus_chain::mine_from;
    use std::io::BufReader;
    use std::net::TcpStream;

    let p = Profile { n: 4, k: 4, bits: 6 };
    let (addr, shared, stats) = start_pool(p, [8u8; 32]);
    let stream = TcpStream::connect(addr).unwrap();
    let mut w = stream.try_clone().unwrap();
    let mut r = BufReader::new(stream);

    let job_line = rpc(&mut w, &mut r, "JOB");
    let job = p2p::decode_job(job_line.strip_prefix("JOB ").unwrap()).unwrap();
    let cp = Profile { n: 4, k: 4, bits: job.bits };
    let base = p2p::nonce_base(job.extranonce);
    let (nonce, c, _sc) =
        mine_from(&cp, &job.chain_id, job.version, job.height, &job.prev, job.timestamp, base, 1_000_000, None)
            .expect("mined a job");
    let reply = rpc(&mut w, &mut r, &format!("SUB {}", p2p::encode_sub(nonce, job.timestamp, &c)));
    assert!(reply.starts_with("OK"), "reply was {reply:?}");
    assert_eq!(shared.lock().unwrap().height(), 1);
    assert_eq!(stats.snapshot(), vec![(job.extranonce, 1, 0)]);
}

#[test]
fn pool_rejects_nonce_outside_extranonce_and_future_timestamp() {
    use abacus_chain::mine_from;
    use std::io::BufReader;
    use std::net::TcpStream;

    let p = Profile { n: 4, k: 4, bits: 4 };
    let (addr, shared, stats) = start_pool(p, [9u8; 32]);
    let stream = TcpStream::connect(addr).unwrap();
    let mut w = stream.try_clone().unwrap();
    let mut r = BufReader::new(stream);
    let job = p2p::decode_job(rpc(&mut w, &mut r, "JOB").strip_prefix("JOB ").unwrap()).unwrap();
    let cp = Profile { n: 4, k: 4, bits: job.bits };

    // A valid block, but in another miner's nonce range.
    let other = p2p::nonce_base(job.extranonce + 1);
    let (nonce, c, _) =
        mine_from(&cp, &job.chain_id, job.version, job.height, &job.prev, job.timestamp, other, 1_000_000, None).unwrap();
    assert_eq!(rpc(&mut w, &mut r, &format!("SUB {}", p2p::encode_sub(nonce, job.timestamp, &c))), "BAD");

    // A valid block in range, but with a timestamp far in the future.
    let far = p2p::now() + 10 * p2p::MAX_FUTURE_DRIFT;
    let base = p2p::nonce_base(job.extranonce);
    let (nonce, c, _) =
        mine_from(&cp, &job.chain_id, job.version, job.height, &job.prev, far, base, 1_000_000, None).unwrap();
    assert_eq!(rpc(&mut w, &mut r, &format!("SUB {}", p2p::encode_sub(nonce, far, &c))), "BAD");

    assert_eq!(shared.lock().unwrap().height(), 0);
    assert_eq!(stats.snapshot(), vec![(job.extranonce, 0, 2)]);
}

#[test]
fn per_miner_stats_are_separate() {
    use abacus_chain::mine_from;
    use std::io::BufReader;
    use std::net::TcpStream;

    let p = Profile { n: 4, k: 4, bits: 3 };
    let (addr, shared, stats) = start_pool(p, [10u8; 32]);
    let mut ens = Vec::new();
    for _ in 0..2 {
        let stream = TcpStream::connect(addr).unwrap();
        let mut w = stream.try_clone().unwrap();
        let mut r = BufReader::new(stream);
        let job = p2p::decode_job(rpc(&mut w, &mut r, "JOB").strip_prefix("JOB ").unwrap()).unwrap();
        let cp = Profile { n: 4, k: 4, bits: job.bits };
        let base = p2p::nonce_base(job.extranonce);
        let (nonce, c, _) =
            mine_from(&cp, &job.chain_id, job.version, job.height, &job.prev, job.timestamp, base, 1_000_000, None).unwrap();
        assert!(rpc(&mut w, &mut r, &format!("SUB {}", p2p::encode_sub(nonce, job.timestamp, &c))).starts_with("OK"));
        ens.push(job.extranonce);
    }
    assert_ne!(ens[0], ens[1]);
    assert_eq!(shared.lock().unwrap().height(), 2);
    assert_eq!(stats.snapshot(), vec![(ens[0], 1, 0), (ens[1], 1, 0)]);
}

#[test]
fn decode_rejects_trailing_bytes_and_bounded_reader_rejects_long_lines() {
    let mut a = Chain::new(profile(), [6u8; 32], 1);
    let blk = a.mine_next(10, 1_000_000).unwrap();
    let enc = p2p::encode_block(&blk);
    assert_eq!(p2p::decode_block(&enc), Some(blk));
    assert!(p2p::decode_block(&format!("{enc}00")).is_none());

    let long = "x".repeat(100) + "
";
    let mut r = std::io::BufReader::new(long.as_bytes());
    assert!(p2p::read_line_bounded(&mut r, 50).is_err());
    let mut r = std::io::BufReader::new("ok
".as_bytes());
    assert_eq!(p2p::read_line_bounded(&mut r, 50).unwrap().as_deref(), Some("ok"));
}

#[test]
fn sync_rejects_peer_block_with_wrong_c_length_without_panicking() {
    use abacus_chain::{block_id, preheader, score, Block};
    let tmpl = Chain::new(profile(), [7u8; 32], 1);
    let c = vec![0u64; 3];
    let bits = tmpl.next_bits();
    let ph = preheader(&tmpl.chain_id, 1, 0, &tmpl.tip(), 10, bits, 0);
    let bad = Block { height: 0, prev: tmpl.tip(), timestamp: 10, bits, nonce: 0, score: score(&ph, &c), id: block_id(&ph, &c), c };
    let addr = serve_once(vec![bad]);
    let mut b = tmpl.clone();
    assert_eq!(p2p::sync_from(&mut b, &addr.to_string()).unwrap(), 0);
    assert_eq!(b.height(), 0);
}

#[test]
fn sync_rejects_peer_chain_with_lowered_difficulty() {
    use abacus_chain::{block_id, mine, preheader, Block};
    // The peer serves a chain whose first block claims 1 bit while the profile requires 8.
    let tmpl = Chain::new(Profile { n: 4, k: 4, bits: 8 }, [8u8; 32], 1);
    let p = Profile { n: 4, k: 4, bits: 1 };
    let (nonce, c, sc) = mine(&p, &tmpl.chain_id, 1, 0, &tmpl.tip(), 10, 1000, None).unwrap();
    let ph = preheader(&tmpl.chain_id, 1, 0, &tmpl.tip(), 10, 1, nonce);
    let easy = Block { height: 0, prev: tmpl.tip(), timestamp: 10, bits: 1, nonce, id: block_id(&ph, &c), c, score: sc };
    let addr = serve_once(vec![easy]);
    let mut b = tmpl.clone();
    assert_eq!(p2p::sync_from(&mut b, &addr.to_string()).unwrap(), 0);
    assert_eq!(b.height(), 0);
}
