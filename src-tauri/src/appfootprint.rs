use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppFootprint {
    pub name: String,
    pub app_size: u64,
    pub caches_size: u64,
    pub support_size: u64,
    pub logs_size: u64,
    pub containers_size: u64,
    pub other_size: u64,
    pub total_size: u64,
    pub locations: Vec<AppLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLocation {
    pub path: String,
    pub size_bytes: u64,
    pub category: String, // "App Bundle" | "Cache" | "Application Support" | "Logs" | "Container"
}

/// Analyze total disk footprint of installed applications
pub fn analyze_footprints() -> Vec<AppFootprint> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));
    let mut footprints: Vec<AppFootprint> = Vec::new();
    
    // Get list of installed apps
    let apps_dir = PathBuf::from("/Applications");
    let entries = match fs::read_dir(&apps_dir) {
        Ok(e) => e,
        Err(_) => return footprints,
    };
    
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().map(|e| e == "app").unwrap_or(false) {
            continue;
        }
        
        let app_name = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let app_name_lower = app_name.to_lowercase();
        let app_size = dir_size(&path);
        
        // Search for related data in Library
        let mut caches_size = 0u64;
        let mut support_size = 0u64;
        let mut logs_size = 0u64;
        let mut containers_size = 0u64;
        let other_size = 0u64;
        let mut locations: Vec<AppLocation> = vec![];
        
        // Add app bundle itself
        locations.push(AppLocation {
            path: path.to_string_lossy().to_string(),
            size_bytes: app_size,
            category: "App Bundle".to_string(),
        });
        
        // Check Caches
        let caches_dir = home.join("Library/Caches");
        if let Ok(entries) = fs::read_dir(&caches_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains(&app_name_lower) || matches_bundle_id(&name, &app_name_lower) {
                    let size = dir_size(&e.path());
                    if size > 0 {
                        caches_size += size;
                        locations.push(AppLocation {
                            path: e.path().to_string_lossy().to_string(),
                            size_bytes: size,
                            category: "Cache".to_string(),
                        });
                    }
                }
            }
        }
        
        // Check Application Support
        let support_dir = home.join("Library/Application Support");
        if let Ok(entries) = fs::read_dir(&support_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains(&app_name_lower) || matches_bundle_id(&name, &app_name_lower) {
                    let size = dir_size(&e.path());
                    if size > 0 {
                        support_size += size;
                        locations.push(AppLocation {
                            path: e.path().to_string_lossy().to_string(),
                            size_bytes: size,
                            category: "Application Support".to_string(),
                        });
                    }
                }
            }
        }
        
        // Check Logs
        let logs_dir = home.join("Library/Logs");
        if let Ok(entries) = fs::read_dir(&logs_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains(&app_name_lower) {
                    let size = dir_size(&e.path());
                    if size > 0 {
                        logs_size += size;
                        locations.push(AppLocation {
                            path: e.path().to_string_lossy().to_string(),
                            size_bytes: size,
                            category: "Logs".to_string(),
                        });
                    }
                }
            }
        }
        
        // Check Containers
        let containers_dir = home.join("Library/Containers");
        if let Ok(entries) = fs::read_dir(&containers_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.contains(&app_name_lower) || matches_bundle_id(&name, &app_name_lower) {
                    let size = dir_size(&e.path());
                    if size > 0 {
                        containers_size += size;
                        locations.push(AppLocation {
                            path: e.path().to_string_lossy().to_string(),
                            size_bytes: size,
                            category: "Container".to_string(),
                        });
                    }
                }
            }
        }
        
        let total_size = app_size + caches_size + support_size + logs_size + containers_size + other_size;
        
        // Only include apps with significant footprint (> 100MB total)
        if total_size > 100_000_000 {
            footprints.push(AppFootprint {
                name: app_name,
                app_size,
                caches_size,
                support_size,
                logs_size,
                containers_size,
                other_size,
                total_size,
                locations,
            });
        }
    }
    
    footprints.sort_by(|a, b| b.total_size.cmp(&a.total_size));
    footprints
}

/// Check if a bundle ID matches an app name
fn matches_bundle_id(bundle_id: &str, app_name: &str) -> bool {
    // e.g., "com.docker.docker" matches "docker"
    // "com.google.chrome" matches "chrome"
    let parts: Vec<&str> = bundle_id.split('.').collect();
    parts.iter().any(|part| part.contains(app_name))
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .max_depth(20)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
