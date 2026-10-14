mod db;
mod listener;
mod tray;

use db::{DbManager, KeyPressRecord};
use listener::start_keyboard_listener;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tray::SystemTray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = env::var("KEYBOARD_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("keypresses.db"));

    let flush_interval_secs: u64 = env::var("FLUSH_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    println!("Initializing Keyboard Server...");
    println!("Database path: {:?}", db_path);
    println!("Flush interval: {} seconds", flush_interval_secs);

    let mut db_conn = DbManager::init_db(&db_path)?;

    let running = Arc::new(AtomicBool::new(true));

    // Handle Ctrl+C / SIGINT
    let r = running.clone();
    let _ = ctrlc::set_handler(move || {
        println!("\nReceived shutdown signal...");
        r.store(false, Ordering::SeqCst);
    });

    let (tx, rx) = mpsc::channel::<KeyPressRecord>();

    // Start global keyboard listener
    let _listener_handle = start_keyboard_listener(tx);

    // Initialize system tray icon (with fallback if tray is unsupported in headless OS environment)
    let _tray = match SystemTray::new(running.clone()) {
        Ok(t) => {
            println!("System tray icon initialized.");
            Some(t)
        }
        Err(e) => {
            println!("System tray unavailable ({:?}), running in background mode.", e);
            None
        }
    };

    println!("Keyboard Server is running. Press Ctrl+C or use System Tray to quit.");

    let mut buffer: Vec<KeyPressRecord> = Vec::new();
    let mut last_flush = Instant::now();

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(200));

        // Drain pending keypress events from channel
        while let Ok(record) = rx.try_recv() {
            buffer.push(record);
        }

        // Flush buffer if interval passed or buffer is large
        if (last_flush.elapsed() >= Duration::from_secs(flush_interval_secs) || buffer.len() >= 100)
            && !buffer.is_empty()
        {
            match DbManager::flush_keypresses(&mut db_conn, &buffer) {
                Ok(count) => {
                    println!("Flushed {} keypresses to SQLite.", count);
                }
                Err(err) => {
                    eprintln!("Error flushing keypresses to SQLite: {:?}", err);
                }
            }
            buffer.clear();
            last_flush = Instant::now();
        }
    }

    // Shutdown cleanup: drain remaining records and do a final flush
    println!("Shutting down Keyboard Server...");
    while let Ok(record) = rx.try_recv() {
        buffer.push(record);
    }

    if !buffer.is_empty() {
        match DbManager::flush_keypresses(&mut db_conn, &buffer) {
            Ok(count) => {
                println!("Final flush: saved {} keypresses to SQLite.", count);
            }
            Err(err) => {
                eprintln!("Error during final SQLite flush: {:?}", err);
            }
        }
    }

    println!("Keyboard Server shut down cleanly.");
    Ok(())
}
