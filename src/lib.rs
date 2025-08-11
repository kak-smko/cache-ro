//! # Persistent Cache for Rust
//!
//! A high-performance, thread-safe cache with:
//! - Optional filesystem persistence
//! - Automatic TTL-based cleanup
//! - Key-level locking
//! - Efficient binary serialization
//!
//! ## Features
//!
//! - **Thread-safe** - Uses DashMap for concurrent access
//! - **Persistent** - Optional filesystem storage
//! - **TTL Support** - Automatic expiration of entries
//! - **Efficient** - bincode serialization
//!
//! ## Examples
//!
//! ### Basic Usage
//!
//! ```rust
//! use cache_ro::{Cache, CacheConfig};
//! use std::time::Duration;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = Cache::new(CacheConfig::default())?;
//!     cache.set("key", "value".to_string(), Duration::from_secs(60))?;
//!
//!     if let Some(value) = cache.get::<String>("key") {
//!         println!("Retrieved: {}", value);
//!     }
//!     Ok(())
//! }
//! ```
//!

use bincode::config::{BigEndian, Configuration};
use bincode::serde::{decode_from_slice, encode_to_vec};
use dashmap::DashMap;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, read_dir, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    value: Vec<u8>,
    expires_at: u128,
}

#[derive(Serialize, Deserialize)]
struct PersistentCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Clone)]
pub struct CacheConfig {
    pub persistent: bool,
    pub hash_prefix_length: usize,
    pub cleanup_interval: Duration,
    pub dir_path: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            persistent: true,
            hash_prefix_length: 2,
            cleanup_interval: Duration::from_secs(60),
            dir_path: "cache_data".to_string(),
        }
    }
}

lazy_static! {
    static ref ENTRIES: DashMap<String, CacheEntry> = DashMap::new();
    static ref KEY_LOCKS: DashMap<String, Arc<Mutex<()>>> = DashMap::new();
    static ref FILE_LOCKS: DashMap<String, Arc<Mutex<()>>> = DashMap::new();
}

#[derive(Clone)]
pub struct Cache {
    config: CacheConfig,
}

impl Cache {
    fn config() -> Configuration<BigEndian> {
        bincode::config::standard()
            .with_big_endian()
            .with_variable_int_encoding()
    }

    fn get_file_path(&self, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        let prefix_len = self.config.hash_prefix_length.min(hash.len());
        let prefix = hash[..prefix_len]
            .iter()
            .map(|b| format!("{:02x}", b).get(0..1).unwrap().to_string())
            .collect::<String>();

        Path::new(&self.config.dir_path).join(format!("cache_{}.bin", prefix))
    }

    pub fn new(config: CacheConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if config.persistent {
            fs::create_dir_all(&config.dir_path)?;
        }

        let cache = Self { config };

        if cache.config.persistent {
            cache.load_persistent_data()?;
        }

        let cache_clone = cache.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(cache_clone.config.cleanup_interval);
            cache_clone.cleanup();
        });

        Ok(cache)
    }

    fn load_persistent_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        for entry in read_dir(&self.config.dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("bin") {
                let file_key = path.to_string_lossy().to_string();
                let file_lock = FILE_LOCKS
                    .entry(file_key)
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone();
                let _guard = file_lock.lock().unwrap();

                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;

                let persistent_cache: HashMap<String, CacheEntry> =
                    decode_from_slice(&buffer, Self::config())?.0;

                for (key, entry) in persistent_cache {
                    let key_lock = KEY_LOCKS
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(Mutex::new(())))
                        .clone();
                    let _guard = key_lock.lock().unwrap();

                    ENTRIES.insert(
                        key,
                        CacheEntry {
                            value: entry.value,
                            expires_at: entry.expires_at,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn cleanup(&self) {
        let now = now();
        let mut rm = vec![];
        for i in ENTRIES.iter() {
            if i.expires_at <= now {
                rm.push(i.key().to_string());
            }
        }
        for key in rm {
            let _ = self.remove(&key);
        }
    }

    pub fn get_key_lock(&self, key: &str) -> Arc<Mutex<()>> {
        KEY_LOCKS
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub fn set<V: Serialize>(
        &self,
        key: &str,
        value: V,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = encode_to_vec(&value, Self::config())?;
        let expires_at = now() + ttl.as_millis();

        let key_lock = KEY_LOCKS
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = key_lock.lock().unwrap();

        ENTRIES.insert(
            key.to_string(),
            CacheEntry {
                value: serialized,
                expires_at,
            },
        );

        if self.config.persistent {
            self.persist_key(key)?;
        }

        Ok(())
    }

    pub fn set_without_guard<V: Serialize>(
        &self,
        key: &str,
        value: V,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = encode_to_vec(&value, Self::config())?;
        let expires_at = now() + ttl.as_millis();

        ENTRIES.insert(
            key.to_string(),
            CacheEntry {
                value: serialized,
                expires_at,
            },
        );

        if self.config.persistent {
            self.persist_key(key)?;
        }

        Ok(())
    }

    fn persist_key(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = self.get_file_path(key);
        let file_key = file_path.to_string_lossy().to_string();

        let file_lock = FILE_LOCKS
            .entry(file_key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = file_lock.lock().unwrap();

        let mut persistent_entries = if file_path.exists() {
            let mut file = File::open(&file_path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            let r: PersistentCache = decode_from_slice(&buffer, Self::config())?.0;
            r.entries
        } else {
            HashMap::new()
        };

        if let Some(entry) = ENTRIES.get(key) {
            persistent_entries.insert(
                key.to_string(),
                CacheEntry {
                    value: entry.value.clone(),
                    expires_at: entry.expires_at,
                },
            );
        } else {
            persistent_entries.remove(key);
        }

        if persistent_entries.is_empty() {
            if file_path.exists() {
                fs::remove_file(file_path)?;
            }
            return Ok(());
        }

        let persistent_cache = PersistentCache {
            entries: persistent_entries,
        };
        let serialized = encode_to_vec(&persistent_cache, Self::config())?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)?;
        file.write_all(&serialized)?;

        Ok(())
    }

    pub fn get<V: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<V> {
        let now = now();
        ENTRIES.get(key).and_then(|entry| {
            if entry.expires_at > now {
                decode_from_slice(&entry.value, Self::config())
                    .ok()
                    .map(|(v, _)| v)
            } else {
                None
            }
        })
    }

    pub fn expire(&self, key: &str) -> Option<Duration> {
        let now = now();
        ENTRIES.get(key).and_then(|entry| {
            if entry.expires_at > now {
                let remaining = entry.expires_at - now;
                Some(Duration::from_millis(remaining as u64))
            } else {
                None
            }
        })
    }

    pub fn remove(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        {
            let key_lock = KEY_LOCKS
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            let _guard = key_lock.lock().unwrap();

            ENTRIES.remove(key);

            if self.config.persistent {
                self.persist_key(key)?;
            }
        }
        KEY_LOCKS.remove(key);

        Ok(())
    }

    pub fn remove_without_guard(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        ENTRIES.remove(key);

        if self.config.persistent {
            self.persist_key(key)?;
        }

        Ok(())
    }

    pub fn clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        ENTRIES.clear();
        KEY_LOCKS.clear();
        FILE_LOCKS.clear();

        if self.config.persistent {
            for entry in read_dir(&self.config.dir_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("bin") {
                    fs::remove_file(path)?;
                }
            }
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        ENTRIES.len()
    }

    pub fn is_empty(&self) -> bool {
        ENTRIES.is_empty()
    }
}
