//! LOBo — a limit order book (LOB) matching engine with price-time priority.
//!
//! The book keeps two sides — bids (buy orders) and asks (sell orders) — organized
//! into price levels. Within a price level, orders are matched in first-in / first-out
//! order, giving strict price-time priority. Whenever the best bid crosses the best ask,
//! trades execute automatically, supporting partial fills and cascading matches.
//!
//! Internally the book uses three collections:
//!   * a `BTreeMap` per side to keep price levels sorted (best bid = highest price,
//!     best ask = lowest price),
//!   * a `VecDeque` at each price level to enforce FIFO time priority, and
//!   * a `HashMap` from order id to its `(side, price)` for fast cancellation.

use std::collections::{BTreeMap, HashMap, VecDeque};

/// Which side of the market an order sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// A bid — an order to buy.
    Buy,
    /// An ask — an order to sell.
    Sell,
}

/// A single limit order.
#[derive(Debug, Clone, Copy)]
pub struct Order {
    /// Unique identifier of the order.
    pub order_id: u32,
    /// Limit price for execution, stored as an integer (divided by 10_000 for display).
    pub price: u64,
    /// Starting volume of the order.
    pub initial_quantity: u8,
    /// Remaining unfilled volume of the order.
    pub remaining_quantity: u8,
    /// Market side (buy / sell).
    pub side: Side,
}

impl Order {
    /// Creates a new order. `remaining_quantity` starts equal to `initial_quantity`.
    pub fn new(order_id: u32, price: u64, initial_quantity: u8, side: Side) -> Self {
        Order {
            order_id,
            price,
            initial_quantity,
            remaining_quantity: initial_quantity,
            side,
        }
    }
}

/// The core matching engine: all resting bids and asks plus the interface to place,
/// cancel, and execute trades based on price-time priority.
#[derive(Default)]
pub struct LimitOrderBook {
    /// Maps an order id to its `(side, price)` for O(1) lookups and cancellations.
    order_index: HashMap<u32, (Side, u64)>,
    /// Buy orders, keyed by price. The best bid is the highest price (last key).
    bids: BTreeMap<u64, VecDeque<Order>>,
    /// Sell orders, keyed by price. The best ask is the lowest price (first key).
    asks: BTreeMap<u64, VecDeque<Order>>,
}

impl LimitOrderBook {
    /// Creates an empty order book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a new order and triggers matching.
    ///
    /// Routes the order to the correct side, records it in the id index for fast
    /// lookups, then repeatedly executes trades while the spread is crossed
    /// (best bid price >= best ask price).
    pub fn place_order(&mut self, order: Order) {
        match order.side {
            Side::Buy => self.bids.entry(order.price).or_default().push_back(order),
            Side::Sell => self.asks.entry(order.price).or_default().push_back(order),
        }
        self.order_index
            .insert(order.order_id, (order.side, order.price));

        while let (Some(bid_price), Some(ask_price)) =
            (self.best_bid_price(), self.best_ask_price())
        {
            if bid_price >= ask_price {
                self.execute_trade();
            } else {
                break;
            }
        }
    }

    /// Removes an order from the book by id, cleaning up an emptied price level.
    /// A no-op if the id is unknown.
    pub fn cancel_order(&mut self, order_id: u32) {
        let (side, price) = match self.order_index.remove(&order_id) {
            Some(entry) => entry,
            None => return,
        };

        let book = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        if let Some(level) = book.get_mut(&price) {
            level.retain(|o| o.order_id != order_id);
            if level.is_empty() {
                book.remove(&price);
            }
        }
    }

    /// Matches the current best bid against the current best ask.
    ///
    /// Fills the smaller of the two remaining quantities, decrements both, and
    /// cancels any order that becomes fully filled. Assumes the prices are crossed;
    /// does nothing if either side is empty.
    pub fn execute_trade(&mut self) {
        let bid_price = match self.best_bid_price() {
            Some(p) => p,
            None => return,
        };
        let ask_price = match self.best_ask_price() {
            Some(p) => p,
            None => return,
        };

        // The best bid / ask are the front (oldest) orders at the best price levels.
        let (filled_bid, filled_ask) = {
            let bid = self
                .bids
                .get_mut(&bid_price)
                .and_then(VecDeque::front_mut)
                .expect("best bid level is non-empty");
            let ask = self
                .asks
                .get_mut(&ask_price)
                .and_then(VecDeque::front_mut)
                .expect("best ask level is non-empty");

            let fill = bid.remaining_quantity.min(ask.remaining_quantity);
            bid.remaining_quantity -= fill;
            ask.remaining_quantity -= fill;

            let filled_bid = if bid.remaining_quantity == 0 {
                Some(bid.order_id)
            } else {
                None
            };
            let filled_ask = if ask.remaining_quantity == 0 {
                Some(ask.order_id)
            } else {
                None
            };
            (filled_bid, filled_ask)
        };

        if let Some(id) = filled_bid {
            self.cancel_order(id);
        }
        if let Some(id) = filled_ask {
            self.cancel_order(id);
        }
    }

    /// Price of the best (highest) bid, if any.
    fn best_bid_price(&self) -> Option<u64> {
        self.bids.keys().next_back().copied()
    }

    /// Price of the best (lowest) ask, if any.
    fn best_ask_price(&self) -> Option<u64> {
        self.asks.keys().next().copied()
    }

    /// Prints the current state of the order book to stdout.
    ///
    /// Shows aggregated bid and ask levels side by side (bids in green on the left,
    /// asks in red on the right) along with the best bid, best ask, and spread.
    /// Uses ANSI escape codes for the screen clear and colors.
    pub fn display(&self) {
        // Bids highest-first, asks lowest-first.
        let flat_bids: Vec<&Order> = self
            .bids
            .iter()
            .rev()
            .flat_map(|(_, level)| level.iter())
            .collect();
        let flat_asks: Vec<&Order> = self
            .asks
            .iter()
            .flat_map(|(_, level)| level.iter())
            .collect();

        let len = flat_bids.len().max(flat_asks.len());
        let dashes = 73;
        let mut height: i32 = 20;

        // Clear the screen and move the cursor home.
        print!("\x1b[2J\x1b[1;1H");

        println!("{}", "-".repeat(dashes));

        for i in 0..len {
            if let Some(b) = flat_bids.get(i) {
                print!("{:<8}", b.order_id);
                print!("\x1b[32m{:>14.4}\x1b[0m", b.price as f64 / 10000.0);
                print!("{:>5}{:>5}", b.initial_quantity, b.remaining_quantity);
            } else {
                print!("{:>32}", ' ');
            }

            print!("    |    ");

            if let Some(a) = flat_asks.get(i) {
                print!("{:<8}", a.order_id);
                print!("\x1b[31m{:>14.4}\x1b[0m", a.price as f64 / 10000.0);
                print!("{:>5}{:>5}", a.initial_quantity, a.remaining_quantity);
            }

            println!();

            height -= 1;
            if height <= 0 {
                break;
            }
        }

        for _ in 0..height {
            print!("{:>32}", ' ');
            println!("    |    ");
        }

        println!("{}", "-".repeat(dashes));

        print!("best bid: ");
        match self.best_bid_price() {
            Some(p) => print!("{:.4}", p as f64 / 10000.0),
            None => print!("n/a"),
        }

        print!(" | best ask: ");
        match self.best_ask_price() {
            Some(p) => print!("{:.4}", p as f64 / 10000.0),
            None => print!("n/a"),
        }

        print!(" | spread: ");
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => println!("{:.4}", (ask as f64 - bid as f64) / 10000.0),
            _ => println!("n/a"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_orders_do_not_cross() {
        let mut book = LimitOrderBook::new();
        book.place_order(Order::new(1, 1_000_000, 5, Side::Buy));
        book.place_order(Order::new(2, 1_010_000, 5, Side::Sell));

        assert_eq!(book.best_bid_price(), Some(1_000_000));
        assert_eq!(book.best_ask_price(), Some(1_010_000));
        assert_eq!(book.order_index.len(), 2);
    }

    #[test]
    fn full_fill_removes_both_orders() {
        let mut book = LimitOrderBook::new();
        book.place_order(Order::new(1, 1_000_000, 5, Side::Buy));
        book.place_order(Order::new(2, 1_000_000, 5, Side::Sell));

        assert_eq!(book.best_bid_price(), None);
        assert_eq!(book.best_ask_price(), None);
        assert!(book.order_index.is_empty());
    }

    #[test]
    fn partial_fill_leaves_remainder_resting() {
        let mut book = LimitOrderBook::new();
        book.place_order(Order::new(1, 1_000_000, 8, Side::Buy));
        // Sell 3 at a lower price -> crosses, fully fills the sell, leaves 5 on the bid.
        book.place_order(Order::new(2, 999_000, 3, Side::Sell));

        assert_eq!(book.best_ask_price(), None);
        assert_eq!(book.best_bid_price(), Some(1_000_000));

        let level = book.bids.get(&1_000_000).unwrap();
        assert_eq!(level.len(), 1);
        assert_eq!(level.front().unwrap().remaining_quantity, 5);
        assert!(book.order_index.contains_key(&1));
        assert!(!book.order_index.contains_key(&2));
    }

    #[test]
    fn price_time_priority_is_fifo_within_a_level() {
        let mut book = LimitOrderBook::new();
        // Two buys at the same price; order 1 arrives first.
        book.place_order(Order::new(1, 1_000_000, 5, Side::Buy));
        book.place_order(Order::new(2, 1_000_000, 5, Side::Buy));
        // A sell of 5 must match the oldest resting buy (order 1) first.
        book.place_order(Order::new(3, 1_000_000, 5, Side::Sell));

        assert!(!book.order_index.contains_key(&1));
        assert!(book.order_index.contains_key(&2));

        let level = book.bids.get(&1_000_000).unwrap();
        assert_eq!(level.len(), 1);
        assert_eq!(level.front().unwrap().order_id, 2);
        assert_eq!(level.front().unwrap().remaining_quantity, 5);
    }

    #[test]
    fn cancel_removes_order_and_empty_level() {
        let mut book = LimitOrderBook::new();
        book.place_order(Order::new(1, 1_000_000, 5, Side::Buy));
        book.cancel_order(1);

        assert_eq!(book.best_bid_price(), None);
        assert!(book.bids.get(&1_000_000).is_none());
        assert!(book.order_index.is_empty());

        // Cancelling an unknown id is a harmless no-op.
        book.cancel_order(999);
    }

    #[test]
    fn marketable_order_cascades_across_levels() {
        let mut book = LimitOrderBook::new();
        book.place_order(Order::new(1, 1_000_000, 3, Side::Sell));
        book.place_order(Order::new(2, 1_001_000, 3, Side::Sell));
        // A buy of 6 that crosses both ask levels clears the book.
        book.place_order(Order::new(3, 1_001_000, 6, Side::Buy));

        assert!(book.order_index.is_empty());
        assert_eq!(book.best_bid_price(), None);
        assert_eq!(book.best_ask_price(), None);
    }
}
