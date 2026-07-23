use std::future::Future;

use redis::AsyncCommands;
use redis::aio::ConnectionManager;

/// High-level Redis cache with automatic JSON serialization.
///
/// Obtain via [`ServiceContext::get_redis_cache()`].
///
/// # Example
/// ```rust
/// let cache = ctx.get_redis_cache();
///
/// // Set
/// cache.set("order:1", &order, 3600).await;
///
/// // Get
/// let order: Option<Order> = cache.get("order:1").await;
///
/// // Get or compute and cache
/// let order = cache.get_or_set("order:1", 3600, || async {
///     db.load_order("1").await
/// }).await;
/// ```
#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
}

impl RedisCache {
    pub(crate) fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }

    /// Returns the raw connection for non-cache operations (pub/sub, rate limiting, etc.).
    pub fn connection(&self) -> ConnectionManager {
        self.conn.clone()
    }

    /// Deserialize a cached value. Returns `None` if key is missing.
    /// Logs a warning if the value exists but deserialization fails (e.g. stale/mismatched type).
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.conn.clone();
        let value: Option<String> = conn.get(key).await.ok()?;
        let raw = value?;
        match serde_json::from_str(&raw) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(key, error = %e, "Cache deserialization failed, treating as cache miss");
                None
            }
        }
    }

    /// Serialize and store a value with a TTL (in seconds).
    /// Logs an error on Redis failure — does not panic, since a cache write failure
    /// should not crash the service.
    pub async fn set<T: serde::Serialize>(&self, key: &str, value: &T, ttl_secs: u64) {
        let serialized = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(key, error = %e, "Failed to serialize cache value");
                return;
            }
        };
        let mut conn = self.conn.clone();
        if let Err(e) = conn.set_ex::<_, _, ()>(key, serialized, ttl_secs).await {
            tracing::error!(key, error = %e, "Redis set_ex failed");
        }
    }

    /// Delete a key. Logs an error on failure.
    pub async fn delete(&self, key: &str) {
        let mut conn = self.conn.clone();
        if let Err(e) = conn.del::<_, ()>(key).await {
            tracing::error!(key, error = %e, "Redis del failed");
        }
    }

    /// Returns cached value if present, otherwise calls `compute`, caches the result, and returns it.
    pub async fn get_or_set<T, F, Fut>(&self, key: &str, ttl_secs: u64, compute: F) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        if let Some(cached) = self.get::<T>(key).await {
            return cached;
        }
        let value = compute().await;
        self.set(key, &value, ttl_secs).await;
        value
    }
}
