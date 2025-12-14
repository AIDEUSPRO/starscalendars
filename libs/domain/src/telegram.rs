//! Telegram domain types for authentication and subscription management
//!
//! Pure domain logic for Telegram-only authentication system with
//! channel subscription verification and spiritual community features.

use crate::errors::DomainError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Telegram User ID (unique across all Telegram)
///
/// Primary identifier for users in our system - no traditional passwords.
/// All authentication flows through Telegram Bot API verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramUserId(pub i64);

impl TelegramUserId {
    /// Create new Telegram User ID with validation
    pub fn new(id: i64) -> Result<Self, DomainError> {
        if id > 0 {
            Ok(Self(id))
        } else {
            Err(DomainError::InvalidTelegramUserId(id))
        }
    }

    /// Get the underlying i64 value
    pub fn value(self) -> i64 {
        self.0
    }
}

/// Telegram Channel ID for subscription verification
///
/// Represents spiritual community channels that users must subscribe to
/// for premium features access. Uses getChatMember API for verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TelegramChannelId(pub i64);

impl TelegramChannelId {
    /// Create new Channel ID with validation
    pub fn new(id: i64) -> Result<Self, DomainError> {
        // Telegram channel IDs are typically negative numbers
        if id < 0 {
            Ok(Self(id))
        } else {
            Err(DomainError::InvalidTelegramChannelId(id))
        }
    }

    /// Get the underlying i64 value
    pub fn value(self) -> i64 {
        self.0
    }
}

/// Unique linking token for web-to-Telegram authentication flow
///
/// Generates UUID tokens for secure account linking without exposing
/// Telegram User IDs in web URLs. Expires after linking or timeout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkingToken(pub Uuid);

impl LinkingToken {
    /// Generate new random linking token
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn uuid(self) -> Uuid {
        self.0
    }

    /// Get string representation for URLs
    pub fn to_string(self) -> String {
        self.0.to_string()
    }
}

/// Subscription status for premium spiritual features
///
/// Represents current membership status in Telegram channels.
/// Cached for performance with periodic verification via getChatMember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    /// User is not subscribed to any channels
    NotSubscribed,
    /// User is subscribed and has access to premium features
    Active,
    /// Subscription expired or user left channel
    Expired,
    /// Subscription verification failed (API error)
    VerificationFailed,
}

impl SubscriptionStatus {
    /// Check if user has access to premium features
    pub fn has_premium_access(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if subscription needs re-verification
    pub fn needs_verification(self) -> bool {
        matches!(self, Self::Expired | Self::VerificationFailed)
    }
}

/// Complete Telegram user profile with authentication context
///
/// Contains all Telegram-related information for authenticated users.
/// Links web session to Telegram identity with subscription status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramProfile {
    pub user_id: TelegramUserId,
    pub username: Option<String>, // Telegram @username (optional)
    pub first_name: String,
    pub last_name: Option<String>,
    pub language_code: Option<String>, // Telegram client language
    pub subscription_status: SubscriptionStatus,
    pub premium_features_access: bool,
}

impl TelegramProfile {
    /// Create new Telegram profile
    pub fn new(
        user_id: TelegramUserId,
        first_name: String,
        subscription_status: SubscriptionStatus,
    ) -> Self {
        let premium_features_access = subscription_status.has_premium_access();

        Self {
            user_id,
            username: None,
            first_name,
            last_name: None,
            language_code: None,
            subscription_status,
            premium_features_access,
        }
    }

    /// Update subscription status and recalculate access
    pub fn update_subscription(&mut self, status: SubscriptionStatus) {
        self.subscription_status = status;
        self.premium_features_access = status.has_premium_access();
    }

    /// Get display name for UI
    pub fn display_name(&self) -> String {
        if let Some(ref username) = self.username {
            format!("@{}", username)
        } else if let Some(ref last_name) = self.last_name {
            format!("{} {}", self.first_name, last_name)
        } else {
            self.first_name.clone()
        }
    }

    /// Get preferred language for i18n
    pub fn preferred_language(&self) -> &str {
        match self.language_code.as_deref() {
            Some(code) => code,
            None => "en",
        }
    }
}

/// Telegram authentication session with linking state
///
/// Tracks the complete authentication flow from web deep linking
/// through Telegram Bot verification to session establishment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAuthSession {
    pub linking_token: LinkingToken,
    pub telegram_user_id: Option<TelegramUserId>,
    pub verification_completed: bool,
    pub session_established: bool,
    pub created_at: time::OffsetDateTime,
    pub expires_at: time::OffsetDateTime,
}

impl TelegramAuthSession {
    /// Create new authentication session
    pub fn new(linking_token: LinkingToken, expiry_minutes: i64) -> Self {
        let now = time::OffsetDateTime::now_utc();
        let expires_at = now + time::Duration::minutes(expiry_minutes);

        Self {
            linking_token,
            telegram_user_id: None,
            verification_completed: false,
            session_established: false,
            created_at: now,
            expires_at,
        }
    }

    /// Complete Telegram verification step
    pub fn complete_verification(&mut self, user_id: TelegramUserId) -> Result<(), DomainError> {
        if self.is_expired() {
            return Err(DomainError::AuthSessionExpired);
        }

        self.telegram_user_id = Some(user_id);
        self.verification_completed = true;
        Ok(())
    }

    /// Establish authenticated session
    pub fn establish_session(&mut self) -> Result<(), DomainError> {
        if !self.verification_completed {
            return Err(DomainError::AuthVerificationNotCompleted);
        }

        if self.is_expired() {
            return Err(DomainError::AuthSessionExpired);
        }

        self.session_established = true;
        Ok(())
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        time::OffsetDateTime::now_utc() > self.expires_at
    }

    /// Check if session is ready for use
    pub fn is_ready(&self) -> bool {
        self.session_established && !self.is_expired()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegram_user_id_validation() {
        assert!(TelegramUserId::new(123456789).is_ok());
        assert!(TelegramUserId::new(0).is_err());
        assert!(TelegramUserId::new(-123).is_err());
    }

    #[test]
    fn telegram_channel_id_validation() {
        assert!(TelegramChannelId::new(-1001234567890).is_ok());
        assert!(TelegramChannelId::new(123).is_err());
        assert!(TelegramChannelId::new(0).is_err());
    }

    #[test]
    fn subscription_status_access() {
        assert!(SubscriptionStatus::Active.has_premium_access());
        assert!(!SubscriptionStatus::NotSubscribed.has_premium_access());
        assert!(!SubscriptionStatus::Expired.has_premium_access());
        assert!(!SubscriptionStatus::VerificationFailed.has_premium_access());
    }

    #[test]
    fn auth_session_flow() {
        let token = LinkingToken::generate();
        let mut session = TelegramAuthSession::new(token, 15);

        assert!(!session.is_ready());
        assert!(!session.is_expired());

        let user_id = match TelegramUserId::new(123456789) {
            Ok(v) => v,
            Err(e) => {
                assert!(false, "test user ID should be valid: {e}");
                return;
            }
        };

        match session.complete_verification(user_id) {
            Ok(()) => {}
            Err(e) => assert!(false, "test verification should succeed: {e}"),
        }

        match session.establish_session() {
            Ok(()) => {}
            Err(e) => assert!(false, "test session establishment should succeed: {e}"),
        }

        assert!(session.is_ready());
    }
}
