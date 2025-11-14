//! Configuration module for Hyperliquid Bot
//! 
//! Handles loading and validation of all configuration parameters

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // API Configuration
    pub api_url: String,
    pub ws_url: String,
    
    // Account Configuration
    pub agent_private_key: String,
    pub wallet_address: String,
    
    // Trading Parameters
    pub perp_symbol: String,
    pub spot_symbol: String,
    pub bps_threshold: f64,
    pub position_size_usd: f64,
    pub leverage: u8,
    
    // Risk Management
    pub max_position_size_usd: f64,
    pub stop_loss_bps: f64,
    pub max_daily_loss_usd: f64,
    
    // Execution Settings
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub rate_limit_per_min: u32,
    
    // Operation Mode
    pub dry_run: bool,
    pub manual_test_trade: bool,
    pub debug_mode: bool,
    pub close_on_shutdown: bool,
    
    // Monitoring (Optional)
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub alert_on_error: bool,
}

impl Config {
    /// Load configuration from environment variables
    pub fn load() -> Result<Self> {
        info!("Loading configuration from environment...");
        
        let config = Config {
            // API Configuration
            api_url: env::var("HYPERLIQUID_API_URL")
                .unwrap_or_else(|_| "https://api.hyperliquid.xyz".to_string()),
            ws_url: env::var("HYPERLIQUID_WS_URL")
                .unwrap_or_else(|_| "wss://api.hyperliquid.xyz/ws".to_string()),
            
            // Account Configuration
            agent_private_key: env::var("HL_API_AGENT_PRIVATE_KEY")?,
            wallet_address: env::var("HL_API_AGENT_WALLET_ADDRESS")?,
            
            // Trading Parameters
            perp_symbol: env::var("PERP_SYMBOL")
                .unwrap_or_else(|_| "HYPE".to_string()),
            spot_symbol: env::var("SPOT_SYMBOL")
                .unwrap_or_else(|_| "@107".to_string()),
            bps_threshold: env::var("BPS_THRESHOLD")
                .unwrap_or_else(|_| "5.0".to_string())
                .parse()?,
            position_size_usd: env::var("POSITION_SIZE_USD")
                .unwrap_or_else(|_| "20.0".to_string())
                .parse()?,
            leverage: env::var("LEVERAGE")
                .unwrap_or_else(|_| "2".to_string())
                .parse()?,
            
            // Risk Management
            max_position_size_usd: env::var("MAX_POSITION_SIZE_USD")
                .unwrap_or_else(|_| "100.0".to_string())
                .parse()?,
            stop_loss_bps: env::var("STOP_LOSS_BPS")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()?,
            max_daily_loss_usd: env::var("MAX_DAILY_LOSS_USD")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()?,
            
            // Execution Settings
            timeout_seconds: env::var("TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()?,
            max_retries: env::var("MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()?,
            rate_limit_per_min: env::var("RATE_LIMIT_PER_MIN")
                .unwrap_or_else(|_| "90".to_string())
                .parse()?,
            
            // Operation Mode
            dry_run: env::var("DRY_RUN")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            manual_test_trade: env::var("MANUAL_TEST_TRADE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            debug_mode: env::var("DEBUG_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            close_on_shutdown: env::var("CLOSE_ON_SHUTDOWN")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
            
            // Monitoring
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN").ok(),
            telegram_chat_id: env::var("TELEGRAM_CHAT_ID").ok(),
            alert_on_error: env::var("ALERT_ON_ERROR")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
        };
        
        if config.dry_run {
            warn!("⚠️ DRY RUN MODE ENABLED - No real trades will be executed!");
        }
        
        if config.debug_mode {
            info!("🐛 Debug mode enabled - Verbose logging active");
        }
        
        Ok(config)
    }
    
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        // Validate private key format
        if !self.agent_private_key.starts_with("0x") || self.agent_private_key.len() != 66 {
            bail!("Invalid private key format");
        }
        
        // Validate wallet address format
        if !self.wallet_address.starts_with("0x") || self.wallet_address.len() != 42 {
            bail!("Invalid wallet address format");
        }
        
        // Validate trading parameters
        if self.bps_threshold < 0.1 || self.bps_threshold > 100.0 {
            bail!("BPS threshold must be between 0.1 and 100");
        }
        
        if self.position_size_usd < 10.0 {
            bail!("Position size must be at least 10 USD (min notional)");
        }
        
        if self.position_size_usd > self.max_position_size_usd {
            bail!("Position size cannot exceed max position size");
        }
        
        if self.leverage < 1 || self.leverage > 20 {
            bail!("Leverage must be between 1 and 20");
        }
        
        // Validate risk parameters
        if self.stop_loss_bps < 10.0 || self.stop_loss_bps > 1000.0 {
            bail!("Stop loss must be between 10 and 1000 bps");
        }
        
        if self.max_daily_loss_usd <= 0.0 {
            bail!("Max daily loss must be positive");
        }
        
        // Validate execution settings
        if self.timeout_seconds < 1 || self.timeout_seconds > 300 {
            bail!("Timeout must be between 1 and 300 seconds");
        }
        
        if self.max_retries > 10 {
            bail!("Max retries cannot exceed 10");
        }
        
        if self.rate_limit_per_min > 95 {
            warn!("⚠️ Rate limit is very close to Hyperliquid's limit (100/min)");
        }
        
        info!("✅ Configuration validation passed");
        Ok(())
    }
    
    /// Get effective leverage considering spot is unleveraged
    pub fn effective_leverage(&self) -> f64 {
        // For delta-neutral: perp_size = leverage / (leverage + 1) * total
        // spot_size = 1 / (leverage + 1) * total
        self.leverage as f64 / (self.leverage as f64 + 1.0)
    }
    
    /// Calculate position sizes for arbitrage
    pub fn calculate_position_sizes(&self, available_balance: f64) -> (f64, f64) {
        let total_size = self.position_size_usd.min(available_balance);
        let perp_size = total_size * self.effective_leverage();
        let spot_size = total_size - perp_size;
        (perp_size, spot_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_effective_leverage() {
        let mut config = Config::load().unwrap_or_else(|_| {
            Config {
                leverage: 2,
                ..Default::default()
            }
        });
        
        config.leverage = 2;
        assert_eq!(config.effective_leverage(), 2.0 / 3.0);
        
        config.leverage = 3;
        assert_eq!(config.effective_leverage(), 3.0 / 4.0);
    }
    
    #[test]
    fn test_position_size_calculation() {
        let config = Config {
            position_size_usd: 100.0,
            leverage: 2,
            ..Default::default()
        };
        
        let (perp_size, spot_size) = config.calculate_position_sizes(200.0);
        assert_eq!(perp_size, 100.0 * 2.0 / 3.0);
        assert_eq!(spot_size, 100.0 * 1.0 / 3.0);
        assert_eq!(perp_size + spot_size, 100.0);
    }
}

// Implement Default for testing
impl Default for Config {
    fn default() -> Self {
        Config {
            api_url: "https://api.hyperliquid.xyz".to_string(),
            ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
            agent_private_key: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            wallet_address: "0x0000000000000000000000000000000000000000".to_string(),
            perp_symbol: "HYPE".to_string(),
            spot_symbol: "@107".to_string(),
            bps_threshold: 5.0,
            position_size_usd: 20.0,
            leverage: 2,
            max_position_size_usd: 100.0,
            stop_loss_bps: 50.0,
            max_daily_loss_usd: 50.0,
            timeout_seconds: 30,
            max_retries: 3,
            rate_limit_per_min: 90,
            dry_run: true,
            manual_test_trade: false,
            debug_mode: false,
            close_on_shutdown: false,
            telegram_bot_token: None,
            telegram_chat_id: None,
            alert_on_error: false,
        }
    }
}