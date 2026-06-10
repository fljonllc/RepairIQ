use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// An archive recommendation for old/inactive projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRecommendation {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub last_opened_days: u64,
    pub project_type: String,
    pub status: String,
}

/// Detected project types and their marker files
const PROJECT_MARKERS: &[(&str, &[&str])] = &[
    ("Node.js", &["package.json"]),
    ("Rust", &["Cargo.toml"]),
    ("Python", &["setup.py", "pyproject.toml", "requirements.txt"]),
    ("Swift/Xcode", &["Package.swift", "*.xcodeproj", "*.xcworkspace"]),
    ("Go", &["go.mod"]),
    ("Java/Kotlin", &["pom.xml", "build.gradle", "build.gradle.kts"]),
    ("Ruby", &["Gemfile"]),
    ("Flutter/Dart", &["pubspec.yaml"]),
    ("C/C++", &["CMakeLists.txt", "Makefile"]),
    ("Unity", &["ProjectSettings"]),
];

/// Detect if a directory is a project and what type
fn detect_project_type(path: &Path) -> Option<String> {
    let entries: Vec<String> = fs::read_dir(path)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    for (proj_type, markers) in PROJECT_MARKERS {
        for marker in *markers {
            if marker.starts_with('*') {
                // Wildcard match on extension
                let ext = &marker[1..];
                if entries.iter().any(|e| e.ends_with(ext)) {
                    return Some(proj_type.to_string());
                }
            } else if entries.iter().any(|e| e == marker) {
                return Some(proj_type.to_string());
            }
        }
    }
    None
}

/// Get the most recent modification time in a directory tree (sampled)
fn most_recent_access(path: &Path) -> Option<u64> {
    let mut most_recent: Option<SystemTime> = None;

    // Sample first 100 files for performance
    let mut count = 0;
    for entry in WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if count >= 100 {
            break;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                most_recent = Some(match most_recent {
                    Some(prev) => prev.max(modified),
                    None => modified,
                });
            }
        }
        count += 1;
    }

    let now = SystemTime::now();
    most_recent.and_then(|t| now.duration_since(t).ok()).map(|d| d.as_secs() / 86400)
}

/// Get directory size
fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Scan common project locations for archive candidates
pub fn find_archive_candidates() -> Vec<ArchiveRecommendation> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));

    // Common places developers keep projects
    let search_dirs = vec![
        home.join("Developer"),
        home.join("Projects"),
        home.join("Code"),
        home.join("workspace"),
        home.join("repos"),
        home.join("src"),
        home.join("Desktop"),
        home.join("Documents"),
    ];

    let mut candidates: Vec<ArchiveRecommendation> = Vec::new();

    for search_dir in &search_dirs {
        if !search_dir.exists() {
            continue;
        }

        let entries = match fs::read_dir(search_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }

            // Check if it's a project
            if let Some(project_type) = detect_project_type(&path) {
                // Check last access time
                if let Some(days) = most_recent_access(&path) {
                    // Only recommend if not accessed in 90+ days
                    if days >= 90 {
                        let size_bytes = dir_size(&path);

                        // Only recommend if > 50MB
                        if size_bytes > 50_000_000 {
                            let status = if days > 365 {
                                "Strong Archive Recommendation".to_string()
                            } else if days > 180 {
                                "Archive Recommended".to_string()
                            } else {
                                "Consider Archiving".to_string()
                            };

                            candidates.push(ArchiveRecommendation {
                                path: path.to_string_lossy().to_string(),
                                name,
                                size_bytes,
                                last_opened_days: days,
                                project_type,
                                status,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by size descending
    candidates.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    candidates
}

/// Verify a copy was successful by comparing sizes
pub fn verify_copy(source: &Path, destination: &Path) -> Result<bool, String> {
    if !source.exists() {
        return Err("Source does not exist".to_string());
    }
    if !destination.exists() {
        return Err("Destination does not exist".to_string());
    }

    let source_size = dir_size(source);
    let dest_size = dir_size(destination);

    // Allow 1% variance for filesystem metadata differences
    let diff = if source_size > dest_size {
        source_size - dest_size
    } else {
        dest_size - source_size
    };

    let threshold = source_size / 100; // 1%
    Ok(diff <= threshold)
}

/// Archive a project to an external location
pub fn archive_project(source_path: &str, destination_dir: &str) -> Result<String, String> {
    let source = Path::new(source_path);
    let dest_base = Path::new(destination_dir);

    if !source.exists() {
        return Err("Source project does not exist".to_string());
    }
    if !dest_base.exists() {
        return Err("Destination directory does not exist".to_string());
    }

    let project_name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let dest_path = dest_base.join(&project_name);

    if dest_path.exists() {
        return Err(format!(
            "Destination already exists: {}",
            dest_path.display()
        ));
    }

    // Copy the project
    copy_dir_recursive(source, &dest_path)?;

    // Verify the copy
    let verified = verify_copy(source, &dest_path)
        .map_err(|e| format!("Verification failed: {}", e))?;

    if !verified {
        // Clean up failed copy
        let _ = fs::remove_dir_all(&dest_path);
        return Err("Copy verification failed — sizes don't match. Original untouched.".to_string());
    }

    Ok(dest_path.to_string_lossy().to_string())
}

/// List available external volumes (macOS)
pub fn list_external_volumes() -> Vec<String> {
    let volumes_path = Path::new("/Volumes");
    if !volumes_path.exists() {
        return Vec::new();
    }

    fs::read_dir(volumes_path)
        .unwrap_or_else(|_| panic!("Cannot read /Volumes"))
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            // Skip the main Macintosh HD volume
            name != "Macintosh HD" && name != "Macintosh HD - Data"
        })
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
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
