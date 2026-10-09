//! Candidate T (spec/07) reference tool.
//!
//! check: n b L w mult epoch_hex hd_hex nonce i c bits  -> recompute ticket (i, c), print piece and verdict
//! bench: n L threads [reps]                              -> CPU time to verify one ticket (L n^2 MACs)

use abacus_chain::score_lead;
use abacus_chain::tnet::*;
use std::time::Instant;

fn hex32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    assert_eq!(s.len(), 64, "expected 32-byte hex");
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex");
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(|s| s.as_str()) {
        Some("check") if a.len() == 12 => {
            let num = |k: usize| a[k].parse::<u64>().expect("number");
            let p = TnetParams {
                n: num(1) as usize,
                b: num(2) as usize,
                layers: num(3) as usize,
                w: num(4) as usize,
                mult: num(5) as i32,
            };
            let (epoch, hd) = (hex32(&a[6]), hex32(&a[7]));
            let (nonce, i, c, bits) = (num(8), num(9) as usize, num(10) as usize, num(11) as u32);
            let weights = epoch_weights(&epoch, &p);
            let row = forward_row(&weights, &p, &x0_seed(&hd, nonce), i, 8);
            let piece = &row[c * p.w..(c + 1) * p.w];
            let h = ticket_hash(piece, &hd, nonce, i as u32, c as u32);
            let piece_hex: String = piece.iter().map(|&x| format!("{:02x}", x as u8)).collect();
            println!(
                "{{\"piece\": \"{piece_hex}\", \"lead\": {}, \"ok\": {}}}",
                score_lead(&h),
                verify_ticket(&weights, &p, &hd, nonce, i, c, bits, 8)
            );
        }
        Some("bench") if a.len() >= 4 => {
            let n: usize = a[1].parse().unwrap();
            let layers: usize = a[2].parse().unwrap();
            let threads: usize = a[3].parse().unwrap();
            let reps: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(7);
            let p = TnetParams { n, b: n, layers, w: 64, mult: default_mult(n) };
            let t = Instant::now();
            let weights = epoch_weights(&[0x11; 32], &p);
            let epoch_s = t.elapsed().as_secs_f64();
            let mut times: Vec<f64> = (0..reps)
                .map(|r| {
                    let t = Instant::now();
                    std::hint::black_box(verify_ticket(&weights, &p, &[0x22; 32], r as u64, r % n, 0, 0, threads));
                    t.elapsed().as_secs_f64() * 1e3
                })
                .collect();
            times.sort_by(|x, y| x.partial_cmp(y).unwrap());
            let macs = layers as f64 * (n as f64).powi(2);
            println!(
                "{{\"n\": {n}, \"L\": {layers}, \"threads\": {threads}, \"reps\": {reps}, \"epoch_weights_s\": {epoch_s:.2}, \
                 \"verify_ms\": {:.2}, \"verify_ms_min\": {:.2}, \"verify_ms_max\": {:.2}, \"GMAC_s\": {:.2}}}",
                times[reps / 2],
                times[0],
                times[reps - 1],
                macs / (times[reps / 2] / 1e3) / 1e9
            );
        }
        _ => eprintln!("usage: abacus-tnet check n b L w mult epoch hd nonce i c bits | bench n L threads [reps]"),
    }
}
