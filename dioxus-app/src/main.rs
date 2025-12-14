//! # StarsCalendars Dioxus Authentication App
//!
//! Fullstack Dioxus 0.7 application with Telegram-only authentication,
//! user profile management, admin interfaces, and 10-language i18n support.
//!
//! ## Architecture
//! - Signals-based reactivity (Dioxus 0.7 alpha)
//! - Server Functions for type-safe RPC
//! - JWT RS256 authentication
//! - Telegram-only authentication flow
//! - Premium subscription verification
//! - Multilingual support with cultural adaptations

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use starscalendars_domain::*;
use std::collections::HashMap;

mod components;
mod server_functions;
mod i18n;
mod utils;

use components::*;
use server_functions::*;
use i18n::*;

/// Performance timer for monitoring Dioxus operations
pub struct PerformanceTimer {
    operation_name: String,
    start_time: std::time::Instant,
}

impl PerformanceTimer {
    pub fn new(operation_name: &str) -> Self {
        tracing::debug!("🚀 Dioxus 0.7: Starting {}", operation_name);

        Self {
            operation_name: operation_name.to_string(),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn mark(&self, checkpoint: &str) {
        let duration = self.start_time.elapsed().as_secs_f64() * 1000.0;
        tracing::debug!("📊 Dioxus 0.7: {} - {} at {:.3}ms",
            self.operation_name, checkpoint, duration);
    }
}

impl Drop for PerformanceTimer {
    fn drop(&mut self) {
        let duration = self.start_time.elapsed().as_secs_f64() * 1000.0;
        tracing::debug!("⏱️ Dioxus 0.7: {} completed in {:.3}ms",
            self.operation_name, duration);
    }
}

/// Main application state using Dioxus 0.7 signals
#[derive(Debug, Clone)]
pub struct AppState {
    pub user: Option<User>,
    pub auth_status: AuthStatus,
    pub language: String,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub subscription_status: SubscriptionStatus,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            user: None,
            auth_status: AuthStatus::Unauthenticated,
            language: "en".to_string(),
            is_loading: false,
            error_message: None,
            subscription_status: SubscriptionStatus::Free,
        }
    }
}

/// User subscription status for premium features
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    Free,
    Premium,
    Admin,
}

/// Available dashboard tabs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardTab {
    Overview,
    Profile,
    Spiritual,
    Admin,
    Settings,
}

/// Authentication flow steps
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStep {
    Welcome,
    TelegramLink,
    ProfileSetup,
    Completed,
}

fn main() {
    // Initialize comprehensive tracing for production
    tracing_subscriber::fmt::init();

    println!("🌟 Starting StarsCalendars Dioxus 0.7 Authentication App");

    // Launch Dioxus app with feature-based configuration
    #[cfg(feature = "web")]
    dioxus_web::launch(App);

    #[cfg(feature = "server")]
    {
        match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(async {
                tracing::info!("🚀 Starting Dioxus 0.7 server mode");

                // Initialize server with Axum integration
                if let Err(e) = start_server().await {
                    tracing::error!("❌ Server startup failed: {}", e);
                    std::process::exit(1);
                }
            }),
            Err(e) => {
                eprintln!("❌ Failed to create Tokio runtime: {}", e);
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(any(feature = "web", feature = "server")))]
    {
        println!("⚠️  No feature enabled. Use --features web or --features server");
    }
}

async fn start_server() -> anyhow::Result<()> {
    // TODO: Initialize Axum server with Dioxus integration
    // This will include JWT middleware, Telegram webhook endpoints,
    // and Server Function handlers
    tracing::warn!("⚠️  Server integration with Axum not implemented yet");

    // Simulate server running
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    Ok(())
}

#[component]
fn App() -> Element {
    let _timer = PerformanceTimer::new("app_initialization");

    // Use Dioxus 0.7 signals for state management
    let mut app_state = use_signal(|| AppState::default());
    let mut i18n_service = use_signal(|| I18nService::default());

    // Initialize authentication state on app startup
    use_effect(move || {
        spawn(async move {
            let _auth_timer = PerformanceTimer::new("auth_initialization");

            // Check for existing authentication
            match get_current_user().await {
                Ok(Some(user)) => {
                    app_state.write().user = Some(user.clone());
                    app_state.write().auth_status = AuthStatus::Authenticated {
                        user_id: user.id,
                        telegram_linked: user.telegram_user_id.is_some(),
                        subscription_active: false, // Will be checked separately
                    };

                    // Check subscription status
                    if let Ok(status) = check_subscription_status().await {
                        app_state.write().subscription_status = status;
                    }
                },
                Ok(None) => {
                    tracing::debug!("No authenticated user found");
                },
                Err(e) => {
                    tracing::error!("Authentication check failed: {}", e);
                    app_state.write().error_message = Some(
                        "Failed to check authentication status".to_string()
                    );
                }
            }
        });
    });

    rsx! {
        div {
            class: "app-container",
            style: "min-height: 100vh; background: linear-gradient(135deg, #1e3c72 0%, #2a5298 100%); font-family: 'Inter', sans-serif;",

            // Header with language selector and user info
            Header {
                app_state: app_state,
                i18n_service: i18n_service,
            }

            // Main content area
            main {
                class: "main-content",
                style: "padding: 2rem; max-width: 1200px; margin: 0 auto;",

                match app_state.read().auth_status {
                    AuthStatus::Authenticated { .. } => rsx! {
                        UserDashboard {
                            app_state: app_state,
                            i18n_service: i18n_service,
                        }
                    },
                    _ => rsx! {
                        AuthenticationFlow {
                            app_state: app_state,
                            i18n_service: i18n_service,
                        }
                    }
                }
            }

            // Global loading overlay
            if app_state.read().is_loading {
                LoadingOverlay {
                    i18n_service: i18n_service,
                }
            }

            // Global error modal
            if let Some(error) = &app_state.read().error_message {
                ErrorModal {
                    message: error.clone(),
                    i18n_service: i18n_service,
                    on_close: move || {
                        app_state.write().error_message = None;
                    }
                }
            }
        }
    }
}
