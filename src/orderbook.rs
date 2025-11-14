//! Order Book Module
//! 
//! High-performance order book aggregation and management

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::debug;

/// Order book aggregator for multiple symbols
#[derive(Clone)]
pub struct Aggregator {
    books: Arc<DashMap<String, Arc<RwLock<OrderBook>>>>,
    last_update: Arc<RwLock<Instant>>,
}

impl Aggregator {
    /// Create new aggregator
    pub fn new() -> Self {
        Aggregator {
            books: Arc::new(DashMap::new()),
            last_update: Arc::new(RwLock::new(Instant::now())),
        }
    }
    
    /// Update order book from WebSocket message
    pub fn update(&self, message: String) -> Result<()> {
        // Parse the message to extract order book data
        if let Ok(data) = serde_json::from_str::<OrderBookUpdate>(&message) {
            self.update_book(data)?;
        }
        
        // Update timestamp
        *self.last_update.write() = Instant::now();
        
        Ok(())
    }
    
    /// Update specific order book
    pub fn update_book(&self, update: OrderBookUpdate) -> Result<()> {
        let book_entry = self.books
            .entry(update.symbol.clone())
            .or_insert_with(|| Arc::new(RwLock::new(OrderBook::new(update.symbol.clone()))));
        
        let mut book = book_entry.write();
        
        // Update bids
        if let Some(bids) = update.bids {
            book.update_bids(bids);
        }
        
        // Update asks
        if let Some(asks) = update.asks {
            book.update_asks(asks);
        }
        
        book.timestamp = update.timestamp.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        });
        
        debug!("Updated order book for {}", update.symbol);
        
        Ok(())
    }
    
    /// Get order book for a symbol
    pub fn get_book(&self, symbol: &str) -> Option<OrderBook> {
        self.books.get(symbol).map(|entry| {
            entry.read().clone()
        })
    }
    
    /// Get best bid/ask for a symbol
    pub fn get_bbo(&self, symbol: &str) -> Option<(Level, Level)> {
        self.get_book(symbol).and_then(|book| {
            match (book.best_bid(), book.best_ask()) {
                (Some(bid), Some(ask)) => Some((bid, ask)),
                _ => None,
            }
        })
    }
    
    /// Calculate basis between perp and spot
    pub fn calculate_basis_bps(&self, perp_symbol: &str, spot_symbol: &str) -> Option<f64> {
        let perp_book = self.get_book(perp_symbol)?;
        let spot_book = self.get_book(spot_symbol)?;
        
        let perp_mid = perp_book.mid_price()?;
        let spot_mid = spot_book.mid_price()?;
        
        Some(10000.0 * ((perp_mid - spot_mid) / spot_mid))
    }
    
    /// Get all tracked symbols
    pub fn get_symbols(&self) -> Vec<String> {
        self.books.iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
    
    /// Clear all order books
    pub fn clear(&self) {
        self.books.clear();
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> AggregatorStats {
        AggregatorStats {
            book_count: self.books.len(),
            last_update: *self.last_update.read(),
        }
    }
}

/// Order book update message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderBookUpdate {
    pub symbol: String,
    pub bids: Option<Vec<Level>>,
    pub asks: Option<Vec<Level>>,
    pub timestamp: Option<u64>,
}

/// Single order book for a symbol
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub timestamp: u64,
}

impl OrderBook {
    /// Create new empty order book
    pub fn new(symbol: String) -> Self {
        OrderBook {
            symbol,
            bids: Vec::new(),
            asks: Vec::new(),
            timestamp: 0,
        }
    }
    
    /// Update bids
    pub fn update_bids(&mut self, mut bids: Vec<Level>) {
        // Sort bids by price descending
        bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
        self.bids = bids;
    }
    
    /// Update asks
    pub fn update_asks(&mut self, mut asks: Vec<Level>) {
        // Sort asks by price ascending
        asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
        self.asks = asks;
    }
    
    /// Get best bid
    pub fn best_bid(&self) -> Option<Level> {
        self.bids.first().cloned()
    }
    
    /// Get best ask
    pub fn best_ask(&self) -> Option<Level> {
        self.asks.first().cloned()
    }
    
    /// Calculate mid price
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / 2.0),
            _ => None,
        }
    }
    
    /// Calculate spread
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }
    
    /// Calculate spread in basis points
    pub fn spread_bps(&self) -> Option<f64> {
        match (self.spread(), self.mid_price()) {
            (Some(spread), Some(mid)) if mid > 0.0 => {
                Some(10000.0 * spread / mid)
            }
            _ => None,
        }
    }
    
    /// Get depth at a price level
    pub fn get_depth_at_price(&self, price: f64, is_bid: bool) -> f64 {
        let levels = if is_bid { &self.bids } else { &self.asks };
        
        levels.iter()
            .filter(|level| {
                if is_bid {
                    level.price >= price
                } else {
                    level.price <= price
                }
            })
            .map(|level| level.quantity)
            .sum()
    }
    
    /// Calculate weighted average price for a quantity
    pub fn calculate_wap(&self, quantity: f64, is_buy: bool) -> Option<f64> {
        let levels = if is_buy { &self.asks } else { &self.bids };
        
        if levels.is_empty() {
            return None;
        }
        
        let mut remaining = quantity;
        let mut total_cost = 0.0;
        
        for level in levels {
            let fill = remaining.min(level.quantity);
            total_cost += fill * level.price;
            remaining -= fill;
            
            if remaining <= 0.0 {
                break;
            }
        }
        
        if remaining > 0.0 {
            // Not enough liquidity
            None
        } else {
            Some(total_cost / quantity)
        }
    }
    
    /// Calculate market impact for a trade
    pub fn calculate_impact(&self, quantity: f64, is_buy: bool) -> Option<f64> {
        let mid = self.mid_price()?;
        let wap = self.calculate_wap(quantity, is_buy)?;
        
        Some(((wap - mid).abs() / mid) * 10000.0) // Return in bps
    }
    
    /// Check if book is stale
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now - self.timestamp > max_age_ms
    }
    
    /// Get book imbalance
    pub fn get_imbalance(&self) -> f64 {
        let bid_volume: f64 = self.bids.iter()
            .take(10)  // Top 10 levels
            .map(|l| l.quantity)
            .sum();
        
        let ask_volume: f64 = self.asks.iter()
            .take(10)
            .map(|l| l.quantity)
            .sum();
        
        if bid_volume + ask_volume > 0.0 {
            (bid_volume - ask_volume) / (bid_volume + ask_volume)
        } else {
            0.0
        }
    }
}

/// Price level in order book
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Level {
    pub price: f64,
    pub quantity: f64,
    pub order_count: Option<u32>,
}

impl Level {
    /// Create new level
    pub fn new(price: f64, quantity: f64) -> Self {
        Level {
            price,
            quantity,
            order_count: None,
        }
    }
    
    /// Calculate notional value
    pub fn notional(&self) -> f64 {
        self.price * self.quantity
    }
}

/// Aggregator statistics
#[derive(Debug, Clone)]
pub struct AggregatorStats {
    pub book_count: usize,
    pub last_update: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_book_operations() {
        let mut book = OrderBook::new("HYPE".to_string());
        
        // Add bids
        book.update_bids(vec![
            Level::new(100.0, 10.0),
            Level::new(99.0, 20.0),
            Level::new(98.0, 30.0),
        ]);
        
        // Add asks
        book.update_asks(vec![
            Level::new(101.0, 10.0),
            Level::new(102.0, 20.0),
            Level::new(103.0, 30.0),
        ]);
        
        // Test best bid/ask
        assert_eq!(book.best_bid().unwrap().price, 100.0);
        assert_eq!(book.best_ask().unwrap().price, 101.0);
        
        // Test mid price
        assert_eq!(book.mid_price().unwrap(), 100.5);
        
        // Test spread
        assert_eq!(book.spread().unwrap(), 1.0);
        
        // Test WAP calculation
        assert_eq!(book.calculate_wap(15.0, true).unwrap(), 101.5);
    }
    
    #[test]
    fn test_aggregator() {
        let aggregator = Aggregator::new();
        
        // Add order book update
        let update = OrderBookUpdate {
            symbol: "HYPE".to_string(),
            bids: Some(vec![Level::new(100.0, 10.0)]),
            asks: Some(vec![Level::new(101.0, 10.0)]),
            timestamp: Some(1234567890),
        };
        
        aggregator.update_book(update).unwrap();
        
        // Test retrieval
        let book = aggregator.get_book("HYPE").unwrap();
        assert_eq!(book.symbol, "HYPE");
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        
        // Test BBO
        let (bid, ask) = aggregator.get_bbo("HYPE").unwrap();
        assert_eq!(bid.price, 100.0);
        assert_eq!(ask.price, 101.0);
    }
}