use std::{sync::Arc, time::Duration};

use axum::{extract::{Request, State}, middleware::Next, response::Response};
use axum_client_ip::ClientIp;

use crate::{constants::rate_limiter as rl_constants, errors::AppError, middleware::get_route_config, state::AppState, utils::{logging::log_rate_limit_event, parse_duration}};


pub async fn layer(
    State(state): State<Arc<AppState>>,
    ClientIp(client_ip): ClientIp,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    
    let route = get_route_config(&state, req.uri().path()).await;

    if let Some(route_config) = route {
        if let Some(rate_limit_config) = route_config.rate_limit.as_ref() {
            let period = parse_duration(&rate_limit_config.period)
                .unwrap_or_else(|_| Duration::from_secs(rl_constants::DEFAULT_PERIOD_SECONDS));
            let capacity = rate_limit_config.requests;
            let refill_rate = rate_limit_config.requests as f64 / period.as_secs_f64();

        let key = client_ip.to_string();
        let allowed = state.rate_limit_store
            .check_and_update(&key, capacity, refill_rate)
            .await;
        
        if !allowed {
            log_rate_limit_event(&key, req.uri().path(), true, capacity);
            return Err(AppError::RateLimited);
        }
      }
    }
    Ok(next.run(req).await)
}