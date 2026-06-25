use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldApp {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub last_opened_days: u64,
    pub recommendation: String,
}

pub fn find_old_apps(days_threshold: u64) -> Vec<OldApp> {
    #[cfg(target_os = "macos")]
    {
        let apps_dir = PathBuf::from("/Applications");
        let mut old_apps = Vec::new();
        let now = SystemTime::now();

        let entries = match fs::read_dir(&apps_dir) {
            Ok(e) => e,
            Err(_) => return old_apps,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().map(|e| e == "app").unwrap_or(false) {
                continue;
            }

            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let days = fs::metadata(&path)
                .ok()
                .and_then(|m| m.accessed().ok())
                .and_then(|a| now.duration_since(a).ok())
                .map(|d| d.as_secs() / 86400)
                .unwrap_or(0);

            if days < days_threshold {
                continue;
            }

            let size_bytes: u64 = walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
                .sum();

            let recommendation = if days > 365 {
                "Remove — not opened in over a year".to_string()
            } else if days > 180 {
                "Consider removing — unused for 6+ months".to_string()
            } else {
                "Monitor — not used recently".to_string()
            };

            old_apps.push(OldApp {
                name,
                path: path.to_string_lossy().to_string(),
                size_bytes,
                last_opened_days: days,
                recommendation,
            });
        }

        old_apps.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        old_apps
    }

    #[cfg(target_os = "windows")]
    {
        let mut old_apps = Vec::new();
        let now = SystemTime::now();

        let program_dirs = vec![
            PathBuf::from("C:\\Program Files"),
            PathBuf::from("C:\\Program Files (x86)"),
        ];

        for programs_dir in &program_dirs {
            let entries = match fs::read_dir(programs_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Skip Windows system directories
                if name.to_lowercase().contains("windows")
                    || name.to_lowercase().contains("common files")
                    || name.to_lowercase().contains("windowsapps")
                {
                    continue;
                }

                let days = fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|a| now.duration_since(a).ok())
                    .map(|d| d.as_secs() / 86400)
                    .unwrap_or(0);

                if days < days_threshold {
                    continue;
                }

                let size_bytes: u64 = walkdir::WalkDir::new(&path)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
                    .sum();

                let recommendation = if days > 365 {
                    "Remove — not modified in over a year".to_string()
                } else if days > 180 {
                    "Consider removing — unused for 6+ months".to_string()
                } else {
                    "Monitor — not used recently".to_string()
                };

                old_apps.push(OldApp {
                    name,
                    path: path.to_string_lossy().to_string(),
                    size_bytes,
                    last_opened_days: days,
                    recommendation,
                });
            }
        }

        old_apps.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        old_apps
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}
