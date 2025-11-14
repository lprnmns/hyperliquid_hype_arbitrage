//! WebSocket Manager Module
//! 
//! Ultra-low latency WebSocket connection with auto-reconnect and optimizations

use anyhow::{Result, Context, bail};
use crate::config::Config;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, protocol::WebSocketConfig},
    MaybeTlsStream, WebSocketStream,
};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn, error, debug};

/// WebSocket message types from Hyperliquid
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "channel", content = "data")]
pub enum WsMessage {
    #[serde(rename = "l2Book")]
    L2Book(L2BookData),
    #[serde(rename = "trades")]
    Trades(Vec<TradeData>),
    #[serde(rename = "allMids")]
    AllMids(AllMidsData),
    #[serde(rename = "notification")]
    Notification(NotificationData),
    #[serde(rename = "webData2")]
    WebData2(WebData2),
    #[serde(rename = "candle")]
    Candle(CandleData),
    #[serde(rename = "orderUpdates")]
    OrderUpdates(OrderUpdateData),
    #[serde(rename = "user")]
    User(UserData),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct L2BookData {
    pub coin: String,
    pub time: u64,
    pub levels: Vec<Vec<Level>>,  // [bids, asks]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Level {
    pub px: String,
    pub sz: String,
    pub n: u32,  // number of orders
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradeData {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllMidsData {
    pub mids: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationData {
    pub notification: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebData2 {
    pub user_funding: Vec<Value>,
    pub meta_and_asset_ctxs: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CandleData {
    pub t: u64,
    pub o: String,
    pub h: String,
    pub l: String,
    pub c: String,
    pub v: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderUpdateData {
    pub order: Value,
    pub status: String,
    pub status_timestamp: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserData {
    pub fills: Vec<Value>,
    pub funding: Vec<Value>,
    pub liquidation: Option<Value>,
    pub non_funding_ledger_updates: Vec<Value>,
}

/// Subscription request structure
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionRequest {
    pub method: String,
    pub subscription: Subscription,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Subscription {
    L2Book { 
        #[serde(rename = "type")]
        sub_type: String, 
        coin: String 
    },
    AllMids {
        #[serde(rename = "type")]
        sub_type: String,
    },
    User {
        #[serde(rename = "type")]
        sub_type: String,
        user: String,
    },
    Trades {
        #[serde(rename = "type")]
        sub_type: String,
        coin: String,
    },
}

use std::collections::HashMap;

/// WebSocket connection manager with auto-reconnect
#[derive(Clone)]
pub struct Manager {
    config: Config,
    url: String,
    ws_stream: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    subscriptions: Arc<RwLock<Vec<Subscription>>>,
    message_tx: mpsc::UnboundedSender<WsMessage>,
    message_rx: Arc<Mutex<mpsc::UnboundedReceiver<WsMessage>>>,
    last_ping: Arc<RwLock<Instant>>,
    reconnect_count: Arc<RwLock<u32>>,
}

impl Manager {
    /// Create a new WebSocket manager
    pub async fn new(config: &Config) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        
        Ok(Manager {
            config: config.clone(),
            url: config.ws_url.clone(),
            ws_stream: Arc::new(Mutex::new(None)),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            message_tx: tx,
            message_rx: Arc::new(Mutex::new(rx)),
            last_ping: Arc::new(RwLock::new(Instant::now())),
            reconnect_count: Arc::new(RwLock::new(0)),
        })
    }
    
    /// Connect to WebSocket with optimizations
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to WebSocket at {}", self.url);
        
        // Connect with timeout - use URL string directly
        let connect_future = connect_async(&self.url);
        let (ws_stream, response) = tokio::time::timeout(
            Duration::from_secs(10),
            connect_future
        )
        .await
        .context("Connection timeout")?
        .context("Failed to connect")?;
        
        info!("WebSocket connected with status: {}", response.status());
        
        // Store the stream
        *self.ws_stream.lock().await = Some(ws_stream);
        
        // Reset reconnect count on successful connection
        *self.reconnect_count.write().await = 0;
        
        // Start message handler in background
        let ws_stream = self.ws_stream.clone();
        let message_tx = self.message_tx.clone();
        let last_ping = self.last_ping.clone();
        let url = self.url.clone();
        let subscriptions = self.subscriptions.clone();
        let reconnect_count = self.reconnect_count.clone();
        
        tokio::spawn(async move {
            Self::message_handler_loop(
                ws_stream,
                message_tx,
                last_ping,
                url,
                subscriptions,
                reconnect_count,
            ).await;
        });
        
        // Start ping task
        self.start_ping_task().await;
        
        // Resubscribe to all channels
        self.resubscribe_all().await?;
        
        Ok(())
    }
    
    /// Message handler loop (static method for spawning)
    async fn message_handler_loop(
        ws_stream: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
        message_tx: mpsc::UnboundedSender<WsMessage>,
        last_ping: Arc<RwLock<Instant>>,
        url: String,
        subscriptions: Arc<RwLock<Vec<Subscription>>>,
        reconnect_count: Arc<RwLock<u32>>,
    ) {
        loop {
            let mut stream_guard = ws_stream.lock().await;
            
            if let Some(stream) = stream_guard.as_mut() {
                // Read next message
                match stream.next().await {
                    Some(Ok(Message::Text(text))) => {
                        // Update last ping time
                        *last_ping.write().await = Instant::now();
                        
                        // Parse and forward message
                        if let Ok(msg) = serde_json::from_str::<WsMessage>(&text) {
                            debug!("Received message: {:?}", msg);
                            let _ = message_tx.send(msg);
                        } else if text.contains("pong") {
                            debug!("Received pong");
                        } else {
                            debug!("Unknown message: {}", text);
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // Respond to ping
                        if let Err(e) = stream.send(Message::Pong(data)).await {
                            error!("Failed to send pong: {}", e);
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        warn!("WebSocket closed by server");
                        drop(stream_guard);
                        Self::perform_reconnect(
                            &ws_stream,
                            &url,
                            &subscriptions,
                            &reconnect_count,
                        ).await;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        drop(stream_guard);
                        Self::perform_reconnect(
                            &ws_stream,
                            &url,
                            &subscriptions,
                            &reconnect_count,
                        ).await;
                    }
                    None => {
                        warn!("WebSocket stream ended");
                        drop(stream_guard);
                        Self::perform_reconnect(
                            &ws_stream,
                            &url,
                            &subscriptions,
                            &reconnect_count,
                        ).await;
                    }
                    _ => {}
                }
            } else {
                // No active connection, try to reconnect
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    /// Perform reconnection (static helper)
    async fn perform_reconnect(
        ws_stream: &Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
        url: &str,
        subscriptions: &Arc<RwLock<Vec<Subscription>>>,
        reconnect_count: &Arc<RwLock<u32>>,
    ) {
        let mut count = reconnect_count.write().await;
        *count += 1;
        
        let delay = Duration::from_millis(100 * 2_u64.pow((*count).min(10)));
        warn!("Reconnecting in {:?} (attempt #{})", delay, count);
        
        tokio::time::sleep(delay).await;
        
        // Try to reconnect
        if let Ok((new_stream, _)) = connect_async(url).await {
            *ws_stream.lock().await = Some(new_stream);
            *reconnect_count.write().await = 0;
            
            // Resubscribe
            let subs = subscriptions.read().await.clone();
            let mut stream_guard = ws_stream.lock().await;
            if let Some(stream) = stream_guard.as_mut() {
                for sub in subs {
                    let request = SubscriptionRequest {
                        method: "subscribe".to_string(),
                        subscription: sub,
                    };
                    let json = serde_json::to_string(&request).unwrap_or_default();
                    let _ = stream.send(Message::Text(json)).await;
                }
            }
        }
    }
    
    /// Start periodic ping task
    async fn start_ping_task(&self) {
        let ws_stream = self.ws_stream.clone();
        let last_ping = self.last_ping.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Check if we've received data recently
                let last = *last_ping.read().await;
                if last.elapsed() > Duration::from_secs(60) {
                    warn!("No data received for 60s, sending ping");
                    
                    let mut stream_guard = ws_stream.lock().await;
                    if let Some(stream) = stream_guard.as_mut() {
                        if let Err(e) = stream.send(Message::Text("{\"method\":\"ping\"}".to_string())).await {
                            error!("Failed to send ping: {}", e);
                        }
                    }
                }
            }
        });
    }
    
    
    /// Resubscribe to all channels after reconnection
    async fn resubscribe_all(&self) -> Result<()> {
        let subs = self.subscriptions.read().await.clone();
        
        for sub in subs {
            self.send_subscription(sub).await?;
        }
        
        Ok(())
    }
    
    /// Send subscription request
    async fn send_subscription(&self, subscription: Subscription) -> Result<()> {
        let request = SubscriptionRequest {
            method: "subscribe".to_string(),
            subscription: subscription.clone(),
        };
        
        let json = serde_json::to_string(&request)?;
        
        let mut stream_guard = self.ws_stream.lock().await;
        if let Some(stream) = stream_guard.as_mut() {
            stream.send(Message::Text(json)).await
                .context("Failed to send subscription")?;
            info!("Subscribed to {:?}", subscription);
        } else {
            bail!("No active WebSocket connection");
        }
        
        Ok(())
    }
    
    /// Subscribe to L2 order book
    pub async fn subscribe_l2_book(&self, symbol: &str) -> Result<()> {
        let subscription = Subscription::L2Book {
            sub_type: "l2Book".to_string(),
            coin: symbol.to_string(),
        };
        
        self.subscriptions.write().await.push(subscription.clone());
        self.send_subscription(subscription).await
    }
    
    /// Subscribe to all mid prices
    pub async fn subscribe_all_mids(&self) -> Result<()> {
        let subscription = Subscription::AllMids {
            sub_type: "allMids".to_string(),
        };
        
        self.subscriptions.write().await.push(subscription.clone());
        self.send_subscription(subscription).await
    }
    
    /// Subscribe to user events
    pub async fn subscribe_user_events(&self) -> Result<()> {
        let subscription = Subscription::User {
            sub_type: "user".to_string(),
            user: self.config.wallet_address.clone(),
        };
        
        self.subscriptions.write().await.push(subscription.clone());
        self.send_subscription(subscription).await
    }
    
    /// Subscribe to trades
    pub async fn subscribe_trades(&self, symbol: &str) -> Result<()> {
        let subscription = Subscription::Trades {
            sub_type: "trades".to_string(),
            coin: symbol.to_string(),
        };
        
        self.subscriptions.write().await.push(subscription.clone());
        self.send_subscription(subscription).await
    }
    
    /// Receive next message
    pub async fn receive_message(&self) -> Result<Option<String>> {
        let mut rx = self.message_rx.lock().await;
        
        match rx.recv().await {
            Some(msg) => Ok(Some(serde_json::to_string(&msg)?)),
            None => Ok(None),
        }
    }
    
    /// Send order via WebSocket
    pub async fn send_order(&self, order_json: &str) -> Result<()> {
        let mut stream_guard = self.ws_stream.lock().await;
        
        if let Some(stream) = stream_guard.as_mut() {
            stream.send(Message::Text(order_json.to_string())).await
                .context("Failed to send order")?;
            debug!("Order sent: {}", order_json);
        } else {
            bail!("No active WebSocket connection");
        }
        
        Ok(())
    }
    
    /// Disconnect WebSocket
    pub async fn disconnect(&self) -> Result<()> {
        let mut stream_guard = self.ws_stream.lock().await;
        
        if let Some(stream) = stream_guard.as_mut() {
            stream.close(None).await
                .context("Failed to close WebSocket")?;
        }
        
        *stream_guard = None;
        info!("WebSocket disconnected");
        
        Ok(())
    }
    
    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        self.ws_stream.lock().await.is_some()
    }
    
    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        ConnectionStats {
            connected: self.is_connected().await,
            last_ping: *self.last_ping.read().await,
            reconnect_count: *self.reconnect_count.read().await,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub connected: bool,
    pub last_ping: Instant,
    pub reconnect_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_manager_creation() {
        let config = Config::default();
        let manager = Manager::new(&config).await;
        assert!(manager.is_ok());
    }
}