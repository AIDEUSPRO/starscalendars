//! WebSocket handlers for real-time communication
//!
//! Handles WebSocket connections for live astronomical data streaming.
//! Supports 1000+ concurrent connections with JWT authentication.

use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use futures::{future::join_all, sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use starscalendars_app::*;
use starscalendars_domain::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{error, info, warn};
use uuid::Uuid;

/// WebSocket message types for spiritual astronomy platform
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    /// Authentication message (must be first message)
    Auth { token: String },
    /// Authentication successful
    AuthSuccess { user_id: String },
    /// Authentication failed
    AuthError { message: String },
    /// Astronomical data update
    AstronomicalUpdate {
        julian_day: f64,
        planetary_positions: Vec<PlanetaryPosition>,
    },
    /// Spiritual event notification
    SpiritualEvent {
        event_id: String,
        event_type: String,
        title: String,
        occurs_at: String,
    },
    /// User subscription status update
    SubscriptionUpdate {
        is_subscribed: bool,
        channel_name: String,
    },
    /// Ping for connection keepalive
    Ping,
    /// Pong response
    Pong,
    /// Connection closing
    Close { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetaryPosition {
    pub body: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub magnitude: Option<f64>,
}

/// WebSocket connection state
#[derive(Debug)]
struct WebSocketConnection {
    connection_id: Uuid,
    user_id: Option<UserId>,
    sender: tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    authenticated: bool,
    connected_at: time::OffsetDateTime,
    last_activity: std::sync::Arc<std::sync::atomic::AtomicI64>,
    language: String,
}

/// WebSocket manager for handling 1000+ concurrent connections
#[derive(Clone)]
pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
    user_connections: Arc<RwLock<HashMap<UserId, Vec<Uuid>>>>, // user_id -> connection_ids
    jwt_service: Arc<dyn JwtService + Send + Sync>,
}

impl WebSocketManager {
    /// Create new WebSocket manager
    pub fn new(jwt_service: Arc<dyn JwtService + Send + Sync>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000); // High capacity for real-time updates

        Self {
            connections: Arc::new(RwLock::new(HashMap::with_capacity(1000))),
            broadcast_tx,
            user_connections: Arc::new(RwLock::new(HashMap::with_capacity(500))),
            jwt_service,
        }
    }

    /// Add new WebSocket connection
    pub async fn add_connection(
        &self,
        sender: tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    ) -> Uuid {
        let connection_id = Uuid::new_v4();

        let connection = WebSocketConnection {
            connection_id,
            user_id: None,
            sender,
            authenticated: false,
            connected_at: time::OffsetDateTime::now_utc(),
            last_activity: std::sync::Arc::new(std::sync::atomic::AtomicI64::new(
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )),
            language: "en".to_string(), // Default language
        };

        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection);

        info!("🔗 WebSocket connection added: {}", connection_id);
        connection_id
    }

    /// Authenticate WebSocket connection
    pub async fn authenticate_connection(
        &self,
        connection_id: &Uuid,
        token: &str,
    ) -> Result<UserId, AppError> {
        // Validate JWT token
        let claims = self.jwt_service.validate_access_token(token).await?;

        let mut connections = self.connections.write().await;
        let mut user_connections = self.user_connections.write().await;

        if let Some(connection) = connections.get_mut(connection_id) {
            connection.user_id = Some(claims.user_id.clone());
            connection.authenticated = true;
            connection.language = match claims.language {
                Some(lang) => lang,
                None => "en".to_string(),
            };

            // Track user connections for targeted messaging
            user_connections
                .entry(claims.user_id.clone())
                .or_insert_with(|| Vec::with_capacity(1))
                .push(*connection_id);

            info!(
                "✅ WebSocket connection authenticated: {} for user {}",
                connection_id,
                claims.user_id.as_uuid()
            );
            Ok(claims.user_id)
        } else {
            Err(AppError::Authentication("Connection not found".to_string()))
        }
    }

    /// Remove WebSocket connection
    pub async fn remove_connection(&self, connection_id: &Uuid) {
        let mut connections = self.connections.write().await;
        let mut user_connections = self.user_connections.write().await;

        if let Some(connection) = connections.remove(connection_id) {
            // Remove from user connections tracking
            if let Some(user_id) = &connection.user_id {
                if let Some(user_conn_list) = user_connections.get_mut(user_id) {
                    user_conn_list.retain(|&id| id != *connection_id);
                    if user_conn_list.is_empty() {
                        user_connections.remove(user_id);
                    }
                }
            }

            info!("🔌 WebSocket connection removed: {}", connection_id);
        }
    }

    /// Broadcast message to all authenticated connections
    pub async fn broadcast(&self, message: WebSocketMessage) {
        if let Err(e) = self.broadcast_tx.send(message) {
            error!("Failed to broadcast message: {}", e);
        }
    }

    /// Send message to specific user's connections
    pub async fn send_to_user(&self, user_id: &UserId, message: WebSocketMessage) {
        let user_connections = self.user_connections.read().await;
        let connections = self.connections.read().await;

        if let Some(connection_ids) = user_connections.get(user_id) {
            for &connection_id in connection_ids {
                if let Some(connection) = connections.get(&connection_id) {
                    if let Err(e) = connection.sender.send(message.clone()) {
                        warn!(
                            "Failed to send message to connection {}: {}",
                            connection_id, e
                        );
                    }
                }
            }
        }
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> (usize, usize) {
        let connections = self.connections.read().await;
        let authenticated_count = connections
            .values()
            .filter(|conn| conn.authenticated)
            .count();

        (connections.len(), authenticated_count)
    }

    /// Cleanup inactive connections
    pub async fn cleanup_inactive(&self, timeout_secs: i64) {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        // ✅ QUALITY: Pre-allocate capacity for connection cleanup
        // Estimated capacity: assume max 10% of connections might be inactive at once
        let estimated_inactive = self.connections.read().await.len() / 10 + 1;
        let mut to_remove = Vec::with_capacity(estimated_inactive);

        {
            let connections = self.connections.read().await;
            for (connection_id, connection) in connections.iter() {
                let last_activity = connection
                    .last_activity
                    .load(std::sync::atomic::Ordering::Relaxed);
                if now - last_activity > timeout_secs {
                    to_remove.push(*connection_id);
                }
            }
        }

        let removal_futures = to_remove
            .into_iter()
            .map(|connection_id| self.remove_connection(&connection_id));
        join_all(removal_futures).await;
    }
}

/// WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(manager): State<Arc<WebSocketManager>>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, manager))
}

/// Handle individual WebSocket connection
async fn handle_websocket(socket: WebSocket, manager: Arc<WebSocketManager>) {
    let (mut sender, mut receiver) = socket.split();

    // Create communication channel for this connection
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<WebSocketMessage>();

    // Add connection to manager
    let connection_id = manager.add_connection(tx.clone()).await;

    // Clone manager for tasks
    let manager_recv = manager.clone();
    let manager_send = manager.clone();

    // Task for sending messages to client
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let msg_text = match serde_json::to_string(&message) {
                Ok(text) => text,
                Err(e) => {
                    error!("Failed to serialize WebSocket message: {}", e);
                    continue;
                }
            };

            if sender.send(Message::Text(msg_text)).await.is_err() {
                break;
            }
        }
    });

    // Task for receiving messages from client
    let recv_task = tokio::spawn(async move {
        let mut authenticated = false;

        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    let ws_message: Result<WebSocketMessage, _> = serde_json::from_str(&text);

                    match ws_message {
                        Ok(WebSocketMessage::Auth { token }) => {
                            if !authenticated {
                                match manager_recv
                                    .authenticate_connection(&connection_id, &token)
                                    .await
                                {
                                    Ok(user_id) => {
                                        authenticated = true;
                                        let response = WebSocketMessage::AuthSuccess {
                                            user_id: user_id.as_uuid().to_string(),
                                        };
                                        if tx.send(response).is_err() {
                                            break;
                                        }
                                        info!(
                                            "🔐 WebSocket authenticated for user: {}",
                                            user_id.as_uuid()
                                        );
                                    }
                                    Err(e) => {
                                        let response = WebSocketMessage::AuthError {
                                            message: e.to_string(),
                                        };
                                        if tx.send(response).is_err() {
                                            break;
                                        }
                                        warn!("🚫 WebSocket authentication failed: {}", e);
                                    }
                                }
                            }
                        }
                        Ok(WebSocketMessage::Ping) => {
                            if tx.send(WebSocketMessage::Pong).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {
                            if !authenticated {
                                let response = WebSocketMessage::AuthError {
                                    message: "Authentication required".to_string(),
                                };
                                if tx.send(response).is_err() {
                                    break;
                                }
                            }
                            // Handle other authenticated messages here
                        }
                        Err(e) => {
                            error!("Failed to parse WebSocket message: {}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!(
                        "🔌 WebSocket connection closed by client: {}",
                        connection_id
                    );
                    break;
                }
                Ok(Message::Ping(data)) => {
                    if sender.send(Message::Pong(data)).await.is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {
            info!("WebSocket send task completed for {}", connection_id);
        }
        _ = recv_task => {
            info!("WebSocket receive task completed for {}", connection_id);
        }
    }

    // Cleanup connection
    manager.remove_connection(&connection_id).await;
}
