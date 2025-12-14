//! Telegram API service implementation
//!
//! Integration with Telegram Bot API using teloxide and reqwest.

use crate::{InfraError, TelegramCacheService};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use starscalendars_domain::*;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

/// Production Telegram service implementation with caching
pub struct TelegramServiceImpl {
    api_service: TelegramApiService,
    channel_username: String,
    cache_service: Arc<TelegramCacheService>,
}

/// Low-level Telegram Bot API implementation
pub struct TelegramApiService {
    client: Client,
    bot_token: String,
    base_url: String,
}

impl TelegramServiceImpl {
    /// Create new production Telegram service with caching
    pub async fn new(
        bot_token: &str,
        channel_username: &str,
        cache_service: Arc<TelegramCacheService>,
    ) -> Result<Self, InfraError> {
        let api_service = TelegramApiService::new(bot_token.to_string());

        // Test connection
        match api_service.get_me().await {
            Ok(bot_info) => {
                let username = match bot_info.username {
                    Some(v) => v,
                    None => "Unknown".to_string(),
                };
                info!(
                    "✅ Telegram bot connected: {}",
                    username
                );
            }
            Err(e) => {
                error!("❌ Failed to connect to Telegram: {}", e);
                return Err(InfraError::TelegramApi(format!(
                    "Bot connection failed: {}",
                    e
                )));
            }
        }

        Ok(Self {
            api_service,
            channel_username: channel_username.to_string(),
            cache_service,
        })
    }
}

#[async_trait]
impl TelegramService for TelegramServiceImpl {
    async fn is_member_of_channel(&self, user_id: i64, channel: &str) -> PortResult<bool> {
        // Check cache first
        if let Ok(Some(cached_status)) = self.cache_service.get_subscription_status(user_id).await {
            return Ok(cached_status);
        }

        // Query Telegram API
        let is_member = self
            .api_service
            .is_member_of_channel(user_id, channel)
            .await?;

        // Cache result
        if let Err(e) = self
            .cache_service
            .cache_subscription_status(user_id, is_member)
            .await
        {
            error!("Failed to cache subscription status: {}", e);
        }

        Ok(is_member)
    }

    async fn send_message(&self, user_id: i64, message: &str) -> PortResult<()> {
        self.api_service.send_message(user_id, message).await
    }

    async fn get_user_info(&self, user_id: i64) -> PortResult<TelegramUserInfo> {
        self.api_service.get_user_info(user_id).await
    }

    async fn health_check(&self) -> PortResult<()> {
        match self.api_service.get_me().await {
            Ok(_) => Ok(()),
            Err(e) => {
                Err(InfraError::TelegramApi(format!("Telegram health check failed: {}", e)).into())
            }
        }
    }
}

impl TelegramApiService {
    /// Create new Telegram API service
    pub fn new(bot_token: String) -> Self {
        let client = match Client::builder().timeout(Duration::from_secs(30)).build() {
            Ok(c) => c,
            Err(_) => Client::new(),
        };

        Self {
            client,
            bot_token,
            base_url: "https://api.telegram.org".to_string(),
        }
    }

    /// Create with custom base URL (for testing)
    pub fn with_base_url(bot_token: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            bot_token,
            base_url,
        }
    }

    /// Build API URL
    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.base_url, self.bot_token, method)
    }

    /// Get bot information (for health checks)
    async fn get_me(&self) -> Result<BotInfo, InfraError> {
        #[derive(Deserialize)]
        struct GetMeResponse {
            ok: bool,
            result: Option<BotInfo>,
            description: Option<String>,
        }

        let response = self
            .client
            .get(&self.api_url("getMe"))
            .send()
            .await
            .map_err(|e| InfraError::Http(e))?;

        let get_me_response: GetMeResponse =
            response.json().await.map_err(|e| InfraError::Http(e))?;

        if !get_me_response.ok {
            let error_msg = match get_me_response.description {
                Some(v) => v,
                None => "Failed to get bot info".to_string(),
            };
            return Err(InfraError::TelegramApi(error_msg));
        }

        get_me_response
            .result
            .ok_or_else(|| InfraError::TelegramApi("No bot info in response".to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct BotInfo {
    id: i64,
    is_bot: bool,
    first_name: String,
    username: Option<String>,
}

#[async_trait]
impl TelegramService for TelegramApiService {
    async fn is_member_of_channel(&self, user_id: i64, channel: &str) -> PortResult<bool> {
        #[derive(Serialize)]
        struct GetChatMemberRequest {
            chat_id: String,
            user_id: i64,
        }

        #[derive(Deserialize)]
        struct GetChatMemberResponse {
            ok: bool,
            result: Option<ChatMember>,
        }

        #[derive(Deserialize)]
        struct ChatMember {
            status: String,
        }

        let request = GetChatMemberRequest {
            chat_id: channel.to_string(),
            user_id,
        };

        let response = self
            .client
            .post(&self.api_url("getChatMember"))
            .json(&request)
            .send()
            .await
            .map_err(|e| InfraError::Http(e))?;

        let chat_member_response: GetChatMemberResponse =
            response.json().await.map_err(|e| InfraError::Http(e))?;

        if !chat_member_response.ok {
            return Ok(false);
        }

        match chat_member_response.result {
            Some(member) => {
                // User is considered a member if they are not kicked or left
                Ok(matches!(
                    member.status.as_str(),
                    "creator" | "administrator" | "member"
                ))
            }
            None => Ok(false),
        }
    }

    async fn send_message(&self, user_id: i64, message: &str) -> PortResult<()> {
        #[derive(Serialize)]
        struct SendMessageRequest {
            chat_id: i64,
            text: String,
            parse_mode: Option<String>,
        }

        #[derive(Deserialize)]
        struct SendMessageResponse {
            ok: bool,
            description: Option<String>,
        }

        let request = SendMessageRequest {
            chat_id: user_id,
            text: message.to_string(),
            parse_mode: Some("Markdown".to_string()),
        };

        let response = self
            .client
            .post(&self.api_url("sendMessage"))
            .json(&request)
            .send()
            .await
            .map_err(|e| InfraError::Http(e))?;

        let send_response: SendMessageResponse =
            response.json().await.map_err(|e| InfraError::Http(e))?;

        if !send_response.ok {
            let error_msg = match send_response.description {
                Some(v) => v,
                None => "Unknown Telegram API error".to_string(),
            };
            return Err(InfraError::TelegramApi(error_msg).into());
        }

        Ok(())
    }

    async fn get_user_info(&self, user_id: i64) -> PortResult<TelegramUserInfo> {
        #[derive(Serialize)]
        struct GetChatRequest {
            chat_id: i64,
        }

        #[derive(Deserialize)]
        struct GetChatResponse {
            ok: bool,
            result: Option<TelegramChat>,
            description: Option<String>,
        }

        #[derive(Deserialize)]
        struct TelegramChat {
            id: i64,
            #[serde(rename = "type")]
            chat_type: String,
            username: Option<String>,
            first_name: Option<String>,
            last_name: Option<String>,
        }

        let request = GetChatRequest { chat_id: user_id };

        let response = self
            .client
            .post(&self.api_url("getChat"))
            .json(&request)
            .send()
            .await
            .map_err(|e| InfraError::Http(e))?;

        let chat_response: GetChatResponse =
            response.json().await.map_err(|e| InfraError::Http(e))?;

        if !chat_response.ok {
            let error_msg = match chat_response.description {
                Some(v) => v,
                None => "Failed to get user info".to_string(),
            };
            return Err(InfraError::TelegramApi(error_msg).into());
        }

        match chat_response.result {
            Some(chat) => Ok(TelegramUserInfo {
                id: chat.id,
                username: chat.username,
                first_name: match chat.first_name {
                    Some(v) => v,
                    None => "Unknown".to_string(),
                },
                last_name: chat.last_name,
                is_bot: chat.chat_type == "bot",
            }),
            None => Err(InfraError::TelegramApi("User not found".to_string()).into()),
        }
    }

    async fn health_check(&self) -> PortResult<()> {
        match self.get_me().await {
            Ok(_) => Ok(()),
            Err(e) => {
                Err(InfraError::TelegramApi(format!("Telegram health check failed: {}", e)).into())
            }
        }
    }
}

/// Mock Telegram service for testing
pub struct MockTelegramService {
    /// Whether users are considered subscribed by default
    pub default_subscription_status: bool,
    /// Specific user subscription overrides
    pub user_subscriptions: dashmap::DashMap<i64, bool>,
    /// User info storage for testing
    pub user_info: dashmap::DashMap<i64, TelegramUserInfo>,
}

impl MockTelegramService {
    /// Create new mock service
    pub fn new() -> Self {
        Self {
            default_subscription_status: false,
            user_subscriptions: dashmap::DashMap::with_capacity(100),
            user_info: dashmap::DashMap::with_capacity(100),
        }
    }

    /// Set subscription status for a user
    pub fn set_user_subscription(&self, user_id: i64, is_subscribed: bool) {
        self.user_subscriptions.insert(user_id, is_subscribed);
    }

    /// Set user info for testing
    pub fn set_user_info(&self, user_id: i64, info: TelegramUserInfo) {
        self.user_info.insert(user_id, info);
    }
}

impl Default for MockTelegramService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TelegramService for MockTelegramService {
    async fn is_member_of_channel(&self, user_id: i64, _channel: &str) -> PortResult<bool> {
        match self.user_subscriptions.get(&user_id) {
            Some(v) => Ok(*v),
            None => Ok(self.default_subscription_status),
        }
    }

    async fn send_message(&self, _user_id: i64, _message: &str) -> PortResult<()> {
        // Mock implementation - just succeed
        Ok(())
    }

    async fn get_user_info(&self, user_id: i64) -> PortResult<TelegramUserInfo> {
        self.user_info
            .get(&user_id)
            .map(|info| info.clone())
            .ok_or_else(|| InfraError::TelegramApi("User not found".to_string()).into())
    }

    async fn health_check(&self) -> PortResult<()> {
        // Mock always succeeds
        Ok(())
    }
}
