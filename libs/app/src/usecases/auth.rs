//! Authentication use cases

use crate::{
    AppResult, AppServices,
    ports::{CacheServiceExt, JwtService, TelegramService, TokenRepository, UserRepository},
};
use async_trait::async_trait;
use starscalendars_domain::*;
use std::sync::Arc;

/// Authentication use case interface
#[async_trait]
pub trait AuthUseCase: Send + Sync {
    async fn authenticate_with_telegram(&self, telegram_user_id: i64) -> AppResult<JwtTokenPair>;
    async fn refresh_tokens(&self, refresh_token: &str) -> AppResult<JwtTokenPair>;
    async fn create_linking_token(&self, user_id: &UserId) -> AppResult<uuid::Uuid>;
    async fn link_telegram_account(
        &self,
        token: &uuid::Uuid,
        telegram_user_id: i64,
    ) -> AppResult<()>;
    async fn check_subscription_status(&self, telegram_user_id: i64) -> AppResult<bool>;
}

/// Implementation of authentication use case
pub struct AuthUseCaseImpl {
    services: AppServices,
    telegram_channel: String,
}

impl AuthUseCaseImpl {
    pub fn new(services: AppServices, telegram_channel: String) -> Self {
        Self {
            services,
            telegram_channel,
        }
    }
}

#[async_trait]
impl AuthUseCase for AuthUseCaseImpl {
    async fn authenticate_with_telegram(&self, telegram_user_id: i64) -> AppResult<JwtTokenPair> {
        let telegram_id =
            TelegramUserId::new(telegram_user_id).map_err(|e| crate::AppError::Domain(e))?;

        // Check if user exists or create new one
        let user = match self.get_or_create_user_by_telegram(telegram_id).await? {
            Some(user) => user,
            None => {
                return Err(crate::AppError::Internal(
                    "Failed to create user".to_string(),
                ));
            }
        };

        // Check subscription status
        let is_subscribed = self.check_subscription_status(telegram_user_id).await?;

        // Create JWT claims
        let roles = if is_subscribed {
            vec!["user".to_string(), "premium".to_string()]
        } else {
            vec!["user".to_string()]
        };
        let claims =
            JwtClaims::new_with_telegram(&user.id, Some(telegram_id), None, is_subscribed, &roles);

        // Generate tokens
        let access_token = self
            .services
            .jwt_service
            .create_access_token(&claims)
            .await?;
        let refresh_token_str = self.services.jwt_service.create_refresh_token().await?;

        // Store refresh token
        let refresh_token = RefreshToken::new(user.id, refresh_token_str.clone());
        self.services
            .token_repo
            .store_refresh_token(&refresh_token)
            .await?;

        Ok(JwtTokenPair::new(access_token, refresh_token_str, 900))
    }

    async fn refresh_tokens(&self, refresh_token: &str) -> AppResult<JwtTokenPair> {
        // Get and validate refresh token
        let token = self
            .services
            .token_repo
            .get_refresh_token(refresh_token)
            .await?
            .ok_or_else(|| crate::AppError::Domain(DomainError::InvalidRefreshToken))?;

        if !token.is_valid() {
            return Err(crate::AppError::Domain(DomainError::InvalidRefreshToken));
        }

        // Get user
        let user = self
            .services
            .user_repo
            .get_user_by_id(&token.user_id)
            .await?
            .ok_or_else(|| {
                crate::AppError::Domain(DomainError::UserNotFound(token.user_id.to_string()))
            })?;

        // Check subscription if Telegram linked
        let is_subscribed = if let Some(telegram_id) = user.telegram_user_id {
            self.check_subscription_status(telegram_id.as_i64()).await?
        } else {
            false
        };

        // Create new tokens
        let roles = if is_subscribed {
            vec!["user".to_string(), "premium".to_string()]
        } else {
            vec!["user".to_string()]
        };
        let claims = JwtClaims::new(&user.id, user.telegram_user_id, is_subscribed, &roles);

        let access_token = self
            .services
            .jwt_service
            .create_access_token(&claims)
            .await?;
        let new_refresh_token = self.services.jwt_service.create_refresh_token().await?;

        // Revoke old refresh token and store new one
        self.services
            .token_repo
            .revoke_refresh_token(refresh_token)
            .await?;
        let new_token = RefreshToken::new(user.id, new_refresh_token.clone());
        self.services
            .token_repo
            .store_refresh_token(&new_token)
            .await?;

        Ok(JwtTokenPair::new(access_token, new_refresh_token, 900))
    }

    async fn create_linking_token(&self, user_id: &UserId) -> AppResult<uuid::Uuid> {
        let token = LinkingToken::new(user_id.clone());
        let token_id = token.token;

        self.services.token_repo.store_linking_token(&token).await?;

        Ok(token_id)
    }

    async fn link_telegram_account(
        &self,
        token: &uuid::Uuid,
        telegram_user_id: i64,
    ) -> AppResult<()> {
        let telegram_id =
            TelegramUserId::new(telegram_user_id).map_err(|e| crate::AppError::Domain(e))?;

        // Get and validate linking token
        let linking_token = self
            .services
            .token_repo
            .get_linking_token(token)
            .await?
            .ok_or_else(|| crate::AppError::Domain(DomainError::InvalidRefreshToken))?;

        if !linking_token.is_valid() {
            return Err(crate::AppError::Domain(DomainError::InvalidRefreshToken));
        }

        // Get user and update with Telegram ID
        let mut user = self
            .services
            .user_repo
            .get_user_by_id(&linking_token.user_id)
            .await?
            .ok_or_else(|| {
                crate::AppError::Domain(DomainError::UserNotFound(
                    linking_token.user_id.to_string(),
                ))
            })?;

        user.telegram_user_id = Some(telegram_id);
        user.updated_at = time::OffsetDateTime::now_utc();

        self.services.user_repo.update_user(&user).await?;
        self.services
            .token_repo
            .mark_linking_token_used(token)
            .await?;

        Ok(())
    }

    async fn check_subscription_status(&self, telegram_user_id: i64) -> AppResult<bool> {
        // Check cache first
        let cache_key = format!("subscription:{}", telegram_user_id);
        if let Ok(Some(cached)) = self
            .services
            .cache_service
            .get_json::<bool>(&cache_key)
            .await
        {
            return Ok(cached);
        }

        // Check via Telegram API
        let is_subscribed = self
            .services
            .telegram_service
            .is_member_of_channel(telegram_user_id, &self.telegram_channel)
            .await?;

        // Cache result for 5 minutes
        let _ = self
            .services
            .cache_service
            .set_json(
                &cache_key,
                &is_subscribed,
                std::time::Duration::from_secs(300),
            )
            .await;

        Ok(is_subscribed)
    }
}

impl AuthUseCaseImpl {
    async fn get_or_create_user_by_telegram(
        &self,
        telegram_id: TelegramUserId,
    ) -> AppResult<Option<User>> {
        // Try to find existing user by Telegram ID
        // This would require a method in UserRepository to find by telegram_user_id
        // For now, create a new user

        let user_info = self
            .services
            .telegram_service
            .get_user_info(telegram_id.as_i64())
            .await?;

        let user = User {
            id: UserId::new(),
            username: user_info
                .username
                .map_or_else(|| format!("telegram_{}", telegram_id.as_i64()), |v| v),
            email: None,
            telegram_user_id: Some(telegram_id),
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
            is_active: true,
        };

        self.services.user_repo.create_user(user.clone()).await?;

        Ok(Some(user))
    }
}
