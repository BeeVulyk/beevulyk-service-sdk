use std::{panic::PanicHookInfo, sync::Arc};

#[cfg(feature = "grpc")]
use crate::{GrpcServer, GrpcServerBuilder};
use axum::middleware::{from_fn, from_fn_with_state};
use chrono::Utc;
#[cfg(feature = "redis")]
use crate::RedisSettings;
#[cfg(feature = "kafka")]
use crate::KafkaSettings;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt};
use beevulyk_rust_extensions::AppStates;

use crate::{
    ServiceInfo, http_metrics_middleware::prometheus_metrics_middleware,
    isalive_middleware::is_alive_middleware, logs::JsonLogLayer, metrics::IsAliveState,
    wrap_router,
};

pub struct ServiceContext {
    pub http_port: u32,
    pub app_states: Arc<beevulyk_rust_extensions::AppStates>,
    pub background_timers: Vec<beevulyk_rust_extensions::MyTimer>,
    pub app_name: String,
    pub app_version: String,
    pub http_router: Option<axum::Router>,
    pub is_debug_mode: bool,
    #[cfg(feature = "grpc")]
    pub grpc_server_builder: Option<GrpcServerBuilder>,
    #[cfg(feature = "grpc")]
    pub grpc_server: Option<GrpcServer>,
    #[cfg(feature = "redis")]
    pub redis_connection: Arc<redis::aio::ConnectionManager>,
    #[cfg(feature = "kafka")]
    kafka_producer: rdkafka::producer::FutureProducer,
    #[cfg(feature = "kafka")]
    kafka_brokers: String,
}

impl ServiceContext {
    pub async fn new(
        settings_reader: beevulyk_service_sdk_macros::generate_settings_signature!(),
    ) -> Self {
        let is_debug_mode = cfg!(debug_assertions) || std::env::var("DEBUG").is_ok();

        let default_env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            if is_debug_mode {
                EnvFilter::new("trace,hyper=off,h2=off,tower=off,tonic=off,axum=off")
            } else {
                EnvFilter::new("info")
            }
        });

        let app_states = Arc::new(AppStates::create_un_initialized());
        let app_name = settings_reader.get_service_name().to_string();
        let app_version = settings_reader.get_service_version().to_string();

        let json_log_layer = JsonLogLayer::new(app_name.clone(), app_version.clone());

        let pretty_console_layer = fmt::layer()
            .pretty()
            .with_target(true)
            .with_file(true)
            .with_line_number(true);

        if is_debug_mode {
            tracing::subscriber::set_global_default(
                Registry::default()
                    .with(default_env_filter)
                    .with(pretty_console_layer),
            )
            .expect("Failed to set subscriber");
        } else {
            tracing::subscriber::set_global_default(
                Registry::default()
                    .with(default_env_filter)
                    .with(json_log_layer),
            )
            .expect("Failed to set subscriber");
        }

        #[cfg(feature = "redis")]
        let redis_connection = {
            let url = settings_reader.get_redis_url().await;
            let client = redis::Client::open(url).expect("Failed to create Redis client");
            let manager = redis::aio::ConnectionManager::new(client)
                .await
                .expect("Failed to connect to Redis");
            Arc::new(manager)
        };

        #[cfg(feature = "kafka")]
        let (kafka_producer, kafka_brokers) = {
            use rdkafka::config::ClientConfig;
            use rdkafka::producer::FutureProducer;

            let brokers = settings_reader.get_kafka_brokers().await;
            let brokers_str = brokers.join(",");

            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", &brokers_str)
                .create()
                .expect("Failed to create Kafka producer");

            (producer, brokers_str)
        };

        Self {
            app_states,
            app_name: app_name.to_string(),
            app_version: app_version.to_string(),
            background_timers: vec![],
            http_router: None,
            http_port: 8000,
            is_debug_mode,
            #[cfg(feature = "grpc")]
            grpc_server_builder: None,
            #[cfg(feature = "grpc")]
            grpc_server: None,
            #[cfg(feature = "redis")]
            redis_connection,
            #[cfg(feature = "kafka")]
            kafka_producer,
            #[cfg(feature = "kafka")]
            kafka_brokers,
        }
    }

    pub fn http_port(self, port: u32) -> Self {
        let mut ctx = self;
        ctx.http_port = port;
        ctx
    }

    pub async fn start_application(&mut self) {
        std::panic::set_hook(Box::new(|itm| {
            let location = itm.location().map(|x| format!("{}:{}", x.file(), x.line()));

            let message = format!(
                "Handled panic in global catch. Payload: {:?}",
                extract_panic_payload(itm)
            );
            tracing::error!(?location, message);
        }));
        self.app_states.set_initialized();
        for timer in self.background_timers.iter() {
            timer.start(self.app_states.clone());
        }

        let http_port = format!("0.0.0.0:{}", self.http_port);
        let addr = TcpListener::bind(http_port.clone()).await.unwrap();
        let router = self.http_router.clone();

        let router = wrap_router(router.unwrap_or_default()).await;

        let is_alive_state = Arc::new(IsAliveState {
            name: self.app_name.clone(),
            version: self.app_version.clone(),
            env_info: std::env::var("ENV_INFO").ok(),
            started: Utc::now().timestamp(),
        });

        let router = router
            .layer(from_fn_with_state(is_alive_state, is_alive_middleware))
            .layer(from_fn(prometheus_metrics_middleware));

        tokio::spawn(async move {
            tracing::info!(%http_port, "Axum started");
            axum::serve(
                addr,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
            .expect("Axum port already in use");
        });

        #[cfg(feature = "grpc")]
        if let Some(mut grpc_server_builder) = self.grpc_server_builder.take() {
            self.grpc_server = Some(grpc_server_builder.build());
        }

        tracing::info!("App started");
        self.app_states.wait_until_shutdown().await;
    }

    pub fn register_timer(
        &mut self,
        duration: std::time::Duration,
        builder: impl Fn(&mut beevulyk_rust_extensions::MyTimer),
    ) {
        let mut timer = beevulyk_rust_extensions::MyTimer::new(duration);
        builder(&mut timer);

        self.background_timers.push(timer);
    }

    pub fn init_http_router(&mut self, router: axum::Router) {
        self.http_router = Some(router);
    }

    #[cfg(feature = "grpc")]
    pub fn configure_grpc_server(&mut self, config: impl Fn(&mut GrpcServerBuilder)) {
        let mut grpc_server_builder = GrpcServerBuilder::new();
        config(&mut grpc_server_builder);
        self.grpc_server_builder = Some(grpc_server_builder);
    }

    /// Returns a cloned Redis connection manager handle (cheap clone, shared pool).
    #[cfg(feature = "redis")]
    pub fn get_redis_connection(&self) -> redis::aio::ConnectionManager {
        self.redis_connection.as_ref().clone()
    }

    /// Returns a high-level cache wrapper with automatic JSON serialization.
    #[cfg(feature = "redis")]
    pub fn get_redis_cache(&self) -> crate::RedisCache {
        crate::RedisCache::new(self.redis_connection.as_ref().clone())
    }

    /// Returns a high-level publisher that handles serialization automatically.
    ///
    /// # Example
    /// ```rust
    /// let publisher = ctx.get_kafka_publisher("orders.created");
    /// publisher.publish("order-123", &order).await;
    /// ```
    #[cfg(feature = "kafka")]
    pub fn get_kafka_publisher(&self, topic: &str) -> crate::KafkaPublisher {
        crate::KafkaPublisher::new(self.kafka_producer.clone(), topic)
    }

    /// Registers a background subscriber that continuously consumes messages
    /// from the given topic and calls `handler.handle()` for each one.
    ///
    /// # Example
    /// ```rust
    /// use beevulyk_service_sdk::KafkaStartOffset;
    ///
    /// ctx.register_kafka_subscriber(
    ///     "orders.created",
    ///     "my-consumer-group",
    ///     KafkaStartOffset::Earliest,
    ///     Arc::new(OrderHandler),
    /// );
    /// ```
    #[cfg(feature = "kafka")]
    pub fn register_kafka_subscriber<T>(
        &self,
        topic: &str,
        group_id: &str,
        start_offset: crate::KafkaStartOffset,
        handler: Arc<dyn crate::KafkaMessageHandler<T>>,
    ) where
        T: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        use rdkafka::config::ClientConfig;
        use rdkafka::consumer::{Consumer, StreamConsumer};

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &self.kafka_brokers)
            .set("group.id", group_id)
            .set("auto.offset.reset", start_offset.as_config_value())
            .set("enable.auto.commit", "true")
            .create()
            .expect("Failed to create Kafka consumer");

        consumer
            .subscribe(&[topic])
            .expect("Failed to subscribe to Kafka topic");

        tokio::spawn(crate::kafka::run_subscriber_loop(consumer, handler));
    }

    #[cfg(feature = "postgresql")]
    pub async fn get_db_pool<T: crate::SqlxSettings>(
        &self,
        connection_settings: &T,
        config_callback: impl Fn(&mut sqlx::postgres::PgPoolOptions) -> sqlx::postgres::PgPoolOptions,
    ) -> sqlx::Pool<sqlx::postgres::Postgres> {
        let mut pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .min_connections(1);
        let pool = config_callback(&mut pool);

        pool.connect(&connection_settings.get_connection_string().await)
            .await
            .unwrap()
    }

    #[cfg(feature = "sqlite")]
    pub async fn get_sqlite_pool<T: crate::SqlxSettings>(
        &self,
        connection_settings: &T,
        config_callback: impl Fn(
            &mut sqlx::sqlite::SqlitePoolOptions,
        ) -> sqlx::sqlite::SqlitePoolOptions,
    ) -> sqlx::Pool<sqlx::sqlite::Sqlite> {
        let mut pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(10)
            .min_connections(1);
        let pool = config_callback(&mut pool);

        pool.connect(&connection_settings.get_connection_string().await)
            .await
            .unwrap()
    }
}

fn extract_panic_payload(info: &PanicHookInfo) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        format!("Non-string panic payload: {:?}", info.payload().type_id())
    }
}
