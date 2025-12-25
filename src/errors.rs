use axum::{http::StatusCode, response::{IntoResponse, Response}};
use reqwest::Error;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    RateLimited,
    ServiceUnavailable,

    // Auth errors
    AuthFailed(String),
    MissingAuthToken,
    InvalidAuthHeader,
    InsufficientPermissions,
    TokenExpired,

    // Proxy errors
    RouteNotFound,
    ProxyError(Error),
    InvalidDestination(String),
    InternalServerError,
    
    // Hot reload errors
    HotReloadError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests".to_string(),
            ),
            AppError::AuthFailed(reason) => (StatusCode::UNAUTHORIZED, format!("Authentication failed: {}", reason)),
            AppError::MissingAuthToken => (StatusCode::UNAUTHORIZED, "Missing 'Authorization' header".to_string()),
            AppError::InvalidAuthHeader => (StatusCode::UNAUTHORIZED, "Invalid 'Authorization' header format. Expected 'Bearer <token>'.".to_string()),
            AppError::InsufficientPermissions => (StatusCode::FORBIDDEN, "You do not have permission to access this resource.".to_string()),
            AppError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token has expired".to_string()), 
            AppError::RouteNotFound => (StatusCode::NOT_FOUND, "Route not found".to_string()),
            AppError::ProxyError(e) => {
                tracing::error!("Proxy error: {}", e);
                (StatusCode::BAD_GATEWAY, "Error proxying request".to_string())
            }
            AppError::InvalidDestination(url) => {
                tracing::error!("Invalid destination URL configured: {}", url);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Invalid gateway configuration".to_string(),
                )
            }
            AppError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal server error occurred".to_string(),
            ),
            AppError::ServiceUnavailable => {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service Unavailable".to_string()
                )
            }
            AppError::HotReloadError(msg) => {
                tracing::error!("Hot reload error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Configuration reload failed".to_string(),
                )
            }
        };

        (status, error_message).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        AppError::ProxyError(error)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::RateLimited => write!(f, "Rate limited"),
            AppError::ServiceUnavailable => write!(f, "Service unavailable"),
            AppError::AuthFailed(reason) => write!(f, "Authentication failed: {}", reason),
            AppError::MissingAuthToken => write!(f, "Missing authorization token"),
            AppError::InvalidAuthHeader => write!(f, "Invalid authorization header"),
            AppError::InsufficientPermissions => write!(f, "Insufficient permissions"),
            AppError::TokenExpired => write!(f, "Token expired"),
            AppError::RouteNotFound => write!(f, "Route not found"),
            AppError::ProxyError(_) => write!(f, "Proxy error"),
            AppError::InvalidDestination(url) => write!(f, "Invalid destination: {}", url),
            AppError::InternalServerError => write!(f, "Internal server error"),
            AppError::HotReloadError(msg) => write!(f, "Hot reload error: {}", msg),
        }
    }
}