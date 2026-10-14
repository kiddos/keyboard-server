use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_item::{IconSource, TrayItem};

pub struct SystemTray {
    _tray: TrayItem,
}

impl SystemTray {
    pub fn new(running: Arc<AtomicBool>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut tray = TrayItem::new("Keyboard Server", IconSource::Resource("keyboard-icon"))?;

        let running_clone = running.clone();
        tray.add_menu_item("Quit", move || {
            println!("Quit requested from tray icon.");
            running_clone.store(false, Ordering::SeqCst);
        })?;

        Ok(Self { _tray: tray })
    }
}
