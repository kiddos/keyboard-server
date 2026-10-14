use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

#[path = "../src/db.rs"]
mod db;

use db::{DbManager, KeyPressRecord};

#[test]
fn test_sqlite_db_creation_and_indexing() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();

    let conn = DbManager::init_db(db_path).expect("Failed to initialize DB");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM keypresses", [], |row| row.get(0))
        .expect("Query failed");
    assert_eq!(count, 0);

    // Verify index exists
    let index_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_keypresses_timestamp'",
            [],
            |row| row.get(0),
        )
        .expect("Index query failed");
    assert_eq!(index_count, 1);
}

#[test]
fn test_keypress_batch_flushing() {
    let mut conn = DbManager::init_db(":memory:").expect("Failed to init in-memory DB");

    let records = vec![
        KeyPressRecord {
            timestamp: "2026-03-31T12:00:00Z".to_string(),
            key_name: "KeyA".to_string(),
            event_type: "KeyPress".to_string(),
        },
        KeyPressRecord {
            timestamp: "2026-03-31T12:00:01Z".to_string(),
            key_name: "KeyB".to_string(),
            event_type: "KeyPress".to_string(),
        },
        KeyPressRecord {
            timestamp: "2026-03-31T12:00:02Z".to_string(),
            key_name: "Return".to_string(),
            event_type: "KeyPress".to_string(),
        },
    ];

    let count = DbManager::flush_keypresses(&mut conn, &records).expect("Flush failed");
    assert_eq!(count, 3);

    // Verify records in DB
    let mut stmt = conn
        .prepare("SELECT timestamp, key_name, event_type FROM keypresses ORDER BY id ASC")
        .unwrap();
    let rows: Vec<KeyPressRecord> = stmt
        .query_map([], |row| {
            Ok(KeyPressRecord {
                timestamp: row.get(0)?,
                key_name: row.get(1)?,
                event_type: row.get(2)?,
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].key_name, "KeyA");
    assert_eq!(rows[1].key_name, "KeyB");
    assert_eq!(rows[2].key_name, "Return");
}

#[test]
fn test_mpsc_channel_queueing_and_flush() {
    let mut conn = DbManager::init_db(":memory:").expect("Failed to init DB");
    let (tx, rx) = mpsc::channel::<KeyPressRecord>();

    // Producer thread
    thread::spawn(move || {
        for i in 0..10 {
            tx.send(KeyPressRecord {
                timestamp: format!("2026-03-31T12:00:{:02}Z", i),
                key_name: format!("Key{}", i),
                event_type: "KeyPress".to_string(),
            })
            .unwrap();
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Receiver / Flush simulation
    thread::sleep(Duration::from_millis(150));
    let mut buffer = Vec::new();
    while let Ok(rec) = rx.try_recv() {
        buffer.push(rec);
    }

    assert_eq!(buffer.len(), 10);
    let count = DbManager::flush_keypresses(&mut conn, &buffer).unwrap();
    assert_eq!(count, 10);

    let db_count = DbManager::count_keypresses(&conn).unwrap();
    assert_eq!(db_count, 10);
}

#[test]
fn test_empty_flush_returns_zero() {
    let mut conn = DbManager::init_db(":memory:").unwrap();
    let empty: Vec<KeyPressRecord> = vec![];
    let count = DbManager::flush_keypresses(&mut conn, &empty).unwrap();
    assert_eq!(count, 0);
}
