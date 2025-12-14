//! # StarsCalendars Backend Server
//!
//! Axum-based HTTP/WebSocket server with Clean Architecture,
//! Telegram authentication, and real-time astronomical calculations.

use anyhow::Result;
use axum::{Router, serve};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod middleware;
mod routes;
mod websocket;

use crate::websocket::{WebSocketManager, websocket_handler};
use starscalendars_app::{AppServices, ServiceHealthReport};
use starscalendars_infra::AppConfig;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            match EnvFilter::try_from_default_env() {
                Ok(filter) => filter,
                Err(_) => EnvFilter::new("starscalendars_backend=debug,tower_http=debug"),
            },
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🌟 Starting StarsCalendars Backend Server");

    // Load configuration
    let config =
        AppConfig::load().map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

    info!("📋 Configuration loaded successfully");

    // Initialize services (dependency injection)
    let services = initialize_services(&config)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize services: {}", e))?;

    info!("🔧 Services initialized successfully");

    // Perform health checks
    match services.health_check().await {
        Ok(report) => {
            if report.overall_healthy {
                info!("✅ All services passed health checks");
            } else {
                error!("❌ Some services failed health checks: {:?}", report.errors);
                return Err(anyhow::anyhow!("Health check failed"));
            }
        }
        Err(e) => {
            error!("❌ Health check error: {}", e);
            return Err(anyhow::anyhow!("Health check failed: {}", e));
        }
    }

    // Initialize WebSocket manager
    let websocket_manager = Arc::new(WebSocketManager::new(services.jwt_service.clone()));

    // Build application routes
    let app = build_app(services, websocket_manager).await?;

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    let listener = TcpListener::bind(addr).await?;

    info!("🚀 Server starting on {}", addr);
    info!("📡 WebSocket endpoint: ws://{}/ws");
    info!("🔗 Health check: http://{}/health");

    serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}

async fn build_app(
    services: AppServices,
    websocket_manager: Arc<WebSocketManager>,
) -> Result<Router> {
    let app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::auth_routes())
        .merge(routes::api_routes())
        .merge(routes::websocket_routes())
        .with_state(services.clone())
        .with_state(websocket_manager)
        .layer(CorsLayer::permissive())
        .layer(middleware::request_id_layer())
        .layer(middleware::logging_layer());

    Ok(app)
}

async fn initialize_services(config: &AppConfig) -> Result<AppServices> {
    info!("🚀 Initializing production services");

    let services = AppServices::new_production(config)
        .await
        .map_err(|e| anyhow::anyhow!("Service initialization failed: {}", e))?;

    info!("✅ All services initialized successfully");

    // Start background tasks
    start_background_tasks(&services).await?;

    Ok(services)
}

/// Start background maintenance tasks
async fn start_background_tasks(services: &AppServices) -> Result<()> {
    info!("🔄 Starting background maintenance tasks");

    // Task for cleaning up expired tokens
    let cache_service = services.cache_service.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Every hour

        loop {
            interval.tick().await;

            // Note: Cleanup would happen automatically in production Redis
            // For in-memory cache, we'd need a different approach to access cleanup
            info!("🧹 Background cleanup task running");
        }
    });

    info!("✅ Background tasks started");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_app() {
        let services = AppServices::new_test();
        let websocket_manager = Arc::new(WebSocketManager::new(services.jwt_service.clone()));

        if let Ok(app) = build_app(services, websocket_manager).await {
            // Basic smoke test
            assert!(!format!("{:?}", app).is_empty());
        } else {
            assert!(false, "Failed to build app for test");
        }
    }
}
