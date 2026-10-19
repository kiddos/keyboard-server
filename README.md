# Keyboard Server

`keyboard-server` is a cross-platform keyboard tracking server written in Rust. It captures global keypress events, tracks real-time keys-per-minute (KPM), logs activity to an SQLite database, provides a JSON-RPC interface for remote queries, and includes a system tray application icon.

## Features

- **Global Key Tracking**: Hooks system keyboard events using `rdev`.
- **SQLite Persistence**: Buffers keypresses and flushes them efficiently to an SQLite database with indexed timestamp logging.
- **Real-Time KPM Tracking**: Maintains a sliding-window counter to calculate current keys-per-minute.
- **JSON-RPC Server**: Exposes TCP JSON-RPC endpoints to query server status, total keypress counts, real-time KPM, and historical minute statistics.
- **System Tray Support**: System tray integration (with graceful fallback in headless or unsupported environments).
- **CLI & RPC Client Mode**: Subcommands to query running server stats directly from the command line.

---

## System Prerequisites

On Linux (Ubuntu/Debian), build dependencies for X11 and DBus are required:

```bash
sudo apt-get update
sudo apt-get install -y libdbus-1-dev pkg-config libxi-dev libx11-dev libxtst-dev
```

---

## Building and Installation

### Building with Cargo

```bash
cargo build --release
```

### Building and Installing with Make

```bash
# Build release binary
make build

# Install binary to ~/.local/bin (default PREFIX)
make install
```

To install to a custom path:

```bash
PREFIX=/usr/local/bin make install
```

---

## Running the Server

Start the server using default options (`keypresses.db`, RPC address `127.0.0.1:9090`, flush interval `5s`):

```bash
./target/release/keyboard-server
```

### Command-Line Arguments & Environment Variables

| Argument | Long Flag | Environment Variable | Default | Description |
|---|---|---|---|---|
| `-d` | `--db-path` | `KEYBOARD_DB_PATH` | `keypresses.db` | Path to the SQLite database file |
| `-r` | `--rpc-addr` | `KEYBOARD_RPC_ADDR` | `127.0.0.1:9090` | Host and port for JSON-RPC server/client |
| `-f` | `--flush-interval` | `FLUSH_INTERVAL_SECS` | `5` | Time interval in seconds to flush buffered keypresses |

Example with custom flags:

```bash
keyboard-server --db-path /var/log/keys.db --rpc-addr 127.0.0.1:9191 --flush-interval 10
```

---

## CLI Commands

### Querying Average KPM

You can query the running server's average KPM via the built-in RPC client subcommand:

```bash
keyboard-server get-average-kpm
```

Example output:
```json
{"kpm": 45.2}
```

---

## JSON-RPC API

The server listens on a TCP socket using line-delimited JSON-RPC 2.0 objects.

### Available RPC Methods

1. **`ping`**
   - **Request**: `{"jsonrpc": "2.0", "method": "ping", "id": 1}`
   - **Response**: `{"jsonrpc": "2.0", "result": {"status": "pong"}, "id": 1}`

2. **`get_kpm`**
   - Returns current real-time KPM calculated over the last 60 seconds.
   - **Request**: `{"jsonrpc": "2.0", "method": "get_kpm", "id": 1}`

3. **`get_total_count`**
   - Returns the total keypress record count stored in the SQLite database.
   - **Request**: `{"jsonrpc": "2.0", "method": "get_total_count", "id": 1}`

4. **`get_average_kpm`**
   - Returns average keypresses per minute across all distinct recorded minutes in the database.
   - **Request**: `{"jsonrpc": "2.0", "method": "get_average_kpm", "id": 1}`

5. **`get_keypresses_per_minute`**
   - Returns keypress counts grouped by minute.
   - **Params**: `{"limit": 10}` (optional, default 10)
   - **Request**: `{"jsonrpc": "2.0", "method": "get_keypresses_per_minute", "params": {"limit": 5}, "id": 1}`

6. **`get_keypress_count_since`**
   - Returns total keypress count recorded since a given ISO 8601 timestamp string.
   - **Request**: `{"jsonrpc": "2.0", "method": "get_keypress_count_since", "params": {"since": "2026-03-31T00:00:00Z"}, "id": 1}`

7. **`get_stats`**
   - Returns aggregated statistics (real-time KPM, total count, and recent minute stats).
   - **Request**: `{"jsonrpc": "2.0", "method": "get_stats", "id": 1}`

---

## Running Tests

Run test suite with Cargo or Make:

```bash
cargo test
# or
make test
```

---

## License

Refer to repository settings or project distribution guidelines.
