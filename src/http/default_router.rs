use axum::Router;
// use axum_prometheus::PrometheusMetricLayer;

pub async fn wrap_router(router: Router) -> axum::Router {
    // let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

    // let router = router
    //     .route(
    //         "/metrics",
    //         axum::routing::get(|| async move {
    //             format!(
    //                 "{}{}",
    //                 tonic_prometheus_layer::metrics::encode_to_string().unwrap_or_default(),
    //                 metric_handle.render()
    //             )
    //         }),
    //     )
    //     .layer(prometheus_layer);

    return router;
}
