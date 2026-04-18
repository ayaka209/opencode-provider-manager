//! File-based caching for discovery results.
//!
//! Cache directory: varies by platform
//! - Linux: ~/.cache/opencode-provider-manager/
//! - macOS: ~/Library/Caches/opencode-provider-manager/
//! - Windows: %LOCALAPPDATA%\opencode-provider-manager\cache\
//!
//! Cache TTL: configurable, default 24 hours.

use crate::error::{DiscoveryError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default cache TTL in seconds (24 hours).
const DEFAULT_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Cache entry with metadata.
/// Uses `serde_json::Value` for serialization to avoid generic bounds issues.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Cached data as JSON value.
    pub data: serde_json::Value,
    /// Timestamp when this entry was created (UNIX epoch seconds).
    pub created_at: u64,
    /// TTL in seconds.
    pub ttl_secs: u64,
}

impl CacheEntry {
    /// Create a new cache entry from a serializable value.
    pub fn new<T: Serialize>(data: T, ttl_secs: u64) -> Result<Self> {
        let data = serde_json::to_value(&data)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to serialize cache data: {e}")))?;
        Ok(Self {
            data,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ttl_secs,
        })
    }

    /// Check if this cache entry has expired.
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.created_at + self.ttl_secs
    }
}

/// File-based cache manager.
pub struct CacheManager {
    cache_dir: PathBuf,
    default_ttl: Duration,
}

impl CacheManager {
    /// Create a new cache manager with the default cache directory.
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| DiscoveryError::Cache("Cannot determine cache directory".to_string()))?
            .join("opencode-provider-manager");

        // Ensure cache directory exists
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to create cache dir: {e}")))?;

        Ok(Self {
            cache_dir,
            default_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
        })
    }

    /// Create a cache manager with a custom directory.
    pub fn with_dir(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to create cache dir: {e}")))?;

        Ok(Self {
            cache_dir,
            default_ttl: Duration::from_secs(DEFAULT_CACHE_TTL_SECS),
        })
    }

    /// Set the default TTL.
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// Get a cached value.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let path = self.cache_dir.join(format!("{}.json", key));
        if !path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to read cache: {e}")))?;

        let entry: CacheEntry = serde_json::from_str(&content)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to parse cache: {e}")))?;

        if entry.is_expired() {
            // Clean up expired entry
            let _ = std::fs::remove_file(&path);
            Ok(None)
        } else {
            let value: T = serde_json::from_value(entry.data).map_err(|e| {
                DiscoveryError::Cache(format!("Failed to deserialize cache data: {e}"))
            })?;
            Ok(Some(value))
        }
    }

    /// Store a value in the cache.
    pub fn set<T: Serialize>(&self, key: &str, data: T) -> Result<()> {
        self.set_with_ttl(key, data, self.default_ttl.as_secs())
    }

    /// Store a value with a custom TTL.
    pub fn set_with_ttl<T: Serialize>(&self, key: &str, data: T, ttl_secs: u64) -> Result<()> {
        let path = self.cache_dir.join(format!("{}.json", key));
        let entry = CacheEntry::new(data, ttl_secs)?;

        let content = serde_json::to_string_pretty(&entry)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to serialize cache: {e}")))?;

        std::fs::write(&path, content)
            .map_err(|e| DiscoveryError::Cache(format!("Failed to write cache: {e}")))?;

        Ok(())
    }

    /// Remove a cached value.
    pub fn remove(&self, key: &str) -> Result<()> {
        let path = self.cache_dir.join(format!("{}.json", key));
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| DiscoveryError::Cache(format!("Failed to remove cache: {e}")))?;
        }
        Ok(())
    }

    /// Clear all cached values.
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)
                .map_err(|e| DiscoveryError::Cache(format!("Failed to read cache dir: {e}")))?
            {
                let entry = entry
                    .map_err(|e| DiscoveryError::Cache(format!("Failed to read dir entry: {e}")))?;
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_new() {
        let entry = CacheEntry::new("test_data", 3600).unwrap();
        assert!(!entry.is_expired());
        assert_eq!(
            entry.data,
            serde_json::Value::String("test_data".to_string())
        );
    }

    #[test]
    fn test_cache_entry_expired() {
        let mut entry = CacheEntry::new("test_data", 1).unwrap();
        entry.created_at = 0; // Long ago
        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_manager_crud() {
        let dir = std::env::temp_dir().join("opm-test-cache");
        let manager = CacheManager::with_dir(dir.clone()).unwrap();

        manager.set("test_key", "test_value").unwrap();
        let value: Option<String> = manager.get("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        manager.remove("test_key").unwrap();
        let value: Option<String> = manager.get("test_key").unwrap();
        assert_eq!(value, None);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
