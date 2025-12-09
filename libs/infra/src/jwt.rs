//! JWT service implementation
//!
//! JSON Web Token creation and validation using RS256 algorithm.

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
// Use only domain types for Clean Architecture
use crate::InfraError;
use rand::thread_rng;
use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::EncodePublicKey;
use starscalendars_domain::*;
use uuid::Uuid;

/// JWT service implementation using RS256
pub struct JwtServiceImpl {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    algorithm: Algorithm,
}

impl JwtServiceImpl {
    /// Create new JWT service with RSA key pair
    pub fn new(private_key_pem: &[u8], public_key_pem: &[u8]) -> Result<Self, InfraError> {
        let encoding_key =
            EncodingKey::from_rsa_pem(private_key_pem).map_err(|e| InfraError::Jwt(e))?;

        let decoding_key =
            DecodingKey::from_rsa_pem(public_key_pem).map_err(|e| InfraError::Jwt(e))?;

        Ok(Self {
            encoding_key,
            decoding_key,
            algorithm: Algorithm::RS256,
        })
    }

    /// Create with generated keys for development/testing
    pub fn new_with_generated_keys() -> Result<Self, InfraError> {
        // Generate a temporary RSA key pair for testing (no embedded keys)
        // In production, keys must be loaded from secure storage
        let mut rng = rand::thread_rng();
        let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048)
            .map_err(|e| InfraError::Internal(e.to_string()))?;
        let public_key = private_key.to_public_key();

        let private_pem = private_key
            .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|e| InfraError::Internal(e.to_string()))?;
        let public_pem = public_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|e| InfraError::Internal(e.to_string()))?;

        Self::new(private_pem.as_bytes(), public_pem.as_bytes())
    }
}

#[async_trait]
impl JwtService for JwtServiceImpl {
    async fn create_access_token(&self, claims: &JwtClaims) -> PortResult<String> {
        let header = Header::new(self.algorithm);

        encode(&header, claims, &self.encoding_key).map_err(|e| InfraError::Jwt(e).into())
    }

    async fn validate_access_token(&self, token: &str) -> PortResult<JwtClaims> {
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;

        let token_data =
            decode::<JwtClaims>(token, &self.decoding_key, &validation).map_err(|e| {
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        // Get expiry time from the token if possible
                        let exp_time = time::OffsetDateTime::now_utc();
                        let infra_error = InfraError::Internal(
                            DomainError::JwtTokenExpired(exp_time).to_string(),
                        );
                        DomainError::from(infra_error)
                    }
                    _ => InfraError::Jwt(e).into(),
                }
            })?;

        Ok(token_data.claims)
    }

    async fn create_refresh_token(&self) -> PortResult<String> {
        // Create a cryptographically secure random token
        Ok(Uuid::new_v4().to_string())
    }
}

/// Mock JWT service for testing
pub struct MockJwtService {
    /// Whether tokens should be considered valid
    pub tokens_valid: bool,
    /// Mock claims to return
    pub mock_claims: Option<JwtClaims>,
}

impl MockJwtService {
    /// Create new mock JWT service
    pub fn new() -> Self {
        Self {
            tokens_valid: true,
            mock_claims: None,
        }
    }

    /// Set whether tokens should be valid
    pub fn set_tokens_valid(&mut self, valid: bool) {
        self.tokens_valid = valid;
    }

    /// Set mock claims to return
    pub fn set_mock_claims(&mut self, claims: JwtClaims) {
        self.mock_claims = Some(claims);
    }
}

impl Default for MockJwtService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JwtService for MockJwtService {
    async fn create_access_token(&self, _claims: &JwtClaims) -> PortResult<String> {
        Ok("mock_access_token".to_string())
    }

    async fn validate_access_token(&self, _token: &str) -> PortResult<JwtClaims> {
        if !self.tokens_valid {
            return Err(InfraError::Internal("Invalid token".to_string()).into());
        }

        match &self.mock_claims {
            Some(claims) => Ok(claims.clone()),
            None => {
                let user_id = starscalendars_domain::auth::UserId::new();
                Ok(JwtClaims::new(&user_id, None, false, &[]))
            }
        }
    }

    async fn create_refresh_token(&self) -> PortResult<String> {
        Ok("mock_refresh_token".to_string())
    }
}
