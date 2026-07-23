use std::sync::Arc;

use axum::{body::Body, extract::State};
use http::header;

use crate::metrics::IsAliveState;

pub async fn is_alive_middleware(
    State(state): State<Arc<IsAliveState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if *request.uri() == "/api/isalive" {
        let json = serde_json::to_string(state.as_ref()).unwrap();
        return axum::response::Response::builder()
            .status(200)
            .body(Body::from(json))
            .unwrap();
    }

    if *request.uri() == "/metrics" {
        let report = prometheus::TextEncoder::new()
            .encode_to_string(&prometheus::default_registry().gather())
            .unwrap();

        return axum::response::Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(report))
            .unwrap();
    }

    

    next.run(request).await
}
