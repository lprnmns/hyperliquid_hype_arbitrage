//! Hyperliquid HYPE Arbitrage Bot - Ultra Low Latency Implementation
//!
//! This bot performs high-frequency arbitrage between HYPE perpetual
//! and spot markets on Hyperliquid exchange.

use anyhow::Result;
use dotenv::dotenv;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::signal;
use std::sync::Arc;

mod config;
mod websocket;
mod nonce;
mod signing;
mod orderbook;
mod arbitrage;
mod order;
mod risk;
mod telemetry;
mod utils;

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv().ok();
    
    // Initialize tracing/logging
    init_tracing();
    
    info!("🚀 Starting Hyperliquid HYPE Arbitrage Bot v{}", env!("CARGO_PKG_VERSION"));
    
    // Load configuration
    let config = Config::load()?;
    info!("Configuration loaded successfully");
    info!("BPS Threshold: {}", config.bps_threshold);
    info!("Position Size: {} USD", config.position_size_usd);
    info!("Leverage: {}x", config.leverage);
    
    // Validate configuration
    config.validate()?;
    
    // Initialize bot components
    let bot = Arc::new(initialize_bot(config).await?);
    
    // Setup graceful shutdown
    let shutdown_handle = setup_shutdown_handler();
    
    // Run the bot
    info!("Bot initialized, starting main loop...");
    
    let bot_run = bot.clone();
    let bot_shutdown = bot.clone();
    
    tokio::select! {
        result = bot_run.run() => {
            match result {
                Ok(_) => info!("Bot stopped normally"),
                Err(e) => error!("Bot stopped with error: {}", e),
            }
        }
        _ = shutdown_handle => {
            info!("Received shutdown signal, stopping gracefully...");
            bot_shutdown.shutdown().await?;
        }
    }
    
    info!("Bot shutdown complete");
    Ok(())
}

/// Initialize tracing subscriber for structured logging
fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .json(); // JSON format for production
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}

/// Initialize the bot with all components
async fn initialize_bot(config: Config) -> Result<Bot> {
    info!("Initializing bot components...");
    
    // Initialize WebSocket manager
    let ws_manager = websocket::Manager::new(&config).await?;
    
    // Initialize nonce manager
    let nonce_manager = nonce::Manager::new();
    
    // Initialize signing module
    let signer = signing::Signer::new(&config.agent_private_key)?;
    
    // Initialize order book aggregator
    let orderbook_aggregator = orderbook::Aggregator::new();
    
    // Initialize arbitrage engine
    let arbitrage_engine = arbitrage::Engine::new(config.clone());
    
    // Initialize order manager
    let order_manager = order::Manager::new(config.clone(), signer, nonce_manager);
    
    // Initialize risk manager
    let risk_manager = risk::Manager::new(config.clone());
    
    // Initialize telemetry
    let telemetry = telemetry::Telemetry::new();
    
    Ok(Bot {
        config,
        ws_manager,
        orderbook_aggregator,
        arbitrage_engine,
        order_manager,
        risk_manager,
        telemetry,
    })
}

/// Setup signal handler for graceful shutdown
fn setup_shutdown_handler() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let ctrl_c = signal::ctrl_c();
        
        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };
        
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();
        
        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C signal");
            }
            _ = terminate => {
                info!("Received terminate signal");
            }
        }
    })
}

/// Main bot structure containing all components
#[derive(Clone)]
struct Bot {
    config: Config,
    ws_manager: websocket::Manager,
    orderbook_aggregator: orderbook::Aggregator,
    arbitrage_engine: arbitrage::Engine,
    order_manager: order::Manager,
    risk_manager: risk::Manager,
    telemetry: telemetry::Telemetry,
}

impl Bot {
    /// Main bot execution loop
    async fn run(&self) -> Result<()> {
        // Start WebSocket connections
        self.ws_manager.connect().await?;
        
        // Subscribe to required channels
        self.ws_manager.subscribe_l2_book("HYPE").await?;
        self.ws_manager.subscribe_l2_book("@107").await?; // HYPE spot
        self.ws_manager.subscribe_user_events().await?;
        
        // Main processing loop
        loop {
            // Process WebSocket messages
            if let Some(msg) = self.ws_manager.receive_message().await? {
                // Update order books
                self.orderbook_aggregator.update(msg)?;
                
                // Check for arbitrage opportunities
                if let Some(opportunity) = self.arbitrage_engine.check_opportunity(&self.orderbook_aggregator)? {
                    // Check risk limits
                    if self.risk_manager.can_trade(&opportunity)? {
                        // Execute trade
                        self.order_manager.execute_arbitrage(opportunity).await?;
                    }
                }
                
                // Update telemetry
                self.telemetry.update(&self.orderbook_aggregator, &self.order_manager)?;
            }
        }
    }
    
    /// Graceful shutdown procedure
    async fn shutdown(&self) -> Result<()> {
        info!("Starting graceful shutdown...");
        
        // Cancel all pending orders
        self.order_manager.cancel_all_orders().await?;
        
        // Close all positions if configured
        if self.config.close_on_shutdown {
            self.order_manager.close_all_positions().await?;
        }
        
        // Disconnect WebSocket
        self.ws_manager.disconnect().await?;
        
        // Save telemetry data
        self.telemetry.save_final_report()?;
        
        Ok(())
    }
}