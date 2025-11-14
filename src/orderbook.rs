//! Order Book Module
//! 
//! High-performance order book aggregation and management

use anyhow::Result;
use dashmap::DashMap;

#[derive(Clone)]
pub struct Aggregator {
    books: DashMap<String, OrderBook>,
}

impl Aggregator {
    pub fn new() -> Self {
        Aggregator {
            books: DashMap::new(),
        }
    }
    
    pub fn update(&self, _message: String) -> Result<()> {
        // TODO: Implement order book update
        Ok(())
    }
    
    pub fn get_book(&self, symbol: &str) -> Option<OrderBook> {
        self.books.get(symbol).map(|entry| entry.clone())
    }
}

#[derive(Clone, Debug)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
}

impl OrderBook {
    pub fn mid_price(&self) -> Option<f64> {
        if !self.bids.is_empty() && !self.asks.is_empty() {
            Some((self.bids[0].price + self.asks[0].price) / 2.0)
        } else {
            None
        }
    }
    
    pub fn spread(&self) -> Option<f64> {
        if !self.bids.is_empty() && !self.asks.is_empty() {
            Some(self.asks[0].price - self.bids[0].price)
        } else {
            None
        }
    }
}