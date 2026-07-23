use std::sync::Arc;
use std::time::Duration;

use rdkafka::consumer::StreamConsumer;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;

/// Kafka consumer start offset.
///
/// Controls where a subscriber begins reading messages when no committed offset exists.
///
/// # Example
/// ```rust
/// use beevulyk_service_sdk::KafkaStartOffset;
///
/// let offset = KafkaStartOffset::Latest;    // start from newest
/// let offset = KafkaStartOffset::Earliest;  // start from beginning
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KafkaStartOffset {
    /// Start from the latest message.
    Latest,
    /// Start from the earliest available message.
    Earliest,
}

impl KafkaStartOffset {
    pub(crate) fn as_config_value(&self) -> &'static str {
        match self {
            KafkaStartOffset::Latest => "latest",
            KafkaStartOffset::Earliest => "earliest",
        }
    }
}

/// Implement this trait in your service to handle incoming Kafka messages.
///
/// # Example
/// ```rust
/// struct OrderHandler;
///
/// #[async_trait::async_trait]
/// impl KafkaMessageHandler<Order> for OrderHandler {
///     async fn handle(&self, message: Order) {
///         // process order...
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait KafkaMessageHandler<T: Send + Sync>: Send + Sync {
    async fn handle(&self, message: T);
}

/// High-level Kafka publisher.
/// Handles JSON serialization and record construction automatically.
///
/// # Example
/// ```rust
/// let publisher = ctx.get_kafka_publisher("orders.created");
/// publisher.publish("order-123", &order).await;
/// ```
#[derive(Clone)]
pub struct KafkaPublisher {
    producer: FutureProducer,
    topic: String,
}

impl KafkaPublisher {
    pub(crate) fn new(producer: FutureProducer, topic: impl Into<String>) -> Self {
        Self {
            producer,
            topic: topic.into(),
        }
    }

    /// Serialize `message` as JSON and produce it to Kafka with the given key.
    pub async fn publish<T: serde::Serialize>(&self, key: &str, message: &T) {
        let payload = serde_json::to_vec(message).expect("Failed to serialize Kafka message");

        self.producer
            .send(
                FutureRecord::to(&self.topic)
                    .key(key)
                    .payload(&payload),
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to produce Kafka message");
    }

    /// Serialize `message` as JSON and produce it to a specific partition.
    pub async fn publish_to_partition<T: serde::Serialize>(
        &self,
        key: &str,
        message: &T,
        partition: i32,
    ) {
        let payload = serde_json::to_vec(message).expect("Failed to serialize Kafka message");

        self.producer
            .send(
                FutureRecord::to(&self.topic)
                    .key(key)
                    .payload(&payload)
                    .partition(partition),
                Duration::from_secs(5),
            )
            .await
            .expect("Failed to produce Kafka message");
    }
}

/// Internal consumer loop spawned as a background tokio task.
pub(crate) async fn run_subscriber_loop<T>(
    consumer: StreamConsumer,
    handler: Arc<dyn KafkaMessageHandler<T>>,
) where
    T: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    loop {
        match consumer.recv().await {
            Ok(msg) => {
                if let Some(payload) = msg.payload() {
                    match serde_json::from_slice::<T>(payload) {
                        Ok(message) => {
                            handler.handle(message).await;
                        }
                        Err(e) => {
                            tracing::error!(?e, "Failed to deserialize Kafka message");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(?e, "Kafka consumer error, retrying in 1s");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
