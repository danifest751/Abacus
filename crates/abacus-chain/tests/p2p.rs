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

#[test]
fn solo_job_submit_accepts_and_appends() {
    use abacus_chain::mine;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};

    let p = Profile { n: 4, k: 4, bits: 6 };
    let chain = Chain::new(p, [8u8; 32], 1);
    let shared = Arc::new(Mutex::new(chain));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let s2 = Arc::clone(&shared);
    std::thread::spawn(move || p2p::serve_multi(listener, s2));

    let stream = TcpStream::connect(addr).unwrap();
    let mut w = stream.try_clone().unwrap();
    let mut r = BufReader::new(stream);

    writeln!(w, "JOB").unwrap();
    w.flush().unwrap();
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let job = p2p::decode_job(line.trim_end().strip_prefix("JOB ").unwrap()).unwrap();

    let cp = Profile { n: 4, k: 4, bits: job.bits };
    let (nonce, c, _sc) = mine(&cp, &job.chain_id, job.version, job.height, &job.prev, job.timestamp, 1_000_000, None)
        .expect("mined a job");
    writeln!(w, "SUB {}", p2p::encode_sub(nonce, job.timestamp, &c)).unwrap();
    w.flush().unwrap();

    let mut reply = String::new();
    r.read_line(&mut reply).unwrap();
    assert!(reply.trim_end().starts_with("OK"), "reply was {reply:?}");
    assert_eq!(shared.lock().unwrap().height(), 1);
}
