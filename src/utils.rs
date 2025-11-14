//! Utility functions and helpers
//! 
//! Common utilities used across the bot

use rust_decimal::Decimal;
use std::str::FromStr;

/// Round price to tick size
pub fn round_price(price: f64, tick_size: f64) -> f64 {
    (price / tick_size).round() * tick_size
}

/// Round quantity to decimals
pub fn round_quantity(quantity: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (quantity * factor).floor() / factor
}

/// Calculate basis in bps
pub fn calculate_basis_bps(perp_price: f64, spot_price: f64) -> f64 {
    10000.0 * ((perp_price - spot_price) / spot_price)
}

/// Convert string to decimal safely
pub fn parse_decimal(s: &str) -> Option<Decimal> {
    Decimal::from_str(s).ok()
}

/// Format USD amount
pub fn format_usd(amount: f64) -> String {
    format!("${:.2}", amount)
}

/// Format BPS
pub fn format_bps(bps: f64) -> String {
    format!("{:.2} bps", bps)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_round_price() {
        assert_eq!(round_price(1.23456, 0.0001), 1.2346);
        assert_eq!(round_price(1.23454, 0.0001), 1.2345);
        assert_eq!(round_price(1.23456, 0.01), 1.23);
    }
    
    #[test]
    fn test_round_quantity() {
        assert_eq!(round_quantity(1.23456, 2), 1.23);
        assert_eq!(round_quantity(1.23456, 4), 1.2345);
        assert_eq!(round_quantity(1.99999, 2), 1.99);
    }
    
    #[test]
    fn test_calculate_basis_bps() {
        assert_eq!(calculate_basis_bps(1.005, 1.000), 50.0);
        assert_eq!(calculate_basis_bps(1.000, 1.005), -49.751243781094534);
        assert_eq!(calculate_basis_bps(1.010, 1.000), 100.0);
    }
}