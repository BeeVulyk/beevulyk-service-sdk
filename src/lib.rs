#[cfg(feature = "grpc")]
mod grpc;
#[cfg(feature = "ql")]
mod gql;
mod http;
pub mod logs;
mod metrics;
mod service_context;
mod service_info;
#[cfg(any(feature = "postgresql", feature = "sqlite"))]
mod sqlx;
mod telemetry;
#[cfg(feature = "redis")]
mod redis_settings;
#[cfg(feature = "redis")]
mod redis_cache;
#[cfg(feature = "kafka")]
mod kafka_settings;
#[cfg(feature = "kafka")]
pub mod kafka;

#[cfg(feature = "grpc")]
pub use grpc::*;
#[cfg(feature = "ql")]
pub use gql::*;
pub use http::*;
pub use service_context::*;
pub use service_info::*;
pub extern crate beevulyk_service_sdk_macros as macros;
#[cfg(any(feature = "postgresql", feature = "sqlite"))]
pub use sqlx::*;
#[cfg(feature = "redis")]
pub use redis_settings::*;
#[cfg(feature = "redis")]
pub use redis_cache::RedisCache;
#[cfg(feature = "kafka")]
pub use kafka_settings::*;
#[cfg(feature = "kafka")]
pub use kafka::{KafkaPublisher, KafkaMessageHandler, KafkaStartOffset};

#[cfg(any(feature = "postgresql", feature = "sqlite"))]
pub mod db {
    pub extern crate sqlx;
}

pub mod external {
    #[cfg(feature = "ql")]
    pub extern crate async_graphql;
    #[cfg(feature = "ql")]
    pub extern crate async_graphql_axum;
    pub extern crate async_trait;
    pub extern crate axum;
    pub extern crate envy;
    #[cfg(feature = "grpc")]
    pub extern crate futures_core;
    #[cfg(feature = "grpc")]
    pub extern crate beevulyk_grpc_extensions;
    pub extern crate lazy_static;
    #[cfg(feature = "redis")]
    pub extern crate redis;
    #[cfg(feature = "kafka")]
    pub extern crate rdkafka;
    pub extern crate prometheus;
    pub extern crate serde;
    pub extern crate tracing;
    pub extern crate beevulyk_rust_extensions;
    pub extern crate tower_http;
    pub extern crate tower;
}
