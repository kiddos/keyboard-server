use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPressRecord {
    pub timestamp: String,
    pub key_name: String,
    pub event_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinuteStat {
    pub minute: String,
    pub count: i64,
}

pub struct DbManager;

impl DbManager {
    pub fn init_db<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS keypresses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                key_name TEXT NOT NULL,
                event_type TEXT NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_keypresses_timestamp ON keypresses(timestamp);",
            [],
        )?;

        Ok(conn)
    }

    pub fn flush_keypresses(
        conn: &mut Connection,
        records: &[KeyPressRecord],
    ) -> Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }

        let tx = conn.transaction()?;
        let mut count = 0;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO keypresses (timestamp, key_name, event_type) VALUES (?1, ?2, ?3)",
            )?;

            for record in records {
                stmt.execute(params![record.timestamp, record.key_name, record.event_type])?;
                count += 1;
            }
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn count_keypresses(conn: &Connection) -> Result<i64> {
        conn.query_row("SELECT COUNT(*) FROM keypresses", [], |row| row.get(0))
    }

    pub fn get_keypresses_per_minute(conn: &Connection, limit: usize) -> Result<Vec<MinuteStat>> {
        let mut stmt = conn.prepare(
            "SELECT substr(timestamp, 1, 16) AS minute, COUNT(*) AS count
             FROM keypresses
             GROUP BY minute
             ORDER BY minute DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(MinuteStat {
                minute: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn get_keypress_count_since(conn: &Connection, since_iso: &str) -> Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM keypresses WHERE timestamp >= ?1",
            params![since_iso],
            |row| row.get(0),
        )
    }

    pub fn get_average_kpm(conn: &Connection) -> Result<f64> {
        // Average keypresses per minute across all distinct minutes recorded in DB
        let mut stmt = conn.prepare(
            "SELECT AVG(minute_count) FROM (
                SELECT COUNT(*) as minute_count
                FROM keypresses
                GROUP BY substr(timestamp, 1, 16)
            )",
        )?;
        let avg: Option<f64> = stmt.query_row([], |row| row.get(0))?;
        Ok(avg.unwrap_or(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_init_and_flush() {
        let mut conn = DbManager::init_db(":memory:").unwrap();
        assert_eq!(DbManager::count_keypresses(&conn).unwrap(), 0);

        let records = vec![
            KeyPressRecord {
                timestamp: "2026-03-31T10:00:00Z".to_string(),
                key_name: "KeyA".to_string(),
                event_type: "KeyPress".to_string(),
            },
            KeyPressRecord {
                timestamp: "2026-03-31T10:00:01Z".to_string(),
                key_name: "Space".to_string(),
                event_type: "KeyPress".to_string(),
            },
        ];

        let flushed = DbManager::flush_keypresses(&mut conn, &records).unwrap();
        assert_eq!(flushed, 2);
        assert_eq!(DbManager::count_keypresses(&conn).unwrap(), 2);
    }
}
