mod appfootprint;
mod archive;
mod browser;
mod duplicates;
mod forecast;
mod scanner;
mod timemachine;
mod vault;

use appfootprint::AppFootprint;
use archive::ArchiveRecommendation;
use browser::BrowserCache;
use duplicates::DuplicateGroup;
use forecast::StorageForecast;
use scanner::{ScanResult, ScannedItem};
use tauri::Emitter;
use timemachine::TimeMachineInfo;
use vault::VaultItem;

/// Perform a full storage scan
#[tauri::command]
fn scan_storage() -> Result<ScanResult, String> {
    Ok(scanner::perform_scan())
}

/// Perform a full storage scan with progress streaming
#[tauri::command]
fn scan_storage_with_progress(app: tauri::AppHandle) -> Result<ScanResult, String> {
    Ok(scanner::perform_scan_with_progress(move |msg| {
        let _ = app.emit("scan-progress", msg);
    }))
}

/// Initialize the recovery vault
#[tauri::command]
fn init_vault() -> Result<(), String> {
    vault::init_vault()
}

/// Move an item to the recovery vault
#[tauri::command]
fn vault_move(path: String, retention_days: u32, category: String) -> Result<VaultItem, String> {
    vault::move_to_vault(&path, retention_days, &category)
}

/// Restore an item from the vault
#[tauri::command]
fn vault_restore(id: i64) -> Result<(), String> {
    vault::restore_from_vault(id)
}

/// List all vault items
#[tauri::command]
fn vault_list() -> Result<Vec<VaultItem>, String> {
    vault::list_vault_items()
}

/// Purge expired vault items
#[tauri::command]
fn vault_purge() -> Result<u64, String> {
    vault::purge_expired()
}

/// Get a single item's details for the visual explorer
#[tauri::command]
fn get_item_children(path: String) -> Result<Vec<ScannedItem>, String> {
    use std::fs;
    use std::path::Path;
    use walkdir::WalkDir;

    let base = Path::new(&path);
    if !base.exists() {
        return Err("Path does not exist".to_string());
    }

    let mut items: Vec<ScannedItem> = Vec::new();

    let entries = fs::read_dir(base).map_err(|e| format!("Cannot read directory: {}", e))?;

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let name = entry_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let size_bytes = if entry_path.is_dir() {
            WalkDir::new(&entry_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
                .sum()
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };

        // Skip tiny items
        if size_bytes < 1_048_576 {
            continue;
        }

        let (safety, safety_score, description, impact, recovery_method, owner, verdict, verdict_reason, depends_on, clean_command, recommendation, action_label, risk_level, time_to_rebuild, side_effects, why_here, reasoning, confidence, evidence, why_recommended, what_if_wrong) =
            scanner::get_item_intel(&entry_path, "Drill-down");

        items.push(ScannedItem {
            path: entry_path.to_string_lossy().to_string(),
            name,
            size_bytes,
            category: "Drill-down".to_string(),
            subcategory: path.clone(),
            safety,
            safety_score,
            last_accessed_days: None,
            description,
            impact,
            recovery_method,
            owner,
            verdict,
            verdict_reason,
            file_count: 0,
            largest_files: vec![],
            depends_on,
            clean_command,
            recommendation,
            action_label,
            risk_level,
            time_to_rebuild,
            side_effects,
            why_here,
            reasoning,
            confidence,
            evidence,
            why_recommended,
            what_if_wrong,
        });
    }

    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    Ok(items)
}

/// Find projects that should be archived
#[tauri::command]
fn find_archive_candidates() -> Vec<ArchiveRecommendation> {
    archive::find_archive_candidates()
}

/// List external volumes for archiving
#[tauri::command]
fn list_volumes() -> Vec<String> {
    archive::list_external_volumes()
}

/// Archive a project to an external drive (copies + verifies before suggesting removal)
#[tauri::command]
fn archive_project(source_path: String, destination_dir: String) -> Result<String, String> {
    archive::archive_project(&source_path, &destination_dir)
}

#[tauri::command]
fn detect_duplicates() -> Vec<DuplicateGroup> {
    duplicates::find_duplicates()
}

#[tauri::command]
fn detect_browser_caches() -> Vec<BrowserCache> {
    browser::detect_browser_caches()
}

#[tauri::command]
fn detect_time_machine() -> TimeMachineInfo {
    timemachine::detect_snapshots()
}

#[tauri::command]
fn get_storage_forecast() -> StorageForecast {
    forecast::get_forecast()
}

#[tauri::command]
fn record_storage_snapshot(used_bytes: u64, free_bytes: u64, total_bytes: u64) -> Result<(), String> {
    forecast::record_snapshot(used_bytes, free_bytes, total_bytes)
}

#[tauri::command]
fn record_clean_action(bytes_freed: u64, items_cleaned: u32) -> Result<(), String> {
    forecast::record_clean(bytes_freed, items_cleaned)
}

#[tauri::command]
fn analyze_app_footprints() -> Vec<AppFootprint> {
    appfootprint::analyze_footprints()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_storage,
            scan_storage_with_progress,
            init_vault,
            vault_move,
            vault_restore,
            vault_list,
            vault_purge,
            get_item_children,
            find_archive_candidates,
            list_volumes,
            archive_project,
            detect_duplicates,
            detect_browser_caches,
            detect_time_machine,
            get_storage_forecast,
            record_storage_snapshot,
            record_clean_action,
            analyze_app_footprints,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
