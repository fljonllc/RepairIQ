use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserCache {
    pub browser: String,
    pub size_bytes: u64,
    pub path: String,
    pub clean_command: String,
}

/// Detect browser caches across all installed browsers
pub fn detect_browser_caches() -> Vec<BrowserCache> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));
    let mut caches = Vec::new();
    
    // Chrome
    let chrome_cache = home.join("Library/Caches/Google/Chrome");
    let chrome_data = home.join("Library/Application Support/Google/Chrome/Default/Cache");
    if chrome_cache.exists() {
        let size = dir_size(&chrome_cache);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Google Chrome".to_string(),
                size_bytes: size,
                path: chrome_cache.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/Google/Chrome".to_string(),
            });
        }
    }
    if chrome_data.exists() {
        let size = dir_size(&chrome_data);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Chrome Cache Data".to_string(),
                size_bytes: size,
                path: chrome_data.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Application\\ Support/Google/Chrome/Default/Cache".to_string(),
            });
        }
    }
    
    // Safari
    let safari_cache = home.join("Library/Caches/com.apple.Safari");
    if safari_cache.exists() {
        let size = dir_size(&safari_cache);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Safari".to_string(),
                size_bytes: size,
                path: safari_cache.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/com.apple.Safari".to_string(),
            });
        }
    }
    
    // Firefox
    let firefox_dir = home.join("Library/Caches/Firefox/Profiles");
    if firefox_dir.exists() {
        let size = dir_size(&firefox_dir);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Firefox".to_string(),
                size_bytes: size,
                path: firefox_dir.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/Firefox".to_string(),
            });
        }
    }
    
    // Brave
    let brave_cache = home.join("Library/Caches/BraveSoftware/Brave-Browser");
    if brave_cache.exists() {
        let size = dir_size(&brave_cache);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Brave".to_string(),
                size_bytes: size,
                path: brave_cache.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/BraveSoftware".to_string(),
            });
        }
    }
    
    // Arc
    let arc_cache = home.join("Library/Caches/company.thebrowser.Browser");
    if arc_cache.exists() {
        let size = dir_size(&arc_cache);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Arc".to_string(),
                size_bytes: size,
                path: arc_cache.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/company.thebrowser.Browser".to_string(),
            });
        }
    }
    
    // Edge
    let edge_cache = home.join("Library/Caches/Microsoft Edge");
    if edge_cache.exists() {
        let size = dir_size(&edge_cache);
        if size > 0 {
            caches.push(BrowserCache {
                browser: "Microsoft Edge".to_string(),
                size_bytes: size,
                path: edge_cache.to_string_lossy().to_string(),
                clean_command: "rm -rf ~/Library/Caches/Microsoft\\ Edge".to_string(),
            });
        }
    }
    
    caches.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    caches
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
