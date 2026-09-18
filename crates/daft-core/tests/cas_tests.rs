use daft_core::cas::{ObjectId, ObjectStore, ObjectType, RawObject};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_object_id_roundtrip() {
    let raw = [42u8; 32];
    let oid = ObjectId::from_bytes(raw);
    assert_eq!(oid.as_bytes(), &raw);
    assert_eq!(oid.into_bytes(), raw);
    assert!(!oid.is_zero());

    let hex_str = oid.to_hex();
    assert_eq!(hex_str.len(), 64);
    let parsed = ObjectId::from_hex(&hex_str).expect("parse from valid hex");
    assert_eq!(oid, parsed);

    let (prefix, remainder) = oid.fanout_parts();
    assert_eq!(prefix.len(), 2);
    assert_eq!(remainder.len(), 62);
    assert_eq!(format!("{}{}", prefix, remainder), hex_str);

    // Invalid hex length
    assert!(ObjectId::from_hex("abcd").is_err());
    // Non-hex chars
    assert!(ObjectId::from_hex(&"g".repeat(64)).is_err());

    // Zero ID
    assert!(ObjectId::ZERO.is_zero());
}

#[test]
fn test_raw_object_envelope_framing() {
    let payload = b"hello daft vcs".to_vec();
    let raw = RawObject::blob(payload.clone());
    let framed = raw.framed_bytes();
    assert_eq!(&framed[..8], b"blob 14\0");
    assert_eq!(&framed[8..], &payload[..]);

    let parsed = RawObject::parse_framed(&framed).expect("parse valid framed");
    assert_eq!(parsed.object_type, ObjectType::Blob);
    assert_eq!(parsed.data, payload);

    // Missing null byte error
    assert!(RawObject::parse_framed(b"blob 14 hello").is_err());
    // Missing space delimiter
    assert!(RawObject::parse_framed(b"blob14\0hello").is_err());
    // Size mismatch
    assert!(RawObject::parse_framed(b"blob 20\0hello").is_err());
}

#[test]
fn test_cas_store_write_and_read_blob() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    let data = b"Content of a test file with some binary \x00\xFF\xAA bytes";
    let oid = store.write_blob(data).expect("write blob");
    assert!(!oid.is_zero());

    assert!(store.has_object(&oid));

    let read_back = store.read_blob(&oid).expect("read blob");
    assert_eq!(read_back, data);

    // Verified read
    let verified = store.read_raw_verified(&oid).expect("read verified");
    assert_eq!(verified.data, data);
}

#[test]
fn test_cas_store_deduplication() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    let data = b"identical duplicate content";
    let oid1 = store.write_blob(data).expect("write 1");
    let oid2 = store.write_blob(data).expect("write 2");

    assert_eq!(oid1, oid2);
    let objects = store.list_objects().expect("list objects");
    assert_eq!(objects.len(), 1);
}

#[test]
fn test_cas_store_mmap_read() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    // 1 MB payload
    let large_data = vec![0xABu8; 1024 * 1024];
    let oid = store.write_blob(&large_data).expect("write large blob");

    let mmap_read = store.read_raw_mmap(&oid).expect("mmap read");
    assert_eq!(mmap_read.data.len(), large_data.len());
    assert_eq!(mmap_read.data, large_data);
}

#[test]
fn test_cas_store_clean_stale_tmp() {
    let dir = tempdir().expect("tempdir");
    let store = ObjectStore::init(dir.path().join("objects")).expect("init store");

    // Create a dummy file in tmp
    let stale_file = store.tmp_path().join("stale_temp_file");
    std::fs::write(&stale_file, b"orphan").expect("write stale file");
    assert!(stale_file.exists());

    // Clean with zero duration (all files older than 0s)
    std::thread::sleep(Duration::from_millis(50));
    let cleaned = store
        .clean_stale_tmp(Duration::from_millis(10))
        .expect("clean stale");
    assert!(cleaned >= 1);
    assert!(!stale_file.exists());
}

#[test]
fn test_cas_concurrent_identical_writes() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(ObjectStore::init(dir.path().join("objects")).expect("init store"));

    let data = Arc::new(b"concurrent shared data across multiple dimensions".to_vec());
    let mut handles = Vec::new();

    for _ in 0..16 {
        let store_clone = Arc::clone(&store);
        let data_clone = Arc::clone(&data);
        handles.push(thread::spawn(move || {
            store_clone.write_blob(&data_clone).expect("write blob")
        }));
    }

    let mut oids = Vec::new();
    for handle in handles {
        oids.push(handle.join().expect("thread join"));
    }

    // All threads must return the exact same OID
    let first_oid = oids[0];
    for oid in &oids {
        assert_eq!(oid, &first_oid);
    }

    // Store must have exactly 1 object
    assert_eq!(store.list_objects().expect("list").len(), 1);
    assert_eq!(store.read_blob(&first_oid).expect("read"), *data);
}

#[test]
fn test_cas_concurrent_distinct_writes() {
    let dir = tempdir().expect("tempdir");
    let store = Arc::new(ObjectStore::init(dir.path().join("objects")).expect("init store"));

    let mut handles = Vec::new();

    for thread_idx in 0..16 {
        let store_clone = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let mut thread_oids = Vec::new();
            for obj_idx in 0..50 {
                let content = format!("thread {} unique object {}", thread_idx, obj_idx);
                let oid = store_clone
                    .write_blob(content.as_bytes())
                    .expect("write distinct blob");
                thread_oids.push((oid, content));
            }
            thread_oids
        }));
    }

    let mut all_oids = Vec::new();
    for handle in handles {
        let thread_oids = handle.join().expect("thread join");
        all_oids.extend(thread_oids);
    }

    assert_eq!(all_oids.len(), 16 * 50);

    // Verify all objects can be read back cleanly
    for (oid, expected_content) in all_oids {
        let read = store.read_blob(&oid).expect("read blob");
        assert_eq!(read, expected_content.as_bytes());
    }
}
