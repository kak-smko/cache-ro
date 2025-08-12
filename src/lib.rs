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
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, read_dir, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

#[derive(Serialize, Deserialize)]
struct PersistentCache {
    entries: HashMap<String, (Vec<u8>, u128)>, // key (value, expires_at)
}

#[derive(Clone)]
pub struct CacheConfig {
    pub persistent: bool,
    pub hash_prefix_length: usize,
    pub dir_path: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            persistent: true,
            hash_prefix_length: 2,
            dir_path: "cache_data".to_string(),
        }
    }
}

lazy_static! {
    static ref ENTRIES: DashMap<String, (Vec<u8>,u128)> = DashMap::new();
    static ref KEY_LOCKS: DashMap<String, Arc<Mutex<()>>> = DashMap::new();
    static ref FILE_LOCKS: DashMap<String, Arc<Mutex<()>>> = DashMap::new();
    static ref CACHE: RwLock<Option<Cache>> = RwLock::new(None);
    static ref EXPIRATION_QUEUE: DashMap<String,u128> = DashMap::new();
    static ref CACHESTATE: AtomicU8 = AtomicU8::new(0);// 0 no run,1 running,2 in closing
    static ref CLEANUP_THREAD_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
}

fn start_cleanup_thread(cache: Cache) -> JoinHandle<()>{
    std::thread::spawn(move || {
        let mut heap:BTreeMap<u128, Vec<String>>=BTreeMap::new();
        loop {
            if CACHESTATE.load(Ordering::SeqCst) != 1 {
                CACHESTATE.store(0, Ordering::SeqCst);
                break;
            }
            let mut removes =vec![];
            {
                for item in EXPIRATION_QUEUE.iter() {
                    heap.entry(*item.value()).or_default().push(item.key().clone());
                    removes.push(item.key().clone());
                }
            }
            for rm in removes{
                EXPIRATION_QUEUE.remove(&rm).unwrap();
            }

            if let Some((next_ts, _)) = heap.iter().next() {
                let now_ms = now();
                if next_ts > &now_ms {
                    let sleep_ms = (next_ts - now_ms).min(5000);
                    std::thread::sleep(Duration::from_millis(sleep_ms as u64));
                    continue;
                }
                let now_ms = now();
                let expired_ts: Vec<u128> = heap
                    .iter()
                    .take_while(|&(&ts, _)| ts <= now_ms)
                    .map(|(&ts, _)| ts)
                    .collect();
                if expired_ts.is_empty() {
                    break;
                }
                for ts in expired_ts {
                    if let Some(keys) = heap.remove(&ts) {
                        for key in keys {
                            let _ = cache.remove(&key);
                        }
                    }
                }
            } else {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    })}

#[derive(Clone)]
pub struct Cache {
    config: CacheConfig,
}

impl Cache {

    /// Drops the global cache instance and clears all in-memory entries, locks, and file locks.
    ///
    /// After calling this, [`Cache::instance`] will return an error until [`Cache::new`] is called again.
    pub fn drop() {
        CACHESTATE.store(2, Ordering::SeqCst);
        if let Some(handle) = CLEANUP_THREAD_HANDLE.lock().unwrap().take() {
            let _ = handle.join();
        }
        ENTRIES.clear();
        KEY_LOCKS.clear();
        FILE_LOCKS.clear();
        let mut conf = CACHE.write().unwrap();
        *conf = None;
    }

    /// Returns the globally initialized [`Cache`] instance if it exists.
    ///
    /// # Errors
    /// Returns an error if [`Cache::new`] has not been called yet.
    ///
    /// # Example
    /// ```
    /// let cache = cache_ro::Cache::instance().unwrap();
    /// ```
    pub fn instance() -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(cache) = CACHE.read().unwrap().as_ref() {
            return Ok(cache.clone());
        }
        Err(Box::new(std::io::Error::new(
            ErrorKind::Other,
            "Cache::new not running",
        )))
    }

    /// Creates and initializes a new global [`Cache`] instance.
    ///
    /// If persistence is enabled in the provided [`CacheConfig`], the directory will be created
    /// and any persisted entries will be loaded into memory.
    ///
    /// This will also start a background thread to periodically clean up expired entries.
    ///
    /// # Errors
    /// Returns an error if:
    /// - A cache instance already exists.
    /// - The persistence directory cannot be created.
    /// # Example
    /// ```
    /// let cache = cache_ro::Cache::new(Default::default()).unwrap();
    /// ```
    pub fn new(config: CacheConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if CACHESTATE.load(Ordering::SeqCst) != 0 {
            return Err(Box::new(std::io::Error::new(
                ErrorKind::Other,
                "Cache is running",
            )));
        }
        CACHESTATE.store(1, Ordering::SeqCst);

        if config.persistent {
            fs::create_dir_all(&config.dir_path)?;
        }

        let cache = Self { config };

        if cache.config.persistent {
            cache.load_persistent_data()?;
        }

        {
            let mut conf = CACHE.write().unwrap();
            *conf = Some(cache.clone());
        }

        let handle = start_cleanup_thread(cache.clone());
        *CLEANUP_THREAD_HANDLE.lock().unwrap() = Some(handle);

        Ok(cache)
    }

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

                let persistent_cache: HashMap<String, (Vec<u8>,u128)> =
                    decode_from_slice(&buffer, Self::config())?.0;

                for (key, (value,expires_at)) in persistent_cache {
                    let key_lock = KEY_LOCKS
                        .entry(key.clone())
                        .or_insert_with(|| Arc::new(Mutex::new(())))
                        .clone();
                    let _guard = key_lock.lock().unwrap();

                    ENTRIES.insert(key.clone(),  (value,expires_at));
                    EXPIRATION_QUEUE.insert( key,expires_at);
                }
            }
        }
        Ok(())
    }


    /// Returns the per-key lock for the specified cache key.
    ///
    /// Useful when performing multiple operations atomically for a single key.
    ///
    /// # Arguments
    /// * `key` - The key to lock.
    ///
    /// # Example
    /// ```
    /// let cache = cache_ro::Cache::new(Default::default()).unwrap();
    /// let lock = cache.get_key_lock("my_key");
    /// let _guard = lock.lock().unwrap();
    /// // do protected work here
    /// ```
    pub fn get_key_lock(&self, key: &str) -> Arc<Mutex<()>> {
        KEY_LOCKS
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }


    /// Stores a value in the cache with a specified TTL (time-to-live).
    ///
    /// If persistence is enabled, the value will also be saved to disk.
    ///
    /// # Arguments
    /// * `key` - Cache key.
    /// * `value` - Serializable value to store.
    /// * `ttl` - Expiration duration.
    ///
    /// # Errors
    /// Returns an error if serialization or persistence fails.
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

        ENTRIES.insert(key.to_string(), (serialized,expires_at));
        EXPIRATION_QUEUE.insert( key.to_string(),expires_at);
        if self.config.persistent {
            self.persist_key(key)?;
        }

        Ok(())
    }

    /// Stores a value in the cache without acquiring a key lock.
    ///
    /// Intended for internal use when you already hold the lock.
    ///
    /// # Arguments
    /// * `key` - Cache key.
    /// * `value` - Serializable value to store.
    /// * `ttl` - Expiration duration.
    ///
    /// # Errors
    /// Returns an error if serialization or persistence fails.
    pub fn set_without_guard<V: Serialize>(
        &self,
        key: &str,
        value: V,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = encode_to_vec(&value, Self::config())?;
        let expires_at = now() + ttl.as_millis();

        ENTRIES.insert(key.to_string(), (serialized,expires_at), );
        EXPIRATION_QUEUE.insert( key.to_string(),expires_at);
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

        if let Some(v) = ENTRIES.get(key) {
            persistent_entries.insert(key.to_string(), v.clone());
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

    /// Retrieves and deserializes a value from the cache if it has not expired.
    ///
    /// # Type Parameters
    /// * `V` - Type to deserialize into.
    ///
    /// # Arguments
    /// * `key` - Cache key.
    ///
    /// # Returns
    /// `Some(value)` if the entry exists and is valid, otherwise `None`.
    pub fn get<V: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<V> {
        let now = now();
        ENTRIES.get(key).and_then(|entry| {
            if entry.1 > now {
                decode_from_slice(&entry.0, Self::config())
                    .ok()
                    .map(|(v, _)| v)
            } else {
                None
            }
        })
    }


    /// Returns the remaining TTL for a given cache key.
    ///
    /// # Returns
    /// `Some(duration)` if the entry exists and is not expired, otherwise `None`.
    pub fn expire(&self, key: &str) -> Option<Duration> {
        let now = now();
        ENTRIES.get(key).and_then(|entry| {
            if entry.1 > now {
                let remaining = entry.1 - now;
                Some(Duration::from_millis(remaining as u64))
            } else {
                None
            }
        })
    }

    /// Removes an entry from the cache.
    ///
    /// If persistence is enabled, the removal is also reflected on disk.
    ///
    /// # Errors
    /// Returns an error if persistence fails.
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

    /// Removes an entry without acquiring the key lock.
    ///
    /// Used internally when the lock is already held.
    ///
    /// # Errors
    /// Returns an error if persistence fails.
    pub fn remove_without_guard(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        ENTRIES.remove(key);

        if self.config.persistent {
            self.persist_key(key)?;
        }

        Ok(())
    }

    /// Clears all entries from the cache (in memory and on disk if persistent).
    ///
    /// # Errors
    /// Returns an error if persistent files cannot be deleted.
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

    /// Returns the number of entries currently stored in memory.
    pub fn len(&self) -> usize {
        ENTRIES.len()
    }

    /// Checks whether the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        ENTRIES.is_empty()
    }
}
