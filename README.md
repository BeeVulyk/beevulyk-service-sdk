# BeeVulyk Service SDK

## Origin

This crate is the BeeVulyk fork of [`ITYFT/yft-service-sdk`](https://github.com/ITYFT/yft-service-sdk) at tag `0.1.16`. The original was a Cargo workspace of three crates (`yft-service-sdk`, `yft-service-sdk-macros`, `yft-tonic-prometheus`). In this fork, `beevulyk-service-sdk` is a **standalone crate** — the sibling crates have been extracted into their own repos and are consumed here as git dependencies:

- [`beevulyk-rust-extensions`](https://github.com/BeeVulyk/beevulyk-rust-extensions) `0.1.0`
- [`beevulyk-tonic-prometheus`](https://github.com/BeeVulyk/beevulyk-tonic-prometheus) `0.1.0`
- [`beevulyk-service-sdk-macros`](https://github.com/BeeVulyk/beevulyk-service-sdk-macros) `0.1.0`
- [`beevulyk-grpc-extensions`](https://github.com/BeeVulyk/beevulyk-grpc-extensions) `0.1.0`

Versioning starts fresh at `0.1.0` for the BeeVulyk lineage — the upstream version history is not inherited.

## Overview

Runtime building blocks for BeeVulyk services with an Axum HTTP surface, optional gRPC endpoints, Prometheus metrics, structured JSON logs, and a handful of proc macros to wire settings and integrations consistently.

## What it provides
- Service bootstrap via `ServiceContext` (logging setup, panic hook, background timers, HTTP server, optional gRPC server).
- Health and metrics endpoints out of the box: `/api/isalive` returns service metadata and `/metrics` exposes Prometheus metrics.
- HTTP metrics middleware that records per-route counters, gauges, and latency histograms.
- gRPC server builder that layers structured tracing middleware and tonic Prometheus metrics (with configurable buckets and registry).
- Structured JSON logging through a `tracing` layer, including contextual fields like target, module path, location, and timestamps.
- Optional integrations behind feature flags: PostgreSQL/SQLite pools (`sqlx`), Redis cache/connection manager, Kafka publisher/subscriber, WebSocket/GraphQL helpers, and gRPC client/server helpers.

## Quick start

Add the SDK to your service (enable the features you need):

```toml
[dependencies]
beevulyk-service-sdk = { git = "https://github.com/BeeVulyk/beevulyk-service-sdk.git", tag = "0.1.0", features = ["grpc", "postgresql"] }
```

Create a settings reader that implements `ServiceInfo` (the derive macro assumes the type is named `SettingsReader`):

```rust
use std::sync::Arc;
use axum::{routing::get, Router};
use beevulyk_service_sdk::{ServiceContext, ServiceInfo};
use beevulyk_service_sdk::macros::SdkSettingsTraits;

#[derive(Clone, SdkSettingsTraits)]
struct SettingsReader;

#[tokio::main]
async fn main() {
    let settings = Arc::new(SettingsReader);

    // Build your HTTP API
    let router = Router::new().route("/api/ping", get(|| async { "pong" }));

    // Start the service (default HTTP port 8000; gRPC optional)
    let mut ctx = ServiceContext::new(settings.clone()).await.http_port(8080);
    ctx.init_http_router(router);
    ctx.start_application().await;
}
```

What you get by default:
- `/api/isalive` with service name/version, optional `ENV_INFO`, and start timestamp.
- `/metrics` with Prometheus output that includes HTTP counters (`total_http_requests_counter`), gauges (`total_http_pending_requests_gauge`), and latency histogram (`http_requests_sec_duration_historgram`), plus any gRPC metrics if enabled.
- JSON logs printed to stdout with consistent structure and timestamps.

## REST service example

Realistic Axum setup that wires routes, shared state, and launches the service:

```rust
use std::sync::Arc;
use axum::routing::post;
use beevulyk_service_sdk::external::axum::Router;

#[tokio::main]
async fn main() {
    let settings_reader = SettingsReader::new(".my-service-name").await;
    let settings_reader = Arc::new(settings_reader);

    let mut service_context = beevulyk_service_sdk::ServiceContext::new(settings_reader.clone()).await;
    let app_context = Arc::new(AppContext::new(&service_context, settings_reader.clone()).await);

    service_context.init_http_router(
        Router::new()
            .route("/api/trading/active", post(update_active))
            .route("/api/trading/active/close", post(close_active_position))
            .route("/api/trading/order/place", post(place_order))
            .route("/api/trading/order", post(update_pending))
            .route("/api/trading/order/cancel", post(cancel_order))
            .with_state(app_context.clone()),
    );

    service_context.start_application().await;
}
```

## gRPC server example

Enable the `grpc` feature and add your tonic services via the builder. The SDK wires `beevulyk-tonic-prometheus` and tracing middleware automatically.

```rust
use std::sync::Arc;
use beevulyk_service_sdk::{GrpcServerBuilder, ServiceContext};
use my_proto::my_service_server::MyServiceServer;

let mut ctx = ServiceContext::new(settings.clone()).await;
ctx.configure_grpc_server(|builder: &mut GrpcServerBuilder| {
    builder.add_grpc_service(MyServiceServer::new(app_logic.clone()));
    // builder.update_listen_endpoint("0.0.0.0".parse().unwrap(), 8890); // optional override (defaults to 8888 or $GRPC_PORT)
});
ctx.start_application().await;
```

### gRPC metrics

- Server metrics: `grpc_server_started_total`, `grpc_server_handled_total{grpc_code=...}`, and `grpc_server_handling_seconds` with labels for service/method and status code. Legacy path-based counters (`function_calls_total`, etc.) are also emitted.
- Client-side metrics are exposed by `beevulyk-tonic-prometheus` (`grpc_client_started_total`, `grpc_client_handled_total`, `grpc_client_handling_seconds`) if you apply the `MetricsLayer` on clients.
- To customize buckets or share a registry, call `beevulyk_tonic_prometheus::metrics::try_init_settings(...)` before building the server.

## Feature flags

| Feature | What it enables |
| --- | --- |
| `full` | Enables `ws`, `ql`, `postgresql`, `redis`, `kafka`, `grpc` at once |
| `grpc` | gRPC server builder and `beevulyk-grpc-extensions` helpers |
| `grpc_metrics_disabled` | Turns off Prometheus export in `beevulyk-tonic-prometheus` |
| `ws` | Axum WebSocket support |
| `ql` | GraphQL helpers via `async-graphql` and `async-graphql-axum` |
| `postgresql` / `sqlite` | `sqlx` pool helpers (`ServiceContext::get_db_pool` / `get_sqlite_pool`) |
| `redis` | Redis `ConnectionManager` and `RedisCache` helpers |
| `kafka` | Kafka `FutureProducer` publisher and consumer subscriber loop |
| `macros` | Convenience imports for gRPC integrations |

## Notes

- `ServiceContext` installs a panic hook, sets up `tracing` with JSON logs, and starts any registered background timers.
- Health/metrics middleware is attached automatically when the HTTP router is initialized; paths starting with `/ws` and `/metrics` are skipped by the HTTP metrics middleware.
- The derive macros in `beevulyk-service-sdk-macros` target a type named `SettingsReader`; if you prefer a different name, implement `ServiceInfo` (and any required integration traits) manually.
