# Persistent Cache for Rust

[![Crates.io](https://img.shields.io/crates/v/cache-ro)](https://crates.io/crates/cache-ro)
[![Documentation](https://docs.rs/cache-ro/badge.svg)](https://docs.rs/cache-ro)
[![License](https://img.shields.io/crates/l/cache-ro)](LICENSE)

A high-performance, thread-safe persistent cache with TTL support.

## Features

- 🚀 Thread-safe operations using DashMap
- 💾 Optional persistent storage
- ⏱ Time-to-live (TTL) support
- 🔒 Key-level locking for safe concurrent access
- 📦 Efficient binary serialization with bincode

## Usage

Add to your `Cargo.toml`:
```toml
[dependencies]
cache-ro = "0.1"
```

## Example
```rust
use cache_ro::{Cache, CacheConfig};
use std::time::Duration;

let cache = Cache::new(CacheConfig::default())?;
cache.set("my_key", "my_value".to_string(), Duration::from_secs(60))?;
let value: String = cache.get("my_key").unwrap();
```

## Contributing

Contributions are welcome! Please open an issue or submit a PR for:
- New features
- Performance improvements
- Bug fixes


## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.