//! LOBo demo — drives the limit order book with a stream of random orders.
//!
//! Mirrors the reference simulation: around an SPY-like base price of 720.65,
//! it generates 500 random orders (random side, price within +/-10% of the base,
//! quantity 1-9), placing one every 10 ms and redrawing the book after each so the
//! matching engine can be watched live in the terminal.

use std::thread;
use std::time::Duration;

use lobo::{LimitOrderBook, Order, Side};

/// A tiny, dependency-free pseudo-random number generator (SplitMix64).
///
/// Seeded from the standard library's OS-randomized `RandomState`, so each run
/// produces a different sequence — the same intent as seeding from a random
/// device, without pulling in an external crate.
mod rng {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    pub struct Rng {
        state: u64,
    }

    impl Rng {
        pub fn new() -> Self {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(0x9E37_79B9_7F4A_7C15);
            Rng {
                state: hasher.finish() | 1,
            }
        }

        fn next_u64(&mut self) -> u64 {
            self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        /// Returns a uniformly distributed value in the inclusive range `[low, high]`.
        pub fn range(&mut self, low: u64, high: u64) -> u64 {
            let span = high - low + 1;
            low + self.next_u64() % span
        }
    }
}

fn main() {
    let mut book = LimitOrderBook::new();
    let mut rng = rng::Rng::new();

    // ---------- begin testing ----------

    let spy: u64 = 7_206_500;
    let price_low = (spy as f64 * 0.9) as u64; // 6_485_850
    let price_high = (spy as f64 * 1.1 + 1.0) as u64; // 7_927_151

    for oid in 1u32..=500 {
        let side = if rng.range(0, 1) == 0 {
            Side::Buy
        } else {
            Side::Sell
        };
        let price = rng.range(price_low, price_high);
        let quantity = rng.range(1, 9) as u8;

        thread::sleep(Duration::from_millis(10));
        book.place_order(Order::new(oid, price, quantity, side));
        book.display();
    }

    // ----------- end testing -----------

    book.display();
}
