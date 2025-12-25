use std::{sync::Arc, time::Instant};

use axum::{extract::{Request, State}, middleware::Next, response::Response};

use crate::{errors::AppError, features::circuit_breaker::circuit_breaker::{State as CircuitStateEnum}, middleware::rate_limiter::rate_limit::parse_duration, state::AppState, utils::logging::log_circuit_breaker_event};


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

    // Single lock scope to prevent race conditions
    let mut current_state = circuit.state.write().await;
    
    // Check circuit breaker state before processing request
    let should_process_request = match *current_state {
        CircuitStateEnum::Open { opened_at } => {
            let open_duration = parse_duration(&cb_config.open_duration).unwrap_or_default();

            if opened_at.elapsed() > open_duration {
                *current_state = CircuitStateEnum::HalfOpen { consecutive_successes: 0 };
                log_circuit_breaker_event(&route.name, "open", "half_open", "timeout_elapsed");
                true
            } else {
                log_circuit_breaker_event(&route.name, "open", "open", "rejecting_request");
                false
            }
        },
        CircuitStateEnum::HalfOpen { consecutive_successes: _ } => {
            true
        },
        CircuitStateEnum::Closed { consecutive_failures: _ } => {
            true
        }
    };

    if !should_process_request {
        return Err(AppError::ServiceUnavailable);
    }

    // Process request while holding the lock
    let response = next.run(req).await;

    // Update state based on response while still holding the lock
    if response.status().is_server_error() {
        // Request Failed
        match *current_state {
            CircuitStateEnum::HalfOpen { .. } | CircuitStateEnum::Closed { .. } => {
                // If a trial fails OR a normal request fails, we check the failure threshold.
                let failures = match *current_state {
                    CircuitStateEnum::Closed { consecutive_failures } => consecutive_failures + 1,
                    _ => 1, // First failure in HalfOpen state
                };

                if failures >= cb_config.failure_threshold {
                    *current_state = CircuitStateEnum::Open { opened_at: Instant::now() };
                    log_circuit_breaker_event(&route.name, "closed", "open", "failure_threshold_reached");
                } else {
                    *current_state = CircuitStateEnum::Closed { consecutive_failures: failures };
                }
            }
            _ => {}
        }
    } else {
        // Request Succeeded
        match *current_state {
            CircuitStateEnum::HalfOpen { consecutive_successes } => {
                let new_successes = consecutive_successes + 1;
                if new_successes >= cb_config.success_threshold {
                    *current_state = CircuitStateEnum::Closed { consecutive_failures: 0 };
                    log_circuit_breaker_event(&route.name, "half_open", "closed", "success_threshold_reached");
                } else {
                    *current_state = CircuitStateEnum::HalfOpen { consecutive_successes: new_successes };
                }
            }
            CircuitStateEnum::Closed { consecutive_failures } => {
                if consecutive_failures > 0 {
                    // Reset failure count on success.
                    *current_state = CircuitStateEnum::Closed { consecutive_failures: 0 };
                }
            }
            _ => {}
        }
    }

    Ok(response)

}
