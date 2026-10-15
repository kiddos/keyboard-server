use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Keyboard tracking server with SQLite persistence and RPC support")]
pub struct Cli {
    /// Path to SQLite database file where keypresses are saved
    #[arg(short = 'd', long = "db-path", env = "KEYBOARD_DB_PATH", default_value = "keypresses.db")]
    pub db_path: PathBuf,

    /// Address for the RPC server to listen on or client to connect to (e.g. 127.0.0.1:9090)
    #[arg(short = 'r', long = "rpc-addr", env = "KEYBOARD_RPC_ADDR", default_value = "127.0.0.1:9090")]
    pub rpc_addr: String,

    /// Interval in seconds for flushing buffered keypresses to SQLite
    #[arg(short = 'f', long = "flush-interval", env = "FLUSH_INTERVAL_SECS", default_value_t = 5)]
    pub flush_interval_secs: u64,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Query average KPM from the server via RPC
    GetAverageKpm,
}

#[allow(dead_code)]
pub fn parse_args() -> Cli {
    Cli::parse()
}
