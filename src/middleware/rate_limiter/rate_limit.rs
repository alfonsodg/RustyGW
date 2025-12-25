use std::{sync::Arc, time::Duration};

use axum::{extract::{Request, State}, middleware::Next, response::Response};
use axum_client_ip::ClientIp;
use tracing::{info, warn};

use crate::{constants::time, constants::rate_limiter as rl_constants, errors::AppError, state::AppState};


pub async fn layer(
    State(state): State<Arc<AppState>>,
    ClientIp(client_ip): ClientIp,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    
    info!(client_ip = ?client_ip, "Client connected");
    let config_guard = state.config.read().await;
    let route = config_guard
        .find_route_for_path(req.uri().path());

    if let Some(route_config) = route {
        if let Some(rate_limit_config) = route_config.rate_limit.as_ref() {
            let period = parse_duration(&rate_limit_config.period)
                .unwrap_or_else(|_| Duration::from_secs(rl_constants::DEFAULT_PERIOD_SECONDS));
            let capacity = rate_limit_config.requests;
            let refill_rate = rate_limit_config.requests as f64 / period.as_secs_f64();

        // clinets Ip address as key to rate limiting
        let key = client_ip.to_string();
        let allowed = state.rate_limit_store
            .check_and_update(&key, capacity, refill_rate)
            .await;
        
        if !allowed {
            warn!(ip=%key, path=%req.uri().path(),"Request rate-limited");
            return Err(AppError::RateLimited);
        }
      }
    }
    Ok(next.run(req).await)
}

pub fn parse_duration(s: &str) -> Result<Duration, &'static str> {
    let s = s.trim();
    let unit = s.chars().last().ok_or("Empty durtion")?;
    let value: u64 = s[..s.len()-1]
        .parse()
        .map_err(|_| "Invalid number in duration")?;

    match  unit {
        's' => Ok(Duration::from_secs(value)),
        'm' => Ok(Duration::from_secs(value * time::SECONDS_PER_MINUTE)),
        'h' => Ok(Duration::from_secs(value * time::SECONDS_PER_HOUR)),
        _ => Err("Invalid duration unit")
    }
}