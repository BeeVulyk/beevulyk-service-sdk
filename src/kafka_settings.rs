#[async_trait::async_trait]
pub trait KafkaSettings {
    async fn get_kafka_brokers(&self) -> Vec<String>;
}
