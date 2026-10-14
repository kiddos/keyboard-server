use crate::db::KeyPressRecord;
use chrono::Utc;
use rdev::{listen, Event, EventType};
use std::sync::mpsc::Sender;
use std::thread;

pub fn start_keyboard_listener(tx: Sender<KeyPressRecord>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let callback = move |event: Event| {
            if let EventType::KeyPress(key) = event.event_type {
                let record = KeyPressRecord {
                    timestamp: Utc::now().to_rfc3339(),
                    key_name: format!("{:?}", key),
                    event_type: "KeyPress".to_string(),
                };
                let _ = tx.send(record);
            }
        };

        if let Err(error) = listen(callback) {
            eprintln!("Warning: Keyboard listener stopped or unavailable: {:?}", error);
        }
    })
}
