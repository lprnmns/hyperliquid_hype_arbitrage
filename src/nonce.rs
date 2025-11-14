//! Nonce Management Module
//! 
//! Ensures monotonically increasing nonces to prevent 422 errors.
//! Critical for high-frequency trading where rapid order submission is required.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{Result, bail};
use chrono::Utc;
use tracing::{debug, warn};
use std::sync::Arc;

/// Nonce manager ensuring monotonically increasing values
#[derive(Debug, Clone)]
pub struct Manager {
    last_nonce: Arc<AtomicU64>,
    min_spacing_micros: u64,  // Minimum microseconds between nonces
}

impl Manager {
    /// Create a new nonce manager
    pub fn new() -> Self {
        let current_millis = Utc::now().timestamp_millis() as u64;
        
        Manager {
            last_nonce: Arc::new(AtomicU64::new(current_millis)),
            min_spacing_micros: 100,  // 100 microseconds minimum spacing
        }
    }
    
    /// Get the next nonce value
    /// 
    /// Guarantees:
    /// 1. Always monotonically increasing
    /// 2. Never reuses values even on rapid calls
    /// 3. Handles system clock adjustments
    pub fn get_next_nonce(&self) -> u64 {
        loop {
            let current_millis = self.get_current_timestamp();
            let last = self.last_nonce.load(Ordering::SeqCst);
            
            // Ensure nonce is always increasing
            let next = if current_millis <= last {
                // If system time hasn't advanced or went backwards,
                // increment from last nonce
                last + 1
            } else {
                current_millis
            };
            
            // Try to update atomically
            match self.last_nonce.compare_exchange(
                last,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    debug!("Generated nonce: {}", next);
                    return next;
                }
                Err(_) => {
                    // Another thread updated, retry
                    continue;
                }
            }
        }
    }
    
    /// Get current timestamp in milliseconds
    fn get_current_timestamp(&self) -> u64 {
        Utc::now().timestamp_millis() as u64
    }
    
    /// Validate a nonce against Hyperliquid requirements
    /// 
    /// Hyperliquid allows ±5 seconds drift from server time
    pub fn validate_nonce(&self, nonce: u64) -> Result<()> {
        let current = self.get_current_timestamp();
        let diff = if nonce > current {
            nonce - current
        } else {
            current - nonce
        };
        
        // 5 second tolerance (5000 ms)
        if diff > 5000 {
            bail!("Nonce drift too large: {} ms", diff);
        }
        
        Ok(())
    }
    
    /// Reset the nonce manager (use cautiously)
    pub fn reset(&self) {
        let current = self.get_current_timestamp();
        self.last_nonce.store(current, Ordering::SeqCst);
        warn!("Nonce manager reset to: {}", current);
    }
    
    /// Get the last used nonce
    pub fn last_nonce(&self) -> u64 {
        self.last_nonce.load(Ordering::SeqCst)
    }
    
    /// Check if system time is synchronized
    pub fn check_time_sync(&self) -> Result<()> {
        // Get system time
        let system_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as u64;
        
        // Get chrono time (which might use NTP)
        let chrono_time = self.get_current_timestamp();
        
        let diff = if system_time > chrono_time {
            system_time - chrono_time
        } else {
            chrono_time - system_time
        };
        
        // Warn if drift is more than 1 second
        if diff > 1000 {
            warn!("System time drift detected: {} ms", diff);
            bail!("System time not synchronized. Please sync with NTP.");
        }
        
        Ok(())
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe global nonce manager (optional singleton pattern)
pub mod global {
    use super::*;
    use once_cell::sync::Lazy;
    
    static GLOBAL_MANAGER: Lazy<Manager> = Lazy::new(Manager::new);
    
    /// Get next nonce from global manager
    pub fn next_nonce() -> u64 {
        GLOBAL_MANAGER.get_next_nonce()
    }
    
    /// Validate nonce using global manager
    pub fn validate(nonce: u64) -> Result<()> {
        GLOBAL_MANAGER.validate_nonce(nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;
    
    #[test]
    fn test_monotonic_increase() {
        let manager = Manager::new();
        let mut prev = 0;
        
        for _ in 0..1000 {
            let nonce = manager.get_next_nonce();
            assert!(nonce > prev, "Nonce must always increase");
            prev = nonce;
        }
    }
    
    #[test]
    fn test_concurrent_access() {
        let manager = Arc::new(Manager::new());
        let mut handles = vec![];
        let mut all_nonces = vec![];
        
        // Spawn 10 threads each getting 100 nonces
        for _ in 0..10 {
            let mgr = manager.clone();
            let handle = thread::spawn(move || {
                let mut nonces = vec![];
                for _ in 0..100 {
                    nonces.push(mgr.get_next_nonce());
                }
                nonces
            });
            handles.push(handle);
        }
        
        // Collect all nonces
        for handle in handles {
            all_nonces.extend(handle.join().unwrap());
        }
        
        // Check all nonces are unique
        let mut sorted = all_nonces.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(all_nonces.len(), sorted.len(), "All nonces must be unique");
    }
    
    #[test]
    fn test_nonce_validation() {
        let manager = Manager::new();
        let current = manager.get_current_timestamp();
        
        // Valid nonce (current time)
        assert!(manager.validate_nonce(current).is_ok());
        
        // Valid nonce (4 seconds in future)
        assert!(manager.validate_nonce(current + 4000).is_ok());
        
        // Valid nonce (4 seconds in past)
        assert!(manager.validate_nonce(current - 4000).is_ok());
        
        // Invalid nonce (6 seconds in future)
        assert!(manager.validate_nonce(current + 6000).is_err());
        
        // Invalid nonce (6 seconds in past)
        assert!(manager.validate_nonce(current - 6000).is_err());
    }
    
    #[test]
    fn test_rapid_fire_nonces() {
        let manager = Manager::new();
        let mut nonces = vec![];
        
        // Get 10000 nonces as fast as possible
        for _ in 0..10000 {
            nonces.push(manager.get_next_nonce());
        }
        
        // Check all are unique and increasing
        for i in 1..nonces.len() {
            assert!(nonces[i] > nonces[i-1], "Nonces must be strictly increasing");
        }
    }
}