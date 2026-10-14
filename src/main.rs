mod cli;
mod db;
mod listener;
mod rpc;
mod tracker;
mod tray;

use cli::parse_args;
use db::{DbManager, KeyPressRecord};
use listener::start_keyboard_listener;
use rpc::RpcServer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracker::KeyTracker;
use tray::SystemTray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let db_path = args.db_path;
    let rpc_addr = args.rpc_addr;
    let flush_interval_secs = args.flush_interval_secs;

    println!("Initializing Keyboard Server...");
    println!("Database path: {:?}", db_path);
    println!("RPC address: {}", rpc_addr);
    println!("Flush interval: {} seconds", flush_interval_secs);

    let mut db_conn = DbManager::init_db(&db_path)?;

    let running = Arc::new(AtomicBool::new(true));

    // Handle Ctrl+C / SIGINT
    let r = running.clone();
    let _ = ctrlc::set_handler(move || {
        println!("\nReceived shutdown signal...");
        r.store(false, Ordering::SeqCst);
    });

    let key_tracker = KeyTracker::new();

    // Start RPC server
    let rpc_server = RpcServer::new(
        rpc_addr,
        db_path.clone(),
        key_tracker.clone(),
        running.clone(),
    );
    let _rpc_handle = rpc_server.start();

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
            key_tracker.record_keypress();
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
        key_tracker.record_keypress();
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
