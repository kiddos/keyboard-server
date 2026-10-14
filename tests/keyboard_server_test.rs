use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

#[path = "../src/cli.rs"]
mod cli;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/rpc.rs"]
mod rpc;
#[path = "../src/tracker.rs"]
mod tracker;

use db::{DbManager, KeyPressRecord};
use rpc::RpcServer;
use tracker::KeyTracker;

#[test]
fn test_cli_args_defaults() {
    use clap::Parser;
    let cli = cli::Cli::parse_from(&["keyboard-server"]);
    assert_eq!(cli.db_path.to_str().unwrap(), "keypresses.db");
    assert_eq!(cli.rpc_addr, "127.0.0.1:9090");
    assert_eq!(cli.flush_interval_secs, 5);
}

#[test]
fn test_cli_args_custom() {
    use clap::Parser;
    let cli = cli::Cli::parse_from(&[
        "keyboard-server",
        "--db-path",
        "/tmp/custom_key.db",
        "--rpc-addr",
        "127.0.0.1:8080",
        "--flush-interval",
        "10",
    ]);
    assert_eq!(cli.db_path.to_str().unwrap(), "/tmp/custom_key.db");
    assert_eq!(cli.rpc_addr, "127.0.0.1:8080");
    assert_eq!(cli.flush_interval_secs, 10);
}

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

#[test]
fn test_keypresses_per_minute_db_query() {
    let mut conn = DbManager::init_db(":memory:").expect("Failed to init DB");

    let records = vec![
        KeyPressRecord {
            timestamp: "2026-03-31T12:00:10Z".to_string(),
            key_name: "KeyA".to_string(),
            event_type: "KeyPress".to_string(),
        },
        KeyPressRecord {
            timestamp: "2026-03-31T12:00:20Z".to_string(),
            key_name: "KeyB".to_string(),
            event_type: "KeyPress".to_string(),
        },
        KeyPressRecord {
            timestamp: "2026-03-31T12:01:05Z".to_string(),
            key_name: "KeyC".to_string(),
            event_type: "KeyPress".to_string(),
        },
    ];

    DbManager::flush_keypresses(&mut conn, &records).unwrap();

    let minute_stats = DbManager::get_keypresses_per_minute(&conn, 10).unwrap();
    assert_eq!(minute_stats.len(), 2);
    // ordered desc
    assert_eq!(minute_stats[0].minute, "2026-03-31T12:01");
    assert_eq!(minute_stats[0].count, 1);
    assert_eq!(minute_stats[1].minute, "2026-03-31T12:00");
    assert_eq!(minute_stats[1].count, 2);
}

#[test]
fn test_rpc_server_communication() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_path_buf();

    // Prepare DB
    {
        let mut conn = DbManager::init_db(&db_path).unwrap();
        let records = vec![
            KeyPressRecord {
                timestamp: "2026-03-31T12:00:00Z".to_string(),
                key_name: "KeyA".to_string(),
                event_type: "KeyPress".to_string(),
            },
            KeyPressRecord {
                timestamp: "2026-03-31T12:00:30Z".to_string(),
                key_name: "KeyB".to_string(),
                event_type: "KeyPress".to_string(),
            },
        ];
        DbManager::flush_keypresses(&mut conn, &records).unwrap();
    }

    let tracker = KeyTracker::new();
    tracker.record_keypress();
    tracker.record_keypress();
    tracker.record_keypress();

    let rpc_addr = "127.0.0.1:19876";
    let running = Arc::new(AtomicBool::new(true));

    let server = RpcServer::new(
        rpc_addr.to_string(),
        db_path,
        tracker,
        running.clone(),
    );
    let _handle = server.start();

    // Give server time to bind
    thread::sleep(Duration::from_millis(100));

    // Connect via TcpStream
    let mut stream = TcpStream::connect(rpc_addr).expect("Failed to connect to RPC server");

    // Send ping request
    let req = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
    writeln!(stream, "{}", req).unwrap();
    stream.flush().unwrap();

    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    let json_resp: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(json_resp["result"]["status"], "pong");

    // Send get_kpm request
    let req_kpm = r#"{"jsonrpc":"2.0","method":"get_kpm","id":2}"#;
    writeln!(stream, "{}", req_kpm).unwrap();
    stream.flush().unwrap();

    response.clear();
    reader.read_line(&mut response).unwrap();
    let json_resp2: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(json_resp2["result"]["kpm"], 3);

    // Send get_total_count
    let req_total = r#"{"jsonrpc":"2.0","method":"get_total_count","id":3}"#;
    writeln!(stream, "{}", req_total).unwrap();
    stream.flush().unwrap();

    response.clear();
    reader.read_line(&mut response).unwrap();
    let json_resp3: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(json_resp3["result"]["total_keypresses"], 2);

    running.store(false, Ordering::SeqCst);
}
