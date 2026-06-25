use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeFile {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub last_accessed_days: u64,
    pub file_type: String,
}

pub fn find_large_files(min_size_mb: u64) -> Vec<LargeFile> {
    let home = dirs::home_dir().unwrap_or_default();
    let min_bytes = min_size_mb * 1_048_576;
    let mut files = Vec::new();

    let scan_dirs = vec![
        home.join("Desktop"),
        home.join("Documents"),
        home.join("Downloads"),
        home.join("Movies"),
        home.join("Music"),
    ];

    let now = std::time::SystemTime::now();

    for dir in scan_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size < min_bytes {
                continue;
            }

            let days = entry
                .metadata()
                .ok()
                .and_then(|m| m.accessed().ok())
                .and_then(|a| now.duration_since(a).ok())
                .map(|d| d.as_secs() / 86400)
                .unwrap_or(0);

            let ext = entry
                .path()
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let file_type = match ext.as_str() {
                "mp4" | "mov" | "avi" | "mkv" => "Video",
                "dmg" | "iso" | "zip" | "tar" | "gz" => "Archive/Installer",
                "mp3" | "wav" | "flac" | "m4a" => "Audio",
                "psd" | "ai" | "sketch" => "Design",
                "vmdk" | "qcow2" | "vdi" => "Virtual Machine",
                _ => "Other",
            }
            .to_string();

            files.push(LargeFile {
                path: entry.path().to_string_lossy().to_string(),
                name: entry.file_name().to_string_lossy().to_string(),
                size_bytes: size,
                last_accessed_days: days,
                file_type,
            });
        }
    }

    files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    files.truncate(20);
    files
}
