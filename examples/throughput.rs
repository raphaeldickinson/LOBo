//! Throughput benchmark for the LOBo engine with the demo's 10 ms sleep removed.
//!
//! It replays the same workload as the live demo (SplitMix64 RNG, an SPY-like price
//! band, quantities 1-9) as fast as the machine allows, resetting the book every 500
//! orders so each book stays in the same small regime the demo actually runs in.
//!
//! Run it with:
//!   cargo run --release --example throughput
//!
//! Two figures are reported to stderr:
//!   A) `place_order` only, which measures the matching engine itself, and
//!   B) `place_order` + `display()`, which shows how much terminal rendering costs.
//! Scenario B floods stdout, so it only runs when you pass `display` and is best timed
//! with stdout redirected, e.g. `cargo run --release --example throughput display > out.txt`.

use std::hint::black_box;
use std::time::Instant;

use lobo::{LimitOrderBook, Order, Side};

/// A tiny, dependency-free SplitMix64 generator, matching the demo's RNG but seeded
/// with a fixed constant so the benchmark is reproducible from run to run.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Rng { state: seed | 1 }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a uniformly distributed value in the inclusive range `[low, high]`.
    fn range(&mut self, low: u64, high: u64) -> u64 {
        let span = high - low + 1;
        low + self.next_u64() % span
    }
}

/// The demo's price band: an SPY-like base of 720.65 with prices within +/-10%.
const SPY: u64 = 7_206_500;
/// Orders placed before the book is reset, keeping it in the demo's small regime.
const BATCH: u32 = 500;
const SEED: u64 = 0x1234_5678;

fn report(label: &str, orders: u64, elapsed_secs: f64) {
    let per_sec = orders as f64 / elapsed_secs;
    eprintln!(
        "{label}: {orders} orders in {elapsed_secs:.3}s  =>  {per_sec:.0} orders/sec  |  {:.0} per 10 ms",
        per_sec * 0.01
    );
}

fn main() {
    let price_low = (SPY as f64 * 0.9) as u64;
    let price_high = (SPY as f64 * 1.1 + 1.0) as u64;
    let with_display = std::env::args().any(|a| a == "display");

    // Scenario A: the matching engine on its own (no display, no sleep).
    {
        let repeats: u64 = 40_000; // 40_000 * 500 = 20M place_order calls
        let mut rng = Rng::new(SEED);
        let mut placed: u64 = 0;
        let start = Instant::now();
        for _ in 0..repeats {
            let mut book = LimitOrderBook::new();
            for oid in 1..=BATCH {
                let side = if rng.range(0, 1) == 0 {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let price = rng.range(price_low, price_high);
                let quantity = rng.range(1, 9) as u8;
                book.place_order(Order::new(oid, price, quantity, side));
                placed += 1;
            }
            black_box(&book);
        }
        report("A) place_order only          ", placed, start.elapsed().as_secs_f64());
    }

    // Scenario B: the same loop but redrawing the book each time. Opt-in, since it
    // writes a full frame per order; redirect stdout to time it cleanly.
    if with_display {
        let repeats: u64 = 400; // 400 * 500 = 200k display() calls
        let mut rng = Rng::new(SEED);
        let mut placed: u64 = 0;
        let start = Instant::now();
        for _ in 0..repeats {
            let mut book = LimitOrderBook::new();
            for oid in 1..=BATCH {
                let side = if rng.range(0, 1) == 0 {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let price = rng.range(price_low, price_high);
                let quantity = rng.range(1, 9) as u8;
                book.place_order(Order::new(oid, price, quantity, side));
                book.display();
                placed += 1;
            }
            black_box(&book);
        }
        report("B) place_order + display     ", placed, start.elapsed().as_secs_f64());
    } else {
        eprintln!("(pass `display` to also benchmark place_order + display(); redirect stdout when you do)");
    }
}
