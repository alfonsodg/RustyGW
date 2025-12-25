use std::{sync::Arc};

use axum::{extract::{Request, State}, middleware::Next, response::Response};

use http::Uri;

use crate::{config::RouteConfig, errors::AppError, features::auth::auth::{check_roles, verify_token}, state::AppState};

// axum middleware layer for authentication
pub async fn layer (
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next
) -> Result<Response, AppError> 
{
    let route = find_route_for_uri(req.uri(), state.clone()).await?;

    if let Some(auth_config) = &route.auth {
        let claims = {
            // Acquire read lock on the key store for API key checks
            let key_store_guard = state.key_store.read().await;
            // Pass all necessary configs to the verification function
            verify_token(req.headers(), auth_config, &state.secrets, &key_store_guard)?
        };

        if let Some(required_roles) = &auth_config.roles {
            check_roles(&claims.roles, required_roles)?;
        }

        req.extensions_mut().insert(claims);
    }

    Ok(next.run(req).await)
}

async fn find_route_for_uri(uri: &Uri, state: Arc<AppState>) -> Result<Arc<RouteConfig>,AppError> {

    let config_guard = state.config.read().await;

    config_guard
        .find_route_for_path(uri.path())
        .ok_or(AppError::RouteNotFound)
    
}