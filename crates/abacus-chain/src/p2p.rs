//! Minimal line-protocol P2P for the prototype: serve a chain, sync from a peer, solo/pool mining.
//!
//! No third-party crates. A block is hex-encoded with a fixed layout. A server writes `B <hex>` for
//! each block (oldest first) then `E`. A client pulls all blocks, validates them into a fresh chain
//! and adopts it only if it has strictly greater cumulative work. Pull-based; no gossip, no peer
//! discovery.
//!
//! Resource bounds (THREAT-MODEL §6): every line is read with a hard length cap derived from the
//! profile, sockets have read timeouts, the number of concurrent connections is capped, and the
//! chain lock is never held while writing to the network.

use crate::{block_id, preheader, score, Block, Chain};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A submitted timestamp may be at most this many seconds ahead of the node's clock.
pub const MAX_FUTURE_DRIFT: u64 = 120;
/// Maximum number of concurrently served connections.
pub const MAX_CONNS: usize = 64;
/// Socket read timeout for peers and miners.
pub const READ_TIMEOUT: Duration = Duration::from_secs(300);
/// Maximum number of blocks accepted from one peer snapshot.
pub const MAX_SYNC_BLOCKS: usize = 1 << 20;

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

/// Encoded size in bytes of a block whose `C` has `clen` entries.
pub fn block_bytes(clen: usize) -> usize {
    8 + 32 + 8 + 4 + 8 + 4 + clen * 8 + 32 + 32
}

/// Longest protocol line a node with matrix size `n` accepts (a `B <hex>` block line).
pub fn max_line(n: usize) -> usize {
    2 + 2 * block_bytes(n * n) + 2
}

/// Read one line of at most `max` bytes (newline included). Returns `Ok(None)` at EOF and an error
/// for an over-long line, so a peer cannot grow the buffer without bound.
pub fn read_line_bounded<R: BufRead>(r: &mut R, max: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let got = r.by_ref().take(max as u64 + 1).read_until(b'\n', &mut buf)?;
    if got == 0 {
        return Ok(None);
    }
    if buf.len() > max {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
    }
    let s = String::from_utf8(buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not utf-8"))?;
    Ok(Some(s.trim_end_matches(['\r', '\n']).to_string()))
}

pub fn encode_block(blk: &Block) -> String {
    let mut v = Vec::with_capacity(block_bytes(blk.c.len()));
    put_u64(&mut v, blk.height);
    v.extend_from_slice(&blk.prev);
    put_u64(&mut v, blk.timestamp);
    v.extend_from_slice(&blk.bits.to_le_bytes());
    put_u64(&mut v, blk.nonce);
    v.extend_from_slice(&(blk.c.len() as u32).to_le_bytes());
    for &x in &blk.c {
        put_u64(&mut v, x);
    }
    v.extend_from_slice(&blk.score);
    v.extend_from_slice(&blk.id);
    hex(&v)
}

/// Decode a block; the byte length must match the declared `C` length exactly.
pub fn decode_block(s: &str) -> Option<Block> {
    let v = unhex(s)?;
    if v.len() < block_bytes(0) {
        return None;
    }
    let u64_at = |o: usize| u64::from_le_bytes(v[o..o + 8].try_into().unwrap());
    let u32_at = |o: usize| u32::from_le_bytes(v[o..o + 4].try_into().unwrap());
    let height = u64_at(0);
    let mut prev = [0u8; 32];
    prev.copy_from_slice(&v[8..40]);
    let timestamp = u64_at(40);
    let bits = u32_at(48);
    let nonce = u64_at(52);
    let clen = u32_at(60) as usize;
    if v.len() != block_bytes(clen) {
        return None;
    }
    let c: Vec<u64> = (0..clen).map(|i| u64_at(64 + 8 * i)).collect();
    let o = 64 + 8 * clen;
    let mut score = [0u8; 32];
    score.copy_from_slice(&v[o..o + 32]);
    let mut id = [0u8; 32];
    id.copy_from_slice(&v[o + 32..o + 64]);
    Some(Block { height, prev, timestamp, bits, nonce, c, score, id })
}

/// Serve a snapshot of `blocks` to one connection (oldest first).
pub fn serve_conn(stream: &mut TcpStream, blocks: &[Block]) {
    let mut w = io::BufWriter::new(stream);
    for b in blocks {
        if writeln!(w, "B {}", encode_block(b)).is_err() {
            return;
        }
    }
    let _ = writeln!(w, "E");
    let _ = w.flush();
}

/// Pull a peer's chain and validate it against `template` (same id, version, profile, dataset).
/// Returns the validated chain, or `None` if the peer sent anything invalid. No lock is needed.
pub fn fetch_chain(template: &Chain, addr: &str) -> io::Result<Option<Chain>> {
    let stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    {
        let mut w = stream.try_clone()?;
        writeln!(w, "SYNC")?;
        w.flush()?;
    }
    let mut reader = BufReader::new(stream);
    let max = max_line(template.profile.n);
    let mut fresh = template.empty_like();
    loop {
        let t = match read_line_bounded(&mut reader, max) {
            Ok(Some(t)) => t,
            Ok(None) => break,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => return Ok(None),
            Err(e) => return Err(e),
        };
        if t == "E" {
            break;
        }
        let Some(rest) = t.strip_prefix("B ") else { return Ok(None) };
        if fresh.blocks.len() >= MAX_SYNC_BLOCKS {
            return Ok(None);
        }
        let valid = decode_block(rest).map(|blk| fresh.append_checked(blk)).unwrap_or(false);
        if !valid {
            return Ok(None); // invalid peer chain; ignore it entirely
        }
    }
    Ok(Some(fresh))
}

/// Replace `chain` by `candidate` if it has strictly greater cumulative work; returns the new height.
pub fn adopt_if_better(chain: &mut Chain, candidate: Chain) -> usize {
    if candidate.cumulative_work() > chain.cumulative_work() {
        chain.blocks = candidate.blocks;
        chain.height() as usize
    } else {
        0
    }
}

/// Pull a peer's chain and adopt it if it is valid and has strictly greater cumulative work.
pub fn sync_from(chain: &mut Chain, addr: &str) -> io::Result<usize> {
    match fetch_chain(chain, addr)? {
        Some(fresh) => Ok(adopt_if_better(chain, fresh)),
        None => Ok(0),
    }
}

// ---------------- solo / pool job-submit protocol ----------------

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

/// First nonce of a miner's range: `nonce = (extranonce << 32) | counter`.
pub fn nonce_base(extranonce: u64) -> u64 {
    extranonce << 32
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

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Result of one `SUB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Valid block, appended.
    Accepted,
    /// The tip moved since this connection's last `JOB` (another miner won the height); not verified.
    Stale,
    /// Malformed, out of the extranonce range, bad timestamp, or failed validation.
    Rejected,
}

/// Per-miner counters (keyed by extranonce). Only full blocks exist (there is no share target below
/// the block target), so "accepted" = blocks appended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MinerStats {
    pub extranonce: u64,
    pub accepted: u64,
    pub stale: u64,
    pub rejected: u64,
}

#[derive(Default)]
pub struct PoolStats {
    inner: Mutex<BTreeMap<u64, MinerStats>>,
}

impl PoolStats {
    pub fn record(&self, extranonce: u64, outcome: Outcome) {
        let mut m = self.inner.lock().unwrap();
        let e = m.entry(extranonce).or_insert(MinerStats { extranonce, ..Default::default() });
        match outcome {
            Outcome::Accepted => e.accepted += 1,
            Outcome::Stale => e.stale += 1,
            Outcome::Rejected => e.rejected += 1,
        }
    }

    /// Counters of every miner that submitted at least once, by extranonce.
    pub fn snapshot(&self) -> Vec<MinerStats> {
        self.inner.lock().unwrap().values().copied().collect()
    }
}

/// Handle one connection: `SYNC` (serve chain, then close), or repeated `JOB`/`SUB <hex>` on the
/// same connection. `extranonce` is this miner's nonce-range id; a `SUB` outside the range, with a
/// timestamp too far in the future or not above the median time past is rejected. A `SUB` after the
/// tip moved past this connection's last `JOB` is counted as stale and answered `BAD stale`.
pub fn handle_conn(stream: TcpStream, chain: &Mutex<Chain>, extranonce: u64, stats: &PoolStats) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let max = max_line(chain.lock().unwrap().profile.n);
    let mut w = stream;
    let mut last_job: Option<(u64, [u8; 32])> = None; // (height, prev) of the last JOB handed out
    loop {
        let t = match read_line_bounded(&mut reader, max) {
            Ok(Some(t)) => t,
            _ => break,
        };
        if t == "SYNC" {
            let blocks = chain.lock().unwrap().blocks.clone(); // release the lock before writing
            serve_conn(&mut w, &blocks);
            break;
        } else if t == "JOB" {
            let job = {
                let c = chain.lock().unwrap();
                let tpl = c.template();
                Job {
                    chain_id: c.chain_id,
                    version: c.version,
                    prev: tpl.prev,
                    height: tpl.height,
                    timestamp: now().max(tpl.min_timestamp),
                    bits: tpl.bits,
                    extranonce,
                }
            };
            last_job = Some((job.height, job.prev));
            if writeln!(w, "JOB {}", encode_job(&job)).is_err() {
                break;
            }
        } else if let Some(rest) = t.strip_prefix("SUB ") {
            let outcome = match decode_sub(rest) {
                None => Outcome::Rejected,
                Some((nonce, _, _)) if nonce >> 32 != extranonce => Outcome::Rejected,
                Some((_, timestamp, _)) if timestamp > now() + MAX_FUTURE_DRIFT => Outcome::Rejected,
                Some((nonce, timestamp, c_vec)) => {
                    let mut c = chain.lock().unwrap();
                    let tpl = c.template();
                    if last_job.is_some_and(|job| job != (tpl.height, tpl.prev)) {
                        Outcome::Stale
                    } else {
                        let ph = preheader(&c.chain_id, c.version, tpl.height, &tpl.prev, timestamp, tpl.bits, nonce);
                        let block = Block {
                            height: tpl.height,
                            prev: tpl.prev,
                            timestamp,
                            bits: tpl.bits,
                            nonce,
                            score: score(&ph, &c_vec),
                            id: block_id(&ph, &c_vec),
                            c: c_vec,
                        };
                        if c.append_checked(block) {
                            Outcome::Accepted
                        } else {
                            Outcome::Rejected
                        }
                    }
                }
            };
            stats.record(extranonce, outcome);
            let reply = match outcome {
                Outcome::Accepted => format!("OK {}", chain.lock().unwrap().height()),
                Outcome::Stale => "BAD stale".to_string(),
                Outcome::Rejected => "BAD".to_string(),
            };
            if writeln!(w, "{reply}").is_err() {
                break;
            }
        } else {
            break;
        }
    }
    let _ = w.flush();
}

/// Accept connections forever; assign each a distinct extranonce (from 1; 0 is reserved for the
/// node's own miner) and record per-miner results in `stats`. At most `MAX_CONNS` at once.
pub fn serve_multi(listener: TcpListener, chain: Arc<Mutex<Chain>>, stats: Arc<PoolStats>) {
    let mut next: u64 = 1;
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(s) = stream else { continue };
        if active.load(Ordering::Acquire) >= MAX_CONNS {
            drop(s); // over capacity: refuse
            continue;
        }
        active.fetch_add(1, Ordering::AcqRel);
        let en = next;
        next += 1;
        let (c, st, act) = (Arc::clone(&chain), Arc::clone(&stats), Arc::clone(&active));
        std::thread::spawn(move || {
            handle_conn(s, &c, en, &st);
            act.fetch_sub(1, Ordering::AcqRel);
        });
    }
}
