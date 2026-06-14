use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub count: u32,
    pub total_wasted: u64, // (count - 1) * size
    pub paths: Vec<String>,
}

/// Find duplicate files by size-matching + partial content hash
/// Strategy: group by size first (fast), then compare first 4KB of content (avoids full file hashing)
pub fn find_duplicates() -> Vec<DuplicateGroup> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));
    
    // Scan common user directories
    let scan_dirs = vec![
        home.join("Desktop"),
        home.join("Documents"),
        home.join("Downloads"),
    ];
    
    // Phase 1: Group files by size (only files > 1MB)
    let mut size_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    
    for dir in &scan_dirs {
        if !dir.exists() { continue; }
        for entry in WalkDir::new(dir).max_depth(5).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() { continue; }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size < 1_048_576 { continue; } // Skip < 1MB
            size_map.entry(size).or_default().push(entry.path().to_path_buf());
        }
    }
    
    // Phase 2: For files with same size, compare first 4KB
    let mut duplicates: Vec<DuplicateGroup> = Vec::new();
    
    for (size, paths) in &size_map {
        if paths.len() < 2 { continue; }
        
        // Hash first 4KB of each file
        let mut hash_groups: HashMap<Vec<u8>, Vec<&PathBuf>> = HashMap::new();
        for path in paths {
            if let Ok(partial_hash) = read_partial(path, 4096) {
                hash_groups.entry(partial_hash).or_default().push(path);
            }
        }
        
        for (hash_bytes, group_paths) in &hash_groups {
            if group_paths.len() < 2 { continue; }
            
            let file_name = group_paths[0].file_name()
                .unwrap_or_default().to_string_lossy().to_string();
            let count = group_paths.len() as u32;
            let total_wasted = (count as u64 - 1) * size;
            let hash = format!("{:x}", hash_bytes.iter().take(8).fold(0u64, |acc, &b| acc * 256 + b as u64));
            
            duplicates.push(DuplicateGroup {
                hash,
                file_name,
                size_bytes: *size,
                count,
                total_wasted,
                paths: group_paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            });
        }
    }
    
    duplicates.sort_by(|a, b| b.total_wasted.cmp(&a.total_wasted));
    duplicates
}

fn read_partial(path: &Path, bytes: usize) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![0u8; bytes];
    let n = reader.read(&mut buffer)?;
    buffer.truncate(n);
    Ok(buffer)
}
