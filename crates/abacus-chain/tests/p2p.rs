use abacus_chain::{p2p, Chain, Profile};
use std::net::TcpListener;

fn profile() -> Profile {
    Profile { n: 4, k: 4, bits: 5 }
}

fn serve_once(blocks: Vec<abacus_chain::Block>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((s, _)) = listener.accept() {
            p2p::serve_conn(s, &blocks);
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
