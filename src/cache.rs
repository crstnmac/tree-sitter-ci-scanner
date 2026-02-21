//! Optional caching layer for incremental scans
//!
//! This module provides caching functionality to speed up repeated scans
//! by storing parsed ASTs and scan results.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache entry storing scan results
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// When this cache entry was created
    pub created_at: SystemTime,
    /// Hash of the file content (for invalidation)
    pub content_hash: String,
    /// Cached scan results
    pub results: Vec<serde_json::Value>,
}

/// Cache for storing scan results
pub struct ScanCache {
    cache_dir: PathBuf,
    entries: HashMap<String, CacheEntry>,
}

impl ScanCache {
    /// Create a new cache with the specified cache directory
    ///
    /// # Arguments
///
    /// * `cache_dir` - Directory to store cache files
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Create cache directory if it doesn't exist
        fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        Ok(Self {
            cache_dir,
            entries: HashMap::new(),
        })
    }

    /// Get the cache file path for a given file path
    fn cache_file_path(&self, file_path: &Path) -> PathBuf {
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        self.cache_dir.join(format!("{}.cache", filename))
    }

    /// Load cache entries from disk
    pub fn load(&mut self) -> Result<()> {
        if !self.cache_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.cache_dir).context("Failed to read cache directory")? {
            let entry = entry.context("Failed to read directory entry")?;

            if entry.path().extension().map_or(false, |e| e == "cache") {
                let _content = fs::read_to_string(&entry.path())
                    .context("Failed to read cache file")?;

                // Deserialize cache entry
                // Note: This is a simplified version - you'd want proper serialization
                tracing::debug!("Loaded cache entry: {:?}", entry.path());
            }
        }

        Ok(())
    }

    /// Save cache entries to disk
    pub fn save(&self) -> Result<()> {
        for (file_path, _entry) in &self.entries {
            let cache_path = self.cache_file_path(Path::new(file_path));

            // Serialize and save cache entry
            // Note: This is a simplified version - you'd want proper serialization
            fs::write(&cache_path, "")
                .context("Failed to write cache file")?;
        }

        Ok(())
    }

    /// Get cached results for a file
    ///
    /// # Arguments
///
    /// * `file_path` - Path to the file
    /// * `content_hash` - Hash of the current file content
    ///
    /// # Returns
///
    /// Cached results if valid, None otherwise
    pub fn get(&self, file_path: &str, content_hash: &str) -> Option<&[serde_json::Value]> {
        if let Some(entry) = self.entries.get(file_path) {
            if entry.content_hash == content_hash {
                return Some(&entry.results);
            }
        }
        None
    }

    /// Store scan results in the cache
    ///
    /// # Arguments
///
    /// * `file_path` - Path to the file
    /// * `content_hash` - Hash of the file content
    /// * `results` - Scan results to cache
    pub fn put(&mut self, file_path: String, content_hash: String, results: Vec<serde_json::Value>) {
        let entry = CacheEntry {
            created_at: SystemTime::now(),
            content_hash,
            results,
        };

        self.entries.insert(file_path, entry);
    }

    /// Clear all cache entries
    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();

        // Remove cache directory contents
        for entry in fs::read_dir(&self.cache_dir).context("Failed to read cache directory")? {
            let entry = entry.context("Failed to read directory entry")?;
            if entry.path().is_file() {
                fs::remove_file(entry.path())
                    .context("Failed to remove cache file")?;
            }
        }

        Ok(())
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_file_path() {
        let cache = ScanCache::new("/tmp/test-cache").unwrap();
        let path = cache.cache_file_path(Path::new("test.js"));
        assert!(path.ends_with("test.js.cache"));
    }
}
