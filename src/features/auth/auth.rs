use std::collections::HashSet;

use http::HeaderMap;
use jsonwebtoken::{decode, errors::ErrorKind, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{config::{ApiKeyStore, AuthType, SecretsConfig}, errors::AppError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Claims {
    pub sub: String,  // Subject (Uer Id)
    pub roles: Vec<String>,
    pub exp: usize,  // Required for JWT validation
}

pub fn verify_token(
    headers: &HeaderMap,
    auth_config: &crate::config::AuthConfig,
    secrets: &SecretsConfig,
    key_store: &ApiKeyStore,
) -> Result<Claims, AppError> {
    
    let token = extract_bearer_token(headers)?;

    match auth_config.auth_type {
        AuthType::Jwt => verify_jwt(token,secrets),
        AuthType::ApiKey => verify_api_key(token,key_store),
    }

}

fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, AppError> {
    let auth_header = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::MissingAuthToken)?;

    auth_header.strip_prefix("Bearer ")
        .ok_or(AppError::InvalidAuthHeader)
}

pub fn check_roles(user_roles: &[String], required_roles: &[String]) -> Result<(), AppError> {
    let user_roles_set : HashSet<_> = user_roles.iter().collect();
    for required_role in required_roles {
        if !user_roles_set.contains(required_role) {
            return Err(AppError::InsufficientPermissions);
        }
    }
    Ok(())
}

// ------- Private Helper Functions  -----

fn verify_jwt(token: &str, secrets: &SecretsConfig) -> Result<Claims, AppError> {
    info!(token = "***"); // Mask token to prevent exposure in logs
    let key = DecodingKey::from_secret(secrets.jwt_secret.as_ref());
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    decode::<Claims>(token, &key, &validation)
    .map_err(|error| match error.kind() {
        ErrorKind::ExpiredSignature => AppError::TokenExpired,
        _ => AppError::AuthFailed("Invalid JWT.".to_string()),
    })
    .map(|token_data| token_data.claims)
    
}

fn verify_api_key(token: &str, key_store: &ApiKeyStore) -> Result<Claims,AppError>  {
    let details = key_store
        .keys
        .get(token)
        .ok_or_else(|| AppError::AuthFailed("Invalid API Key.".to_string()))?;

    if details.status != "active" {
        return Err(AppError::AuthFailed("API Key is revoked.".to_string()));
    }

    Ok(Claims {
        sub: details.user_id.clone(),
        roles: details.roles.clone(),
        exp: 0, // Not applicable for API keys
    })
}