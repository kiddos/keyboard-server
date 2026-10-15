use crate::db::{DbManager, MinuteStat};
use crate::tracker::KeyTracker;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: Option<String>,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[allow(dead_code)]
pub struct RpcClient {
    addr: String,
}

#[allow(dead_code)]
impl RpcClient {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub fn get_average_kpm(&self) -> Result<f64, Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect(&self.addr)?;
        let req = json!({
            "jsonrpc": "2.0",
            "method": "get_average_kpm",
            "id": 1
        });
        writeln!(stream, "{}", req)?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response_str = String::new();
        reader.read_line(&mut response_str)?;

        let resp: RpcResponse = serde_json::from_str(&response_str)?;
        if let Some(err) = resp.error {
            return Err(format!("RPC error: {}", err.message).into());
        }

        if let Some(result) = resp.result {
            if let Some(kpm) = result.get("kpm").and_then(|v| v.as_f64()) {
                return Ok(kpm);
            }
        }

        Err("Invalid RPC response structure".into())
    }

    pub fn print_average_kpm(&self) -> Result<(), Box<dyn std::error::Error>> {
        let kpm = self.get_average_kpm()?;
        println!("{}", json!({ "kpm": kpm }));
        Ok(())
    }
}

pub struct RpcServer {
    addr: String,
    db_path: PathBuf,
    tracker: KeyTracker,
    running: Arc<AtomicBool>,
}

impl RpcServer {
    pub fn new(
        addr: String,
        db_path: PathBuf,
        tracker: KeyTracker,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            addr,
            db_path,
            tracker,
            running,
        }
    }

    pub fn start(&self) -> thread::JoinHandle<()> {
        let addr = self.addr.clone();
        let db_path = self.db_path.clone();
        let tracker = self.tracker.clone();
        let running = self.running.clone();

        thread::spawn(move || {
            let listener = match TcpListener::bind(&addr) {
                Ok(l) => {
                    println!("RPC server listening on {}", addr);
                    l
                }
                Err(e) => {
                    eprintln!("Failed to bind RPC server on {}: {:?}", addr, e);
                    return;
                }
            };

            listener.set_nonblocking(true).ok();

            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _peer)) => {
                        let db_path_clone = db_path.clone();
                        let tracker_clone = tracker.clone();
                        thread::spawn(move || {
                            handle_client(stream, db_path_clone, tracker_clone);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("RPC accept error: {:?}", e);
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
            println!("RPC server stopped.");
        })
    }
}

fn handle_client(stream: TcpStream, db_path: PathBuf, tracker: KeyTracker) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;
    let mut line = String::new();

    while let Ok(bytes_read) = reader.read_line(&mut line) {
        if bytes_read == 0 {
            break;
        }

        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(req) => process_request(req, &db_path, &tracker),
            Err(e) => RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
                id: Value::Null,
            },
        };

        if let Ok(resp_json) = serde_json::to_string(&response) {
            let _ = writer.write_all(resp_json.as_bytes());
            let _ = writer.write_all(b"\n");
            let _ = writer.flush();
        }

        line.clear();
    }
}

fn process_request(req: RpcRequest, db_path: &PathBuf, tracker: &KeyTracker) -> RpcResponse {
    let id = req.id.unwrap_or(Value::Null);

    let conn_res = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    );

    match req.method.as_str() {
        "ping" => RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({"status": "pong"})),
            error: None,
            id,
        },
        "get_average_kpm" => match conn_res {
            Ok(conn) => match DbManager::get_average_kpm(&conn) {
                Ok(avg_kpm) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(json!({ "kpm": avg_kpm })),
                    error: None,
                    id,
                },
                Err(e) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32603,
                        message: format!("DB query error: {}", e),
                    }),
                    id,
                },
            },
            Err(e) => RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32603,
                    message: format!("DB open error: {}", e),
                }),
                id,
            },
        },
        "get_kpm" => RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({
                "kpm": tracker.get_kpm()
            })),
            error: None,
            id,
        },
        "get_total_count" => match conn_res {
            Ok(conn) => match DbManager::count_keypresses(&conn) {
                Ok(count) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(json!({ "total_keypresses": count })),
                    error: None,
                    id,
                },
                Err(e) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32603,
                        message: format!("DB query error: {}", e),
                    }),
                    id,
                },
            },
            Err(e) => RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(RpcError {
                    code: -32603,
                    message: format!("DB open error: {}", e),
                }),
                id,
            },
        },
        "get_keypresses_per_minute" => {
            let limit = req
                .params
                .as_ref()
                .and_then(|p| p.get("limit"))
                .and_then(|l| l.as_u64())
                .unwrap_or(10) as usize;

            match conn_res {
                Ok(conn) => match DbManager::get_keypresses_per_minute(&conn, limit) {
                    Ok(stats) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: Some(json!({ "minute_stats": stats })),
                        error: None,
                        id,
                    },
                    Err(e) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(RpcError {
                            code: -32603,
                            message: format!("DB query error: {}", e),
                        }),
                        id,
                    },
                },
                Err(e) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32603,
                        message: format!("DB open error: {}", e),
                    }),
                    id,
                },
            }
        }
        "get_keypress_count_since" => {
            let since_iso = req
                .params
                .as_ref()
                .and_then(|p| p.get("since"))
                .and_then(|s| s.as_str())
                .unwrap_or("");

            match conn_res {
                Ok(conn) => match DbManager::get_keypress_count_since(&conn, since_iso) {
                    Ok(count) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: Some(json!({ "count": count })),
                        error: None,
                        id,
                    },
                    Err(e) => RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(RpcError {
                            code: -32603,
                            message: format!("DB query error: {}", e),
                        }),
                        id,
                    },
                },
                Err(e) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: None,
                    error: Some(RpcError {
                        code: -32603,
                        message: format!("DB open error: {}", e),
                    }),
                    id,
                },
            }
        }
        "get_stats" => {
            let kpm = tracker.get_kpm();
            match conn_res {
                Ok(conn) => {
                    let total = DbManager::count_keypresses(&conn).unwrap_or(0);
                    let minute_stats =
                        DbManager::get_keypresses_per_minute(&conn, 10).unwrap_or_default();
                    RpcResponse {
                        jsonrpc: "2.0".to_string(),
                        result: Some(json!({
                            "kpm": kpm,
                            "total_keypresses": total,
                            "recent_minutes": minute_stats
                        })),
                        error: None,
                        id,
                    }
                }
                Err(_) => RpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(json!({
                        "kpm": kpm,
                        "total_keypresses": 0,
                        "recent_minutes": Vec::<MinuteStat>::new()
                    })),
                    error: None,
                    id,
                },
            }
        }
        _ => RpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
            }),
            id,
        },
    }
}
