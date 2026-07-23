use std::time::Instant;

use axum::extract::MatchedPath;
use lazy_static::lazy_static;
use prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, register_histogram_vec,
    register_int_counter_vec, register_int_gauge_vec,
};

lazy_static! {
    pub static ref HTTP_REQUESTS_COUNTER: IntCounterVec = register_int_counter_vec!(
        "total_http_requests_counter",
        "Total http requests",
        &["path"]
    )
    .unwrap();
    pub static ref HTTP_PENDING_REQUESTS_GAUGE: IntGaugeVec = register_int_gauge_vec!(
        "total_http_pending_requests_gauge",
        "Total http pending requests",
        &["path"]
    )
    .unwrap();
    pub static ref HTTP_REQUESTS_DURATION_HISTORGRAM: HistogramVec = register_histogram_vec!(
        HistogramOpts::new(
            "http_requests_sec_duration_historgram",
            "Http requests histogram"
        )
        .buckets(vec![
            0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
        ]),
        &["path"]
    )
    .unwrap();
}

pub async fn prometheus_metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(route) = request.extensions().get::<MatchedPath>() {
        let uri = route.as_str().to_string();

        if uri.starts_with("/ws") || uri.starts_with("/metrics") {
            let response = next.run(request).await;
            return response;
        }

        HTTP_REQUESTS_COUNTER.with_label_values(&[&uri]).inc();
        HTTP_PENDING_REQUESTS_GAUGE.with_label_values(&[&uri]).inc();
        let start = Instant::now();

        let response = next.run(request).await;

        HTTP_REQUESTS_DURATION_HISTORGRAM
            .with_label_values(&[&uri])
            .observe(start.elapsed().as_secs_f64());

        HTTP_PENDING_REQUESTS_GAUGE.with_label_values(&[&uri]).dec();
        response
    } else {
        let response = next.run(request).await;
        response
    }
}
