//! Minimal line-protocol P2P for the prototype: serve a chain, sync from a peer.
//!
//! No third-party crates. A block is hex-encoded with a fixed layout. A server writes `B <hex>` for
//! each block (tip to genesis order irrelevant; we send oldest-first) then `E`. A client pulls all
//! blocks, validates them into a fresh chain and adopts it only if it has strictly greater cumulative
//! work. This is a pull-based sync; no gossip, no peer discovery.

use crate::{Block, Chain, Profile};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn put_u64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    for i in (0..b.len()).step_by(2) {
        out.push((hexval(b[i])? << 4) | hexval(b[i + 1])?);
    }
    Some(out)
}

pub fn encode_block(blk: &Block) -> String {
    let mut v = Vec::new();
    put_u64(&mut v, blk.height);
    v.extend_from_slice(&blk.prev);
    put_u64(&mut v, blk.timestamp);
    put_u64(&mut v, blk.nonce);
    v.extend_from_slice(&blk.bits.to_le_bytes());
    v.extend_from_slice(&(blk.c.len() as u32).to_le_bytes());
    for &x in &blk.c {
        put_u64(&mut v, x);
    }
    v.extend_from_slice(&blk.score);
    v.extend_from_slice(&blk.id);
    hex(&v)
}

pub fn decode_block(s: &str) -> Option<Block> {
    let v = unhex(s)?;
    let mut o = 0usize;
    let rd_u64 = |v: &[u8], o: &mut usize| -> Option<u64> {
        if *o + 8 > v.len() {
            return None;
        }
        let mut e = [0u8; 8];
        e.copy_from_slice(&v[*o..*o + 8]);
        *o += 8;
        Some(u64::from_le_bytes(e))
    };
    let height = rd_u64(&v, &mut o)?;
    if o + 32 > v.len() {
        return None;
    }
    let mut prev = [0u8; 32];
    prev.copy_from_slice(&v[o..o + 32]);
    o += 32;
    let timestamp = rd_u64(&v, &mut o)?;
    let nonce = rd_u64(&v, &mut o)?;
    if o + 8 > v.len() {
        return None;
    }
    let mut e4 = [0u8; 4];
    e4.copy_from_slice(&v[o..o + 4]);
    let bits = u32::from_le_bytes(e4);
    o += 4;
    e4.copy_from_slice(&v[o..o + 4]);
    let clen = u32::from_le_bytes(e4) as usize;
    o += 4;
    if o + clen * 8 + 64 > v.len() {
        return None;
    }
    let mut c = Vec::with_capacity(clen);
    for _ in 0..clen {
        c.push(rd_u64(&v, &mut o)?);
    }
    let mut score = [0u8; 32];
    score.copy_from_slice(&v[o..o + 32]);
    o += 32;
    let mut id = [0u8; 32];
    id.copy_from_slice(&v[o..o + 32]);
    Some(Block { height, prev, timestamp, nonce, c, score, id, bits })
}

/// Serve a snapshot of `blocks` to one connection (oldest first) and close.
pub fn serve_conn(mut stream: TcpStream, blocks: &[Block]) {
    for b in blocks {
        let _ = writeln!(stream, "B {}", encode_block(b));
    }
    let _ = writeln!(stream, "E");
    let _ = stream.flush();
}

/// Accept connections forever, serving a snapshot each time. `get_blocks` returns the current chain.
pub fn serve_loop<F: Fn() -> Vec<Block> + Send + 'static>(listener: TcpListener, get_blocks: F) {
    for stream in listener.incoming() {
        if let Ok(s) = stream {
            let blocks = get_blocks();
            std::thread::spawn(move || serve_conn(s, &blocks));
        }
    }
}

/// Pull a peer's chain and adopt it if it is valid and has strictly greater cumulative work.
pub fn sync_from(chain: &mut Chain, addr: &str) -> std::io::Result<usize> {
    let stream = TcpStream::connect(addr)?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut fresh = Chain::new(
        Profile { n: chain.profile.n, k: chain.profile.k, bits: chain.profile.bits },
        chain.chain_id,
        chain.version,
    );
    fresh.dataset = chain.dataset.clone();

    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let t = line.trim_end();
        if t == "E" {
            break;
        }
        if let Some(rest) = t.strip_prefix("B ") {
            match decode_block(rest) {
                Some(blk) => {
                    if !fresh.append_checked(blk) {
                        return Ok(0); // invalid peer chain; ignore
                    }
                }
                None => return Ok(0),
            }
        }
    }

    if fresh.cumulative_work() > chain.cumulative_work() {
        chain.blocks = fresh.blocks;
        Ok(chain.height() as usize)
    } else {
        Ok(0)
    }
}
