use axum::{extract::Request, middleware::Next, response::Response};
use http::HeaderValue;
use uuid::Uuid;

const TRACEPARENT: &str = "traceparent";
const TRACESTATE: &str = "tracestate";

/// W3C Trace Context propagation middleware.
/// If incoming request has traceparent, forward it.
/// If not, generate a new trace context.
pub async fn layer(mut req: Request, next: Next) -> Response {
    let traceparent = req
        .headers()
        .get(TRACEPARENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let traceparent = match traceparent {
        Some(tp) => tp,
        None => {
            let trace_id = Uuid::new_v4().as_simple().to_string();
            let span_id = &Uuid::new_v4().as_simple().to_string()[..16];
            format!("00-{}-{}-01", trace_id, span_id)
        }
    };

    if let Ok(v) = HeaderValue::from_str(&traceparent) {
        req.headers_mut().insert(TRACEPARENT, v);
    }

    let mut response = next.run(req).await;

    if let Ok(v) = HeaderValue::from_str(&traceparent) {
        response.headers_mut().insert(TRACEPARENT, v);
    }

    // Forward tracestate if present
    if let Some(ts) = response.headers().get(TRACESTATE).cloned() {
        response.headers_mut().insert(TRACESTATE, ts);
    }

    response
}
