use std::sync::Arc;
use std::time::{Duration};
use serde::{Serialize, Deserialize};
use std::{fs, thread};
use std::thread::{sleep, spawn};
use cache_ro::{Cache, CacheConfig};

// Helper function to create a temp directory
fn setup_temp_dir() -> String {
    let dir = "./test_cache_temp";
    fs::create_dir_all(dir).unwrap();
    dir.to_string()
}

// Helper function to clean up temp directory
fn cleanup_temp_dir(dir: &str) {
    fs::remove_dir_all(dir).unwrap();
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
struct TestData {
    field1: String,
    field2: i32,
}

#[test]
fn test_file_backed_cache() {
    let dir = setup_temp_dir();
    // Create new cache
    let cache = Cache::new(
        CacheConfig {
            persistent: true,
            dir_path:dir.clone(),
            ..Default::default()
        }
    )
    .unwrap();

    // Test set and get
    let data = TestData {
        field1: "test".to_string(),
        field2: 4203558,
    };
    cache
        .set("key1", data.clone(), Duration::from_secs(2))
        .unwrap();
    assert_eq!(cache.get::<TestData>("key1"), Some(data.clone()));
    // Test persistence
    drop(cache);
    let cache = Cache::new(
        CacheConfig {
            persistent: true,
            dir_path:dir.clone(),
            ..Default::default()
        }
    )
    .unwrap();
    assert_eq!(cache.get::<TestData>("key1"), Some(data.clone()));

    // Test expiration
    thread::sleep(Duration::from_secs(3));
    assert_eq!(cache.get::<TestData>("key1"), None);

    cache.set("key", 23, Duration::from_secs(20)).unwrap();
    assert_eq!(Some(23), cache.get::<i32>("key"));
    // Clean up after test
    cleanup_temp_dir(&dir);
}
#[test]
fn test_basic_operations() {
    let cache = Cache::new(CacheConfig {
        persistent: false,
        ..Default::default()
    })
    .unwrap();

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
}

#[test]
fn test_expiration() {
    let cache = Cache::new(CacheConfig {
        persistent: false,
        ..Default::default()
    })
    .unwrap();

    // Test immediate expiration
    cache
        .set("temp_key", "temp_value".to_string(), Duration::from_secs(0))
        .unwrap();
    sleep(Duration::from_millis(10));
    assert!(cache.get::<String>("temp_key").is_none());

    // Test expiration after time
    cache.set(
            "short_key",
            "short_value".to_string(),
            Duration::from_millis(50),
        )
        .unwrap();
    assert!(cache.get::<String>("short_key").is_some());
    sleep(Duration::from_millis(60));
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
}
#[test]
fn test_expiration_file() {
    let dir = setup_temp_dir();
    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path:dir.clone(),
        ..Default::default()
    })
    .unwrap();


    cache.set("temp_key", "temp_value".to_string(), Duration::from_millis(100))
        .unwrap();
    drop(cache);
    sleep(Duration::from_secs(1));
    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path:dir.clone(),
        ..Default::default()
    })
        .unwrap();
    assert!(cache.get::<String>("temp_key").is_none());
    cleanup_temp_dir(&dir)
}

#[test]
fn test_persistent_storage() {
    let dir = setup_temp_dir();

    // First create and populate cache
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap();

        cache
            .set("persist1", "value1".to_string(), Duration::from_secs(2))
            .unwrap();
        cache.set("persist2", 42, Duration::from_secs(10)).unwrap();
    }
    sleep(Duration::from_secs(3));
    // Then create new cache instance and check if data persisted
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap();

        assert!(cache.get::<String>("persist1").is_none());
        assert_eq!(cache.get::<i32>("persist2").unwrap(), 42);
    }

    cleanup_temp_dir(&dir);
}

#[test]
fn test_persistent_cleanup() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    })
    .unwrap();

    cache
        .set("temp1", "value1".to_string(), Duration::from_millis(50))
        .unwrap();
    cache
        .set("perm1", "value2".to_string(), Duration::from_secs(10))
        .unwrap();

    // Wait for cleanup to happen
    sleep(Duration::from_millis(150));

    assert!(cache.get::<String>("temp1").is_none());
    assert!(cache.get::<String>("perm1").is_some());

    cleanup_temp_dir(&dir);
}

#[test]
fn test_non_persistent_mode() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: false,
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();

    cache
        .set("key1", "value1".to_string(), Duration::from_secs(10))
        .unwrap();

    // Should not create any files
    let files = fs::read_dir(&dir).unwrap();
    assert_eq!(files.count(), 0);

    cleanup_temp_dir(&dir);
}

#[test]
fn test_hash_prefix_length() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        hash_prefix_length: 3,
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();

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
        assert_eq!(prefix.len(), 3); // 3 bytes as hex
    }

    cleanup_temp_dir(&dir);
}

#[test]
fn test_multiple_file_creation() {
    let dir = setup_temp_dir();

    let cache = Cache::new(CacheConfig {
        persistent: true,
        hash_prefix_length: 1, // Very short prefix to increase chance of different files
        dir_path: dir.clone(),
        ..Default::default()
    })
    .unwrap();

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

    cleanup_temp_dir(&dir);
}

#[test]
fn test_config_changes() {
    let dir = setup_temp_dir();

    // First config with prefix length 2
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            hash_prefix_length: 2,
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap();
        cache
            .set("key1", "value1".to_string(), Duration::from_secs(10))
            .unwrap();
    }

    // Then change to prefix length 4
    {
        let cache = Cache::new(CacheConfig {
            persistent: true,
            hash_prefix_length: 4,
            dir_path: dir.clone(),
            ..Default::default()
        })
        .unwrap();

        // Should still be able to read old data
        assert_eq!(cache.get::<String>("key1").unwrap(), "value1");

        // New writes use new prefix length
        cache
            .set("key2", "value2".to_string(), Duration::from_secs(10))
            .unwrap();
    }

    // Verify both files exist
    let files: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("cache_") && name.ends_with(".bin"))
        .collect();

    assert!(files.iter().any(|f| f.len() == "cache_xxxx.bin".len())); // length 4
    assert!(files.iter().any(|f| f.len() == "cache_xx.bin".len())); // length 2

    cleanup_temp_dir(&dir);
}

#[test]
fn test_multithreaded_reads() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
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
}

#[test]
fn test_multithreaded_writes() {
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: false,
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
}

#[test]
fn test_multithreaded_persistent() {
    let dir = setup_temp_dir();
    let cache = Arc::new(
        Cache::new(CacheConfig {
            persistent: true,
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

    // Verify all writes were persisted
    let cache2 = Cache::new(CacheConfig {
        persistent: true,
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

    cleanup_temp_dir(&dir);
}

#[test]
fn test_multithreaded_cleanup() {
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

    cleanup_temp_dir(&dir);
}

#[test]
fn test_read_write_mix() {
    let cache = Arc::new(Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_secs(5),
        ..Default::default()
    }).unwrap());

    let mut handles = vec![];

    // Writers
    for i in 0..50 {
        let cache = cache.clone();
        handles.push(spawn( move|| {
            for j in 0..20 {
                let key = format!("{}-{}", i, j);
                cache.set(&key, j,Duration::from_secs(j as u64)).unwrap();
            }
        }));
    }

    // Readers
    for _ in 0..50 {
        let cache = cache.clone();
        handles.push(spawn( move|| {
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
        handles.push(spawn( move|| {
            for j in 0..5 {
                let key = format!("{}-{}", i, j);
                cache.remove(&key).ok();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_expiration_under_load() {
    let cache = Arc::new(Cache::new(CacheConfig {
        persistent: false,
        cleanup_interval: Duration::from_millis(100),
        ..Default::default()
    }).unwrap());

    // Add short-lived items
    for i in 0..100 {
        cache.set(&format!("short-{}", i), i, (Duration::from_millis(50))).unwrap();
    }

    // Add long-lived items
    for i in 0..100 {
        cache.set(&format!("long-{}", i), i, (Duration::from_secs(10))).unwrap();
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
}


#[test]
fn test_high_contention() {
    let dir=setup_temp_dir();
    let cache = Arc::new(Cache::new(CacheConfig {
        persistent: true,
        dir_path:dir.clone(),
        ..Default::default()
    }).unwrap());

    let mut handles = vec![];
    let key = "contended_key".to_string();

    // 100 threads competing for the same key
    for i in 0..100 {
        let cache = cache.clone();
        let key = key.clone();
        handles.push(spawn( move|| {
            for _ in 0..100 {
                let key_lock=cache.get_key_lock(&key);
                let _unused = key_lock.lock();
                let mut value = cache.get::<i32>(&key).unwrap_or(0);
                value += 1;
                cache.set_without_guard(&key, value, Duration::from_secs(1)).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(cache.get::<i32>(&key), Some(10000));
    cleanup_temp_dir(&dir);
}

#[test]
fn test_massive_expiration() {
    let dir=setup_temp_dir();
    let cache = Arc::new(Cache::new(CacheConfig {
        persistent: true,
        dir_path:dir.clone(),
        cleanup_interval: Duration::from_millis(50),
        ..Default::default()
    }).unwrap());

    // Add 10,000 items that will expire quickly
    for i in 0..10_000 {
        cache.set(&format!("item-{}", i), i, (Duration::from_millis(10))).unwrap();
    }

    // Let cleanup run
    sleep(Duration::from_millis(1000));

    // Verify all expired
    assert_eq!(cache.len(), 0);
    cleanup_temp_dir(&dir);
}

#[test]
fn test_persistent_concurrent() {
    let dir = setup_temp_dir();
    let config = CacheConfig {
        persistent: true,
        dir_path: dir.clone(),
        ..Default::default()
    };

    let cache = Arc::new(Cache::new(config.clone()).unwrap());

    let mut handles = vec![];
    for i in 0..50 {
        let cache = cache.clone();
        handles.push(spawn( move|| {
            for j in 0..20 {
                let key = format!("{}-{}", i, j);
                cache.set(&key, j, (Duration::from_secs(10))).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Reload cache from disk
    drop(cache);
    let cache = Arc::new(Cache::new(config).unwrap());

    // Verify all items persisted
    for i in 0..50 {
        for j in 0..20 {
            let key = format!("{}-{}", i, j);
            assert_eq!(cache.get::<i32>(&key), Some(j));
        }
    }
    cleanup_temp_dir(&dir)
}