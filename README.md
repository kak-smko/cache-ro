#cache_ro — Persistent, High-Performance, Thread-Safe Cache for Rust

[![Crates.io](https://img.shields.io/crates/v/cache-ro)](https://crates.io/crates/cache-ro)
[![Documentation](https://docs.rs/cache-ro/badge.svg)](https://docs.rs/cache-ro)
[![License](https://img.shields.io/crates/l/cache-ro)](LICENSE)

---

`cache_ro` is a blazing fast, thread-safe Rust cache library designed for **high concurrency** and **optional persistence**. It supports **automatic TTL expiration**, **key-level locking**, and **efficient binary serialization** — making it perfect for applications needing reliable caching with disk-backed durability.

---


## Features

- **Thread-Safe**: Uses `DashMap` internally for lock-free concurrent access.
- **Persistent Storage**: Optionally persists cached entries to disk for durability across restarts.
- **TTL Support**: Automatically expires entries based on time-to-live (TTL) durations.
- **Key-Level Locking**: Provides fine-grained per-key locks for atomic operations.
- **Efficient Serialization**: Uses `bincode` with big-endian and variable integer encoding for compact, fast binary serialization.
- **Background Cleanup**: Runs a background thread to remove expired entries from memory and disk transparently.
- **Simple API**: Clean, intuitive methods for set/get/remove operations with expiration control.

---

## Installation

Add `cache_ro` to your `Cargo.toml`:

```toml
[dependencies]
cache_ro = "0.2"
```

---

## Quick Start

```rust
use cache_ro::{Cache, CacheConfig};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a cache with default config (persistent, 2-char hash prefix)
    let cache = Cache::new(CacheConfig::default())?;

    // Store a key with a TTL of 60 seconds
    cache.set("username", "alice".to_string(), Duration::from_secs(60))?;

    // Retrieve and deserialize cached value
    if let Some(username) = cache.get::<String>("username") {
        println!("Cached username: {}", username);
    } else {
        println!("Cache miss or expired");
    }
    
    // This is matter for close graceful
    Cache::drop();
    Ok(())
}
```

---

## Configuration

Customize cache behavior with `CacheConfig`:

| Field                | Description                              | Default        |
| -------------------- | ---------------------------------------- | -------------- |
| `persistent`         | Enable or disable disk persistence       | `true`         |
| `hash_prefix_length` | Number of hash bytes for filename prefix | `2`            |
| `dir_path`           | Directory path to store cache files      | `"cache_data"` |

---

## API Highlights

### `Cache::set(key, value, ttl)`

Stores a serializable value under `key` with expiration after `ttl`.

### `Cache::get::<T>(key) -> Option<T>`

Retrieves and deserializes a value of type `T` if present and unexpired.

### `Cache::remove(key)`

Removes a key and its persisted data.

### `Cache::clear()`

Clears all in-memory and persistent cache entries.

### `Cache::get_key_lock(key)`

Returns a lock object for atomic operations on a specific key.

---

## Internal Mechanics

* Keys are hashed with SHA-256 and stored in files named with a hash prefix to distribute entries.
* Each cache operation acquires per-key locks for consistency.
* A background thread handles automatic expiration cleanup based on TTL.
* Serialization uses `bincode` with big-endian and variable-length integer encoding for speed and compactness.

---


## Contributing

Contributions are welcome! Please open an issue or submit a PR for:
- New features
- Performance improvements
- Bug fixes


## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.