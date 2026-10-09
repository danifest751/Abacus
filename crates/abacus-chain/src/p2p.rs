//! Minimal line-protocol P2P for the prototype: serve a chain, sync from a peer.
//!
//! No third-party crates. A block is hex-encoded with a fixed layout. A server writes `B <hex>` for
//! each block (tip to genesis order irrelevant; we send oldest-first) then `E`. A client pulls all
//! blocks, validates them into a fresh chain and adopts it only if it has strictly greater cumulative
//! work. This is a pull-based sync; no gossip, no peer discovery.

use crate::{block_id, preheader, score, Block, Chain, Profile};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub fn serve_conn(stream: &mut TcpStream, blocks: &[Block]) {
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
            std::thread::spawn(move || {
                let mut s = s;
                serve_conn(&mut s, &blocks);
            });
        }
    }
}

/// Pull a peer's chain and adopt it if it is valid and has strictly greater cumulative work.
pub fn sync_from(chain: &mut Chain, addr: &str) -> std::io::Result<usize> {
    let stream = TcpStream::connect(addr)?;
    {
        let mut w = stream.try_clone()?;
        let _ = writeln!(w, "SYNC");
        let _ = w.flush();
    }
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

// ---------------- solo job/submit protocol ----------------

pub struct Job {
    pub chain_id: [u8; 32],
    pub version: u32,
    pub prev: [u8; 32],
    pub height: u64,
    pub timestamp: u64,
    pub bits: u32,
    pub extranonce: u64,
}

pub fn encode_job(j: &Job) -> String {
    let mut v = Vec::new();
    v.extend_from_slice(&j.chain_id);
    v.extend_from_slice(&j.version.to_le_bytes());
    v.extend_from_slice(&j.prev);
    v.extend_from_slice(&j.height.to_le_bytes());
    v.extend_from_slice(&j.timestamp.to_le_bytes());
    v.extend_from_slice(&j.bits.to_le_bytes());
    v.extend_from_slice(&j.extranonce.to_le_bytes());
    hex(&v)
}

pub fn decode_job(s: &str) -> Option<Job> {
    let v = unhex(s)?;
    if v.len() != 96 {
        return None;
    }
    let mut chain_id = [0u8; 32];
    chain_id.copy_from_slice(&v[0..32]);
    let version = u32::from_le_bytes(v[32..36].try_into().ok()?);
    let mut prev = [0u8; 32];
    prev.copy_from_slice(&v[36..68]);
    let height = u64::from_le_bytes(v[68..76].try_into().ok()?);
    let timestamp = u64::from_le_bytes(v[76..84].try_into().ok()?);
    let bits = u32::from_le_bytes(v[84..88].try_into().ok()?);
    let extranonce = u64::from_le_bytes(v[88..96].try_into().ok()?);
    Some(Job { chain_id, version, prev, height, timestamp, bits, extranonce })
}

pub fn encode_sub(nonce: u64, timestamp: u64, c: &[u64]) -> String {
    let mut v = Vec::new();
    v.extend_from_slice(&nonce.to_le_bytes());
    v.extend_from_slice(&timestamp.to_le_bytes());
    v.extend_from_slice(&(c.len() as u32).to_le_bytes());
    for &x in c {
        v.extend_from_slice(&x.to_le_bytes());
    }
    hex(&v)
}

pub fn decode_sub(s: &str) -> Option<(u64, u64, Vec<u64>)> {
    let v = unhex(s)?;
    if v.len() < 20 {
        return None;
    }
    let nonce = u64::from_le_bytes(v[0..8].try_into().ok()?);
    let timestamp = u64::from_le_bytes(v[8..16].try_into().ok()?);
    let clen = u32::from_le_bytes(v[16..20].try_into().ok()?) as usize;
    if v.len() != 20 + clen * 8 {
        return None;
    }
    let mut c = Vec::with_capacity(clen);
    for k in 0..clen {
        c.push(u64::from_le_bytes(v[20 + k * 8..28 + k * 8].try_into().ok()?));
    }
    Some((nonce, timestamp, c))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Handle one connection: `SYNC` (serve chain, then close), or repeated `JOB`/`SUB <hex>` on the
/// same connection (send a job, accept a block). `extranonce` is this miner's nonce-space offset.
pub fn handle_conn(stream: TcpStream, chain: &Mutex<Chain>, extranonce: u64, shares: &AtomicU64) {
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut w = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let t = line.trim_end().to_string();
        if t == "SYNC" {
            let c = chain.lock().unwrap();
            serve_conn(&mut w, &c.blocks);
            break;
        } else if t == "JOB" {
            let job = {
                let c = chain.lock().unwrap();
                Job {
                    chain_id: c.chain_id,
                    version: c.version,
                    prev: c.tip(),
                    height: c.height(),
                    timestamp: now(),
                    bits: c.next_bits(),
                    extranonce,
                }
            };
            let _ = writeln!(w, "JOB {}", encode_job(&job));
        } else if let Some(rest) = t.strip_prefix("SUB ") {
            match decode_sub(rest) {
                Some((nonce, timestamp, c_vec)) => {
                    let mut c = chain.lock().unwrap();
                    if c_vec.len() != c.profile.n * c.profile.n {
                        let _ = writeln!(w, "BAD");
                        continue;
                    }
                    let height = c.height();
                    let prev = c.tip();
                    let bits = c.next_bits();
                    let ph = preheader(&c.chain_id, c.version, height, &prev, timestamp, nonce);
                    let block = Block {
                        height,
                        prev,
                        timestamp,
                        nonce,
                        c: c_vec.clone(),
                        score: score(&ph, &c_vec),
                        id: block_id(&ph, &c_vec),
                        bits,
                    };
                    if c.append_checked(block) {
                        shares.fetch_add(1, Ordering::Relaxed);
                        let _ = writeln!(w, "OK {}", c.height());
                    } else {
                        let _ = writeln!(w, "BAD");
                    }
                }
                None => {
                    let _ = writeln!(w, "BAD");
                }
            }
        } else {
            break;
        }
    }
    let _ = w.flush();
}

/// Accept connections forever; assign each a distinct extranonce and count accepted shares.
pub fn serve_multi(listener: TcpListener, chain: std::sync::Arc<Mutex<Chain>>) {
    let next = AtomicU64::new(1);
    let shares = std::sync::Arc::new(AtomicU64::new(0));
    for stream in listener.incoming() {
        if let Ok(s) = stream {
            let c = std::sync::Arc::clone(&chain);
            let en = next.fetch_add(1, Ordering::Relaxed);
            let sh = std::sync::Arc::clone(&shares);
            std::thread::spawn(move || handle_conn(s, &c, en, &sh));
        }
    }
}
