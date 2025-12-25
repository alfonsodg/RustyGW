use std::{sync::Arc, time::Instant};

use axum::{extract::{Request, State}, middleware::Next, response::Response};
use tracing::{info, warn};

use crate::{errors::AppError, features::circuit_breaker::circuit_breaker::{State as CircuitStateEnum}, middleware::rate_limiter::rate_limit::parse_duration, state::AppState};


pub async fn layer(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {

    let config_guard = state.config.read().await;
    let route = match config_guard.find_route_for_path(req.uri().path()) {
        Some(r) => r,
        None => return Ok(next.run(req).await),
    };

    let cb_config = match &route.circuit_breaker {
        Some(c) => c,
        None => return Ok(next.run(req).await),
    };

    let circuit = state.circuit_breaker_store.get_or_insert(&route.name);

    let mut current_state = circuit.state.write().await;

    match *current_state {
        CircuitStateEnum::Open { opened_at } => {
            let open_duration = parse_duration(&cb_config.open_duration).unwrap_or_default();

            if opened_at.elapsed() > open_duration {
                *current_state = CircuitStateEnum::HalfOpen { consecutive_successes: 0 };
                info!(route = %route.name, "Circuit breaker is now HALF-OPEN");
            }else{
                warn!(route = %route.name, "Circuit breaker is OPEN, rejecting request");
                return Err(AppError::ServiceUnavailable);
            }
        },
        CircuitStateEnum::HalfOpen { consecutive_successes: _ } => {
            info!(route = %route.name, "Circuit breaker is HALF-OPEN, allowing trial requests");
        },
        CircuitStateEnum::Closed { consecutive_failures: _ } => {}
    }
    drop(current_state);

    let response = next.run(req).await;

    let mut final_state = circuit.state.write().await;

    if response.status().is_server_error() {
        // Request Failed
        match *final_state {
            CircuitStateEnum::HalfOpen { .. } | CircuitStateEnum::Closed { .. } => {
                // If a trial fails OR a normal request fails, we check the failure threshold.
                let failures = match *final_state {
                    CircuitStateEnum::Closed { consecutive_failures } => consecutive_failures + 1,
                    _ => 1, // First failure in HalfOpen state
                };

                if failures >= cb_config.failure_threshold {
                    *final_state = CircuitStateEnum::Open { opened_at: Instant::now() };
                    warn!(route = %route.name, "Failure threshold reached, circuit is OPENED");
                } else {
                    *final_state = CircuitStateEnum::Closed { consecutive_failures: failures };
                }
            }
            _ => {}
        }
    }else{
        // Request Succeded
        match *final_state {
            CircuitStateEnum::HalfOpen { consecutive_successes } => {
                let new_successes = consecutive_successes + 1;
                if new_successes >= cb_config.success_threshold {
                    // Success threshold reached, close the circuit.
                    *final_state = CircuitStateEnum::Closed { consecutive_failures: 0 };
                    info!(route = %route.name, "Success threshold reached, circuit is now CLOSED");
                } else {
                    // Increment success count but remain Half-Open.
                    *final_state = CircuitStateEnum::HalfOpen { consecutive_successes: new_successes };
                    info!(route = %route.name, successes = new_successes, "Trial request succeeded, remaining HALF-OPEN");
                }
            }
            CircuitStateEnum::Closed { consecutive_failures } => {
                if consecutive_failures > 0 {
                    // Reset failure count on success.
                    *final_state = CircuitStateEnum::Closed { consecutive_failures: 0 };
                }
            }
            _ => {}
        }
    }

    Ok(response)

}
