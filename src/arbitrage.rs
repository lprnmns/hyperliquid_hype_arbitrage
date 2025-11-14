//! Arbitrage Engine Module
//! 
//! Detects and analyzes arbitrage opportunities

use anyhow::Result;
use crate::config::Config;
use crate::orderbook::Aggregator;

#[derive(Clone)]
pub struct Engine {
    config: Config,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Engine { config }
    }
    
    pub fn check_opportunity(&self, _books: &Aggregator) -> Result<Option<Opportunity>> {
        // TODO: Implement opportunity detection
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub struct Opportunity {
    pub perp_price: f64,
    pub spot_price: f64,
    pub basis_bps: f64,
    pub expected_profit: f64,
    pub perp_size: f64,
    pub spot_size: f64,
}