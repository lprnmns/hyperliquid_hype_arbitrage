//! Risk Management Module
//! 
//! Manages risk limits and position safety

use anyhow::Result;
use crate::config::Config;
use crate::arbitrage::Opportunity;

#[derive(Clone)]
pub struct Manager {
    config: Config,
    daily_loss: f64,
    open_positions: usize,
}

impl Manager {
    pub fn new(config: Config) -> Self {
        Manager {
            config,
            daily_loss: 0.0,
            open_positions: 0,
        }
    }
    
    pub fn can_trade(&self, opportunity: &Opportunity) -> Result<bool> {
        // Check daily loss limit
        if self.daily_loss >= self.config.max_daily_loss_usd {
            return Ok(false);
        }
        
        // Check position size limit
        let total_position = opportunity.perp_size + opportunity.spot_size;
        if total_position > self.config.max_position_size_usd {
            return Ok(false);
        }
        
        // Check if we already have an open position
        if self.open_positions > 0 {
            return Ok(false);
        }
        
        Ok(true)
    }
    
    pub fn update_daily_loss(&mut self, loss: f64) {
        self.daily_loss += loss;
    }
    
    pub fn reset_daily_metrics(&mut self) {
        self.daily_loss = 0.0;
    }
}

#[derive(Clone, Debug)]
pub struct RiskMetrics {
    pub daily_pnl: f64,
    pub total_exposure: f64,
    pub margin_usage: f64,
    pub open_positions: usize,
    pub win_rate: f64,
}