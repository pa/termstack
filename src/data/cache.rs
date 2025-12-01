use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Cached data entry
#[derive(Debug, Clone)]
struct CacheEntry {
    data: Value,
    timestamp: SystemTime,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.timestamp.elapsed().unwrap_or(Duration::MAX) > self.ttl
    }
}

/// Simple TTL cache for data provider results
#[derive(Debug, Clone)]
pub struct DataCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl DataCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get cached data if available and not expired
    pub async fn get(&self, key: &str) -> Option<Value> {
        let cache = self.cache.read().await;

        if let Some(entry) = cache.get(key) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            }
        }

        None
    }

    /// Store data in cache with TTL
    pub async fn set(&self, key: String, data: Value, ttl: Duration) {
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now(),
            ttl,
        };

        let mut cache = self.cache.write().await;
        cache.insert(key, entry);
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Remove expired entries
    pub async fn cleanup(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, entry| !entry.is_expired());
    }
}

impl Default for DataCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cache_set_get() {
        let cache = DataCache::new();
        let data = json!({"test": "value"});

        cache
            .set("key1".to_string(), data.clone(), Duration::from_secs(60))
            .await;

        let result = cache.get("key1").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), data);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = DataCache::new();
        let data = json!({"test": "value"});

        // Set with very short TTL
        cache
            .set("key1".to_string(), data, Duration::from_millis(10))
            .await;

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        let result = cache.get("key1").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_invalidate() {
        let cache = DataCache::new();
        let data = json!({"test": "value"});

        cache
            .set("key1".to_string(), data, Duration::from_secs(60))
            .await;
        cache.invalidate("key1").await;

        let result = cache.get("key1").await;
        assert!(result.is_none());
    }
}
