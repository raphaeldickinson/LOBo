# LOBo

**LOBo** is a **L**imit **O**rder **B**ook and matching engine written in Rust. It keeps
two sides of a market, bids (buy orders) and asks (sell orders), each sorted into price
levels, and matches them under strict price-time priority. Orders resting at the same
price fill first-in, first-out. When an incoming order crosses the spread, trades execute
right away, with support for partial fills and cascading matches across several price
levels. The whole thing is a compact model of the core structure that sits at the heart of
every modern electronic exchange.

## Architecture

### Data Structures

**Price Levels (`BTreeMap`):** Price levels on each side are maintained in a `BTreeMap`,
which keeps them naturally sorted by price. Because the tree stays ordered, the best bid
(the highest price) and the best ask (the lowest price) always sit at its two ends, so
matching only ever has to read the top of the book.

**Time Priority (`VecDeque`):** Orders sitting at the same price level are stored in a
`VecDeque`, a double-ended queue. This enforces strict First-In-First-Out (FIFO) execution
while allowing O(1) additions at the back and fills from the front.

**Order Lookup (`HashMap`):** An order id is mapped directly to its `(side, price)` using a
`HashMap`. This guarantees O(1) cancellations without having to search through the book.

### Order Structure

- `order_id` (`u32`): Unique identifier for O(1) lookup.
- `price` (`u64`): The limit price, stored as an integer and divided by 10,000 for display
  (e.g. `7_206_500` shows as `720.6500`).
- `initial_quantity` / `remaining_quantity` (`u8`): The starting size and the size still
  unfilled, so partial fills are handled safely.
- `side` (`Side` enum): Whether the order is a `Buy` (bid) or a `Sell` (ask).

### Core Functions

- `place_order(order)`: Submits a new order to the book and matches while the spread is crossed.
- `cancel_order(id)`: Finds and removes a resting order in O(1) time.
- `execute_trade()`: Fills the best bid against the best ask, updating quantities and clearing filled orders.
- `display()`: Renders the current state of the book to the terminal.

## Build & Run

```sh
cargo build --release   # build an optimized binary
cargo run --release     # run the live demo
cargo test              # run the unit tests
```

The demo simulates stochastic behavior by streaming 500 random orders around an SPY-like
price of 720.65, placing one every 10 ms and redrawing the book after each. Bids show in
green on the left and asks in red on the right, with the best bid, best ask, and spread
reported underneath. You can watch the levels build up and get eaten as marketable orders
cross the spread.

## License

Released under the [MIT License](LICENSE).
