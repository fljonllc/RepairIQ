use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Recovery Vault — nothing is permanently deleted, items are moved here first
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultItem {
    pub id: i64,
    pub original_path: String,
    pub vault_path: String,
    pub name: String,
    pub size_bytes: u64,
    pub moved_at: String,
    pub expires_at: String,
    pub category: String,
}

/// Vault retention period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetentionDays {
    Seven = 7,
    Fourteen = 14,
    Thirty = 30,
}

/// Get the vault base directory
fn vault_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".repairiq/vault")
}

/// Get the database path
fn db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".repairiq/vault.db")
}

/// Initialize the vault database
pub fn init_vault() -> Result<(), String> {
    let vault = vault_dir();
    fs::create_dir_all(&vault).map_err(|e| format!("Failed to create vault dir: {}", e))?;

    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS vault_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            original_path TEXT NOT NULL,
            vault_path TEXT NOT NULL,
            name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            moved_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            category TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create table: {}", e))?;

    Ok(())
}

/// Move an item to the recovery vault
pub fn move_to_vault(
    original_path: &str,
    retention_days: u32,
    category: &str,
) -> Result<VaultItem, String> {
    let source = Path::new(original_path);
    if !source.exists() {
        return Err(format!("Source path does not exist: {}", original_path));
    }

    let name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Get size BEFORE moving (so we don't have to walk the tree after)
    let size_bytes = if source.is_dir() {
        dir_size_simple(source)
    } else {
        fs::metadata(source).map(|m| m.len()).unwrap_or(0)
    };

    let now: DateTime<Utc> = Utc::now();
    let expires = now + chrono::Duration::days(retention_days as i64);

    // Create a unique vault path
    let timestamp = now.format("%Y%m%d_%H%M%S").to_string();
    let vault_subdir = vault_dir().join(format!("{}_{}", timestamp, &name));

    // Move the file/directory
    fs::create_dir_all(vault_subdir.parent().unwrap_or(&vault_dir()))
        .map_err(|e| format!("Failed to create vault subdir: {}", e))?;

    // Use rename for same-filesystem moves (instant), copy+delete for cross-filesystem
    if fs::rename(source, &vault_subdir).is_err() {
        // Cross-filesystem: copy then delete
        if source.is_dir() {
            copy_dir_recursive(source, &vault_subdir)?;
        } else {
            fs::copy(source, &vault_subdir)
                .map_err(|e| format!("Failed to copy to vault: {}", e))?;
        }
        if source.is_dir() {
            fs::remove_dir_all(source)
                .map_err(|e| format!("Failed to remove original dir: {}", e))?;
        } else {
            fs::remove_file(source)
                .map_err(|e| format!("Failed to remove original file: {}", e))?;
        }
    }

    // Record in database
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    conn.execute(
        "INSERT INTO vault_items (original_path, vault_path, name, size_bytes, moved_at, expires_at, category)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            original_path,
            vault_subdir.to_string_lossy().to_string(),
            name,
            size_bytes,
            now.to_rfc3339(),
            expires.to_rfc3339(),
            category,
        ],
    )
    .map_err(|e| format!("Failed to insert vault record: {}", e))?;

    let id = conn.last_insert_rowid();

    Ok(VaultItem {
        id,
        original_path: original_path.to_string(),
        vault_path: vault_subdir.to_string_lossy().to_string(),
        name,
        size_bytes,
        moved_at: now.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        category: category.to_string(),
    })
}

/// Restore an item from the vault
pub fn restore_from_vault(id: i64) -> Result<(), String> {
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    let item: (String, String) = conn
        .query_row(
            "SELECT original_path, vault_path FROM vault_items WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Vault item not found: {}", e))?;

    let (original_path, vault_path) = item;
    let source = Path::new(&vault_path);
    let dest = Path::new(&original_path);

    if !source.exists() {
        return Err("Vault file no longer exists".to_string());
    }

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dir: {}", e))?;
    }

    fs::rename(source, dest).map_err(|e| format!("Failed to restore: {}", e))?;

    // Remove from database
    conn.execute("DELETE FROM vault_items WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to remove vault record: {}", e))?;

    Ok(())
}

/// List all items in the vault
pub fn list_vault_items() -> Result<Vec<VaultItem>, String> {
    let db = db_path();

    if !db.exists() {
        return Ok(Vec::new());
    }

    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, original_path, vault_path, name, size_bytes, moved_at, expires_at, category
             FROM vault_items ORDER BY moved_at DESC",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let items = stmt
        .query_map([], |row| {
            Ok(VaultItem {
                id: row.get(0)?,
                original_path: row.get(1)?,
                vault_path: row.get(2)?,
                name: row.get(3)?,
                size_bytes: row.get(4)?,
                moved_at: row.get(5)?,
                expires_at: row.get(6)?,
                category: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to query vault: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(items)
}

/// Purge expired items from the vault
pub fn purge_expired() -> Result<u64, String> {
    let db = db_path();
    if !db.exists() {
        return Ok(0);
    }

    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;
    let now = Utc::now().to_rfc3339();

    // Get expired items
    let mut stmt = conn
        .prepare("SELECT vault_path FROM vault_items WHERE expires_at < ?1")
        .map_err(|e| format!("Failed to prepare: {}", e))?;

    let paths: Vec<String> = stmt
        .query_map(params![now], |row| row.get(0))
        .map_err(|e| format!("Failed to query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut freed: u64 = 0;
    for vault_path in &paths {
        let p = Path::new(vault_path);
        if p.exists() {
            let size = if p.is_dir() {
                dir_size_simple(p)
            } else {
                fs::metadata(p).map(|m| m.len()).unwrap_or(0)
            };
            freed += size;

            if p.is_dir() {
                let _ = fs::remove_dir_all(p);
            } else {
                let _ = fs::remove_file(p);
            }
        }
    }

    conn.execute("DELETE FROM vault_items WHERE expires_at < ?1", params![now])
        .map_err(|e| format!("Failed to delete expired: {}", e))?;

    Ok(freed)
}

/// Permanently delete ALL items in the vault — reclaim all space now
pub fn purge_all() -> Result<u64, String> {
    let db = db_path();
    if !db.exists() {
        return Ok(0);
    }

    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    // Get ALL vault items
    let mut stmt = conn
        .prepare("SELECT vault_path, size_bytes FROM vault_items")
        .map_err(|e| format!("Failed to prepare: {}", e))?;

    let items: Vec<(String, u64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("Failed to query: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut freed: u64 = 0;
    for (vault_path, size) in &items {
        let p = Path::new(vault_path.as_str());
        if p.exists() {
            if p.is_dir() {
                let _ = fs::remove_dir_all(p);
            } else {
                let _ = fs::remove_file(p);
            }
            freed += size;
        } else {
            freed += size; // Count it as freed even if file already gone
        }
    }

    // Clear the entire table
    conn.execute("DELETE FROM vault_items", [])
        .map_err(|e| format!("Failed to clear vault: {}", e))?;

    Ok(freed)
}

/// Permanently delete a single vault item by ID
pub fn delete_permanently(id: i64) -> Result<u64, String> {
    let db = db_path();
    let conn = Connection::open(&db).map_err(|e| format!("Failed to open DB: {}", e))?;

    let (vault_path, size_bytes): (String, u64) = conn
        .query_row(
            "SELECT vault_path, size_bytes FROM vault_items WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Vault item not found: {}", e))?;

    // Delete the actual files
    let p = Path::new(&vault_path);
    if p.exists() {
        if p.is_dir() {
            fs::remove_dir_all(p).map_err(|e| format!("Failed to delete: {}", e))?;
        } else {
            fs::remove_file(p).map_err(|e| format!("Failed to delete: {}", e))?;
        }
    }

    // Remove from database
    conn.execute("DELETE FROM vault_items WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to remove record: {}", e))?;

    Ok(size_bytes)
}

// Helper: recursive directory copy
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("mkdir failed: {}", e))?;

    for entry in fs::read_dir(src).map_err(|e| format!("readdir failed: {}", e))? {
        let entry = entry.map_err(|e| format!("entry error: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| format!("copy failed: {}", e))?;
        }
    }
    Ok(())
}

// Helper: simple directory size
fn dir_size_simple(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
