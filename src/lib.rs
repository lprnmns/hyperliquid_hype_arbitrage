//! Hyperliquid Bot Library
//! 
//! Core library for high-frequency arbitrage trading on Hyperliquid

pub mod config;
pub mod websocket;
pub mod nonce;
pub mod signing;
pub mod orderbook;
pub mod arbitrage;
pub mod order;
pub mod risk;
pub mod telemetry;
pub mod utils;

// Re-export commonly used types
pub use config::Config;
pub use orderbook::{OrderBook, Level};
pub use arbitrage::{Engine as ArbitrageEngine, Opportunity};
pub use order::{Manager as OrderManager, Order, OrderType};
pub use risk::{Manager as RiskManager, RiskMetrics};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");