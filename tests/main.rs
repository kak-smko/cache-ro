use cache_ro::{Cache, CacheConfig};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};
use std::{fs, thread};

// Helper function to create a temp directory
fn setup_temp_dir() -> String {
    let dir = "./test_cache_temp";
    fs::create_dir_all(dir).unwrap();
    dir.to_string()
}

// Helper function to clean up temp directory
fn cleanup_temp_dirs() {
    fs::remove_dir_all("./test_cache_temp").unwrap();
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
struct TestData {
    field1: String,
    field2: i32,
}

#[test]
fn test_all() {
    let start=Instant::now();
    println!("test_non_persistent_mode");
    test_non_persistent_mode();
    println!("test_hash_prefix_length");
    test_hash_prefix_length();
    println!("test_interval");
    test_interval();
    println!("test_massive_expiration");
    test_massive_expiration();
    println!("test_file_backed_cache");
    test_file_backed_cache();
    println!("test_basic_operations");
    test_basic_operations();
    println!("test_expiration");
    test_expiration();
    println!("test_multi_expiration");
    test_multi_expiration();
    println!("test_expiration_file");
    test_expiration_file();
    println!("test_persistent_storage");
    test_persistent_storage();
    println!("test_persistent_cleanup");
    test_persistent_cleanup();
    println!("test_multiple_file_creation");
    test_multiple_file_creation();
    println!("test_multithreaded_reads");
    test_multithreaded_reads();
    println!("test_multithreaded_writes");
    test_multithreaded_writes();
    println!("test_multithreaded_persistent");
    test_multithreaded_persistent();
    println!("test_multithreaded_cleanup");
    test_multithreaded_cleanup();
    println!("test_read_write_mix");
    test_read_write_mix();
    println!("test_expiration_under_load");
    test_expiration_under_load();
    println!("test_high_contention");
    test_high_contention();
    println!("test_persistent_concurrent");
    test_persistent_concurrent();
    println!("elapsed: {}s",start.elapsed().as_secs());
}
fn test_non_persistent_mode() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_millis(100),
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();
    cache.clear().unwrap();
    cache
        .set("key1", "value1".to_string(), Duration::from_secs(10))
        .unwrap();

    // Should not create any files
    let files = fs::read_dir(&dir).unwrap();
    assert_eq!(files.count(), 0);
    Cache::drop()
}

fn test_hash_prefix_length() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        hash_prefix_length: 4,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
    })
    .unwrap();
    cache.clear().unwrap();

    cache
        .set("key1", "value1".to_string(), Duration::from_secs(10))
        .unwrap();
    cache
        .set("key2", "value2".to_string(), Duration::from_secs(10))
        .unwrap();

    // Check that files were created with 3-character prefixes
    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("cache_") && name.ends_with(".bin"))
        .collect();

    assert!(!files.is_empty());
    for file in files {
        let prefix = file
            .strip_prefix("cache_")
            .unwrap()
            .strip_suffix(".bin")
            .unwrap();
        assert_eq!(prefix.len(), 4); // 4 bytes as hex
    }

    cleanup_temp_dirs();
    Cache::drop()
}

fn test_interval() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        hash_prefix_length: 4,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
    })
        .unwrap();
    cache.clear().unwrap();

    cache
        .set("key1", "value1".to_string(), Duration::from_secs(1))
        .unwrap();

    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("cache_") && name.ends_with(".bin"))
        .collect();

    assert!(!files.is_empty());
    sleep(Duration::from_secs(2));
    // Check that files were created with 3-character prefixes
    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("cache_") && name.ends_with(".bin"))
        .collect();

    assert!(files.is_empty());

    cleanup_temp_dirs();
    Cache::drop()
}

fn test_massive_expiration() {
    let dir = setup_temp_dir();
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: true,
            dir_path: dir.clone(),
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );

    // Add 10,000 items that will expire quickly
    for i in 0..10_000 {
        cache
            .set(&format!("user-ip-{}", i), "127.0.0.1", Duration::from_millis(10))
            .unwrap();
    }

    // Let cleanup run
    sleep(Duration::from_millis(1000));

    // Verify all expired
    assert_eq!(cache.len(), 0);
    cleanup_temp_dirs();
    Cache::drop()
}


fn test_file_backed_cache() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();
    // Test set and get
    let data = TestData {
        field1: "test".to_string(),
        field2: 4203558,
    };
    cache
        .set("key1", data.clone(), Duration::from_secs(200))
        .unwrap();
    assert_eq!(cache.get::<TestData>("key1"), Some(data.clone()));
    // Test persistence
    Cache::drop();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(cache.get::<TestData>("key1"), Some(data.clone()));


    cache.set("key", 23, Duration::from_secs(20)).unwrap();
    assert_eq!(Some(23), cache.get::<i32>("key"));
    cleanup_temp_dirs();
    Cache::drop()
}

fn test_basic_operations() {
    let cache = Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();
    cache.clear().unwrap();
    // Test set and get
    cache
        .set("key1", "value1".to_string(), Duration::from_secs(10))
        .unwrap();
    assert_eq!(cache.get::<String>("key1").unwrap(), "value1");

    // Test remove
    cache.remove("key1").unwrap();
    assert!(cache.get::<String>("key1").is_none());

    // Test clear
    cache
        .set("key2", "value2".to_string(), Duration::from_secs(10))
        .unwrap();
    cache
        .set("key3", "value3".to_string(), Duration::from_secs(10))
        .unwrap();
    assert_eq!(cache.len(), 2);
    cache.clear().unwrap();
    assert_eq!(cache.len(), 0);
    Cache::drop()
}


fn test_expiration() {
    let cache = Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_millis(10),
        ..Default::default()
    })
    .unwrap();

    // Test immediate expiration
    cache
        .set("temp_key", "temp_value".to_string(), Duration::from_secs(0))
        .unwrap();
    sleep(Duration::from_millis(100));
    assert!(cache.get::<String>("temp_key").is_none());

    // Test expiration after time
    cache
        .set(
            "short_key",
            "short_value".to_string(),
            Duration::from_millis(50),
        )
        .unwrap();
    assert!(cache.get::<String>("short_key").is_some());
    sleep(Duration::from_millis(100));
    assert!(cache.get::<String>("short_key").is_none());

    // Test expire() function
    cache
        .set(
            "check_key",
            "check_value".to_string(),
            Duration::from_secs(1),
        )
        .unwrap();
    let remaining = cache.expire("check_key").unwrap();
    assert!(remaining <= Duration::from_secs(1) && remaining > Duration::from_secs(0));
    Cache::drop()
}

fn test_multi_expiration() {
    let cache = Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();

    for i in 0..1000{
        cache
            .set(&format!("temp_key {i}"), "temp_value".to_string(), Duration::from_millis(i*10))
            .unwrap();
    }
    cache.remove("temp_key 250").unwrap();
    sleep(Duration::from_millis(1000));
    assert!(cache.get::<String>("temp_key 0").is_none());
    assert!(cache.get::<String>("temp_key 30").is_none());
    assert!(cache.get::<String>("temp_key 100").is_none());
    assert!(cache.get::<String>("temp_key 250").is_none());
    assert!(cache.get::<String>("temp_key 120").is_some());
    assert!(cache.get::<String>("temp_key 200").is_some());
    assert!(cache.get::<String>("temp_key 999").is_some());
    Cache::drop()
}

fn test_expiration_file() {
    let dir = setup_temp_dir();
    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();

    cache
        .set(
            "temp_key",
            "temp_value".to_string(),
            Duration::from_millis(100),
        )
        .unwrap();
    Cache::drop();
    sleep(Duration::from_millis(200));
    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
        .unwrap();
    assert!(cache.get::<String>("temp_key").is_none());
    cleanup_temp_dirs();
    Cache::drop()
}

fn test_persistent_storage() {
    let dir = setup_temp_dir();

    // First create and populate cache
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            dir_path: dir.clone(),
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap();
        cache
            .set("persist1", "value1".to_string(), Duration::from_millis(100))
            .unwrap();
        cache.set("persist2", 42, Duration::from_secs(10)).unwrap();
    }
    Cache::drop();
    sleep(Duration::from_millis(200));
    // Then create new cache instance and check if data persisted
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            cleanup_interval: Duration::from_millis(100),
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap();

        assert!(cache.get::<String>("persist1").is_none());
        assert_eq!(cache.get::<i32>("persist2").unwrap(), 42);
    }

    cleanup_temp_dirs();
    Cache::drop()
}

fn test_persistent_cleanup() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        cleanup_interval: Duration::from_millis(100),
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();

    cache
        .set("temp1", "value1".to_string(), Duration::from_millis(50))
        .unwrap();
    cache
        .set("perm1", "value2".to_string(), Duration::from_secs(10))
        .unwrap();

    sleep(Duration::from_millis(100));

    assert!(cache.get::<String>("temp1").is_none());
    assert!(cache.get::<String>("perm1").is_some());
    cleanup_temp_dirs();
    Cache::drop()
}

fn test_multiple_file_creation() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        cleanup_interval: Duration::from_millis(100),
        hash_prefix_length: 1, // Very short prefix to increase chance of different files
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();
    cache.clear().unwrap();

    // Add enough keys to likely create multiple files
    for i in 0..100 {
        cache
            .set(&format!("key{}", i), i, Duration::from_secs(10))
            .unwrap();
    }

    // Verify multiple files were created
    let file_count = fs::read_dir(&dir).unwrap().count();
    assert!(
        file_count > 1,
        "Expected multiple cache files, got {}",
        file_count
    );

    cleanup_temp_dirs();
    Cache::drop()
}

fn test_multithreaded_reads() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );
    // Initialize with some data
    cache
        .set("shared_key", 42, Duration::from_secs(10))
        .unwrap();

    let mut handles = vec![];
    for _ in 0..10 {
        let cache_clone = cache.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                assert_eq!(cache_clone.get::<i32>("shared_key").unwrap(), 42);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    Cache::drop()
}

fn test_multithreaded_writes() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );

    let mut handles = vec![];
    for i in 0..10 {
        let cache_clone = cache.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread{}_key{}", i, j);
                cache_clone.set(&key, j, Duration::from_secs(10)).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all writes completed
    for i in 0..10 {
        for j in 0..100 {
            let key = format!("thread{}_key{}", i, j);
            assert_eq!(cache.get::<i32>(&key).unwrap(), j);
        }
    }
    Cache::drop()
}

fn test_multithreaded_persistent() {
    let dir = setup_temp_dir();
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: true,
            cleanup_interval: Duration::from_millis(100),
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap(),
    );

    let mut handles = vec![];
    for i in 0..10 {
        let cache_clone = cache.clone();
        handles.push(thread::spawn(move || {
            for j in 0..50 {
                let key = format!("thread{}_key{}", i, j);
                cache_clone.set(&key, j, Duration::from_secs(10)).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Cache::drop();
    // Verify all writes were persisted
    let cache2 = Cache::new(CacheConfig {
        persistent: true,
        cleanup_interval: Duration::from_millis(100),
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();

    for i in 0..10 {
        for j in 0..50 {
            let key = format!("thread{}_key{}", i, j);
            assert_eq!(cache2.get::<i32>(&key).unwrap(), j);
        }
    }
    cleanup_temp_dirs();
    Cache::drop()
}

fn test_multithreaded_cleanup() {
    let dir = setup_temp_dir();
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: true,
            cleanup_interval: Duration::from_millis(100),
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap(),
    );

    let mut handles = vec![];
    for i in 0..5 {
        let cache_clone = cache.clone();
        handles.push(thread::spawn(move || {
            for j in 0..20 {
                let key = format!("short{}_key{}", i, j);
                cache_clone.set(&key, j, Duration::from_millis(50)).unwrap();

                let key = format!("long{}_key{}", i, j);
                cache_clone.set(&key, j, Duration::from_secs(10)).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Wait for cleanup to happen
    sleep(Duration::from_millis(150));

    // Verify short-lived keys are gone, long-lived keys remain
    for i in 0..5 {
        for j in 0..20 {
            let short_key = format!("short{}_key{}", i, j);
            assert!(cache.get::<i32>(&short_key).is_none());

            let long_key = format!("long{}_key{}", i, j);
            assert_eq!(cache.get::<i32>(&long_key).unwrap(), j);
        }
    }
    cleanup_temp_dirs();
    Cache::drop()
}


fn test_read_write_mix() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );

    let mut handles = vec![];

    // Writers
    for i in 0..50 {
        let cache = cache.clone();
        handles.push(spawn(move || {
            for j in 0..20 {
                let key = format!("{}-{}", i, j);
                cache.set(&key, j, Duration::from_secs(j as u64)).unwrap();
            }
        }));
    }

    // Readers
    for _ in 0..50 {
        let cache = cache.clone();
        handles.push(spawn(move || {
            for i in 0..50 {
                for j in 0..20 {
                    let key = format!("{}-{}", i, j);
                    let _ = cache.get::<i32>(&key);
                }
            }
        }));
    }

    // Deleters
    for i in 0..20 {
        let cache = cache.clone();
        handles.push(spawn(move || {
            for j in 0..5 {
                let key = format!("{}-{}", i, j);
                cache.remove(&key).ok();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    Cache::drop()
}


fn test_expiration_under_load() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
            cleanup_interval: Duration::from_millis(100),
            ..Default::default()
        })
        .unwrap(),
    );

    // Add short-lived items
    for i in 0..100 {
        cache
            .set(&format!("short-{}", i), i, Duration::from_millis(50))
            .unwrap();
    }

    // Add long-lived items
    for i in 0..100 {
        cache
            .set(&format!("long-{}", i), i, Duration::from_secs(10))
            .unwrap();
    }

    // Let cleanup run
    sleep(Duration::from_millis(200));

    // Verify short items expired
    for i in 0..100 {
        assert!(cache.get::<i32>(&format!("short-{}", i)).is_none());
    }

    // Verify long items still exist
    for i in 0..100 {
        assert_eq!(cache.get::<i32>(&format!("long-{}", i)), Some(i));
    }
    Cache::drop()
}


fn test_high_contention() {
    let dir = setup_temp_dir();
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: true,
            cleanup_interval: Duration::from_millis(100),
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap(),
    );

    let mut handles = vec![];
    let key = "contended_key".to_string();
    let lock=Arc::new(Mutex::new(0));
    // 100 threads competing for the same key
    for _ in 0..100 {
        let cache = cache.clone();
        let key = key.clone();
        let lock = lock.clone();
        handles.push(spawn(move || {
            for _ in 0..100 {
                let _unused = lock.lock().unwrap();
                let mut value = cache.get::<i32>(&key).unwrap_or(0);
                value += 1;
                cache
                    .set(&key, value, Duration::from_secs(10))
                    .unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(cache.get::<i32>(&key), Some(10000));

    cleanup_temp_dirs();
    Cache::drop()
}


fn test_persistent_concurrent() {
    let dir = setup_temp_dir();
    let config = CacheConfig {
        persistent: true,
        cleanup_interval: Duration::from_millis(100),
        dir_path: dir.clone(),
        ..Default::default()
    };

    let cache = Arc::new(Cache::new(config.clone()).unwrap());
    let _=&cache.clear().unwrap();
    let mut handles = vec![];
    for i in 0..50 {
        let cache = cache.clone();
        handles.push(spawn(move || {
            for j in 0..20 {
                let key = format!("{}-{}", i, j);
                cache.set(&key, j, Duration::from_secs(10)).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Reload cache from disk
    Cache::drop();
    let cache = Arc::new(Cache::new(config.clone()).unwrap());

    // Verify all items persisted
    for i in 0..50 {
        for j in 0..20 {
            let key = format!("{}-{}", i, j);
            assert_eq!(cache.get::<i32>(&key), Some(j));
        }
    }
    cleanup_temp_dirs();
    Cache::drop()
}
