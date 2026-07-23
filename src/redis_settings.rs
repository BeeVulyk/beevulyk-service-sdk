#[async_trait::async_trait]
pub trait RedisSettings {
    async fn get_redis_url(&self) -> String;
}
