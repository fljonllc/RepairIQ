use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSnapshot {
    pub date: String,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageForecast {
    pub history: Vec<StorageSnapshot>,
    pub days_until_full: Option<u64>,
    pub daily_growth_bytes: i64,
    pub weekly_growth_bytes: i64,
    pub total_cleaned_bytes: u64,
    pub total_cleaned_count: u32,
}

fn db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".repairiq/history.db")
}

/// Initialize the history database
pub fn init_history() -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = home.join(".repairiq");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS storage_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            used_bytes INTEGER NOT NULL,
            free_bytes INTEGER NOT NULL,
            total_bytes INTEGER NOT NULL
        )", []
    ).map_err(|e| format!("Failed to create table: {}", e))?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS clean_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            bytes_freed INTEGER NOT NULL,
            items_cleaned INTEGER NOT NULL
        )", []
    ).map_err(|e| format!("Failed to create table: {}", e))?;
    
    Ok(())
}

/// Record current storage state
pub fn record_snapshot(used_bytes: u64, free_bytes: u64, total_bytes: u64) -> Result<(), String> {
    let _ = init_history();
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("DB error: {}", e))?;
    
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO storage_history (date, used_bytes, free_bytes, total_bytes) VALUES (?1, ?2, ?3, ?4)",
        params![now, used_bytes, free_bytes, total_bytes]
    ).map_err(|e| format!("Insert error: {}", e))?;
    
    Ok(())
}

/// Record a cleaning action
pub fn record_clean(bytes_freed: u64, items_cleaned: u32) -> Result<(), String> {
    let _ = init_history();
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("DB error: {}", e))?;
    
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO clean_history (date, bytes_freed, items_cleaned) VALUES (?1, ?2, ?3)",
        params![now, bytes_freed, items_cleaned]
    ).map_err(|e| format!("Insert error: {}", e))?;
    
    Ok(())
}

/// Get storage forecast
pub fn get_forecast() -> StorageForecast {
    let _ = init_history();
    let db = db_path();
    
    let conn = match Connection::open(&db) {
        Ok(c) => c,
        Err(_) => return StorageForecast {
            history: vec![],
            days_until_full: None,
            daily_growth_bytes: 0,
            weekly_growth_bytes: 0,
            total_cleaned_bytes: 0,
            total_cleaned_count: 0,
        },
    };
    
    // Get last 30 snapshots
    let mut stmt = match conn.prepare(
        "SELECT date, used_bytes, free_bytes, total_bytes FROM storage_history ORDER BY date DESC LIMIT 30"
    ) {
        Ok(s) => s,
        Err(_) => return StorageForecast {
            history: vec![],
            days_until_full: None,
            daily_growth_bytes: 0,
            weekly_growth_bytes: 0,
            total_cleaned_bytes: 0,
            total_cleaned_count: 0,
        },
    };
    
    let history: Vec<StorageSnapshot> = match stmt.query_map([], |row| {
        Ok(StorageSnapshot {
            date: row.get(0)?,
            used_bytes: row.get(1)?,
            free_bytes: row.get(2)?,
            total_bytes: row.get(3)?,
        })
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => Vec::new(),
    };
    
    // Calculate growth rate
    let (daily_growth, days_until_full) = if history.len() >= 2 {
        let newest = &history[0];
        let oldest = &history[history.len() - 1];
        let days_between = history.len() as i64; // approximate
        let growth = newest.used_bytes as i64 - oldest.used_bytes as i64;
        let daily = if days_between > 0 { growth / days_between } else { 0 };
        
        let days_full = if daily > 0 && newest.free_bytes > 0 {
            Some(newest.free_bytes / daily as u64)
        } else {
            None
        };
        
        (daily, days_full)
    } else {
        (0i64, None)
    };
    
    // Get total cleaned
    let (total_cleaned_bytes, total_cleaned_count) = conn.query_row(
        "SELECT COALESCE(SUM(bytes_freed), 0), COALESCE(SUM(items_cleaned), 0) FROM clean_history",
        [],
        |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?))
    ).unwrap_or((0, 0));
    
    StorageForecast {
        history,
        days_until_full,
        daily_growth_bytes: daily_growth,
        weekly_growth_bytes: daily_growth * 7,
        total_cleaned_bytes,
        total_cleaned_count,
    }
}
