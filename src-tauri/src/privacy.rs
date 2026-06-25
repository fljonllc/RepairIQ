use serde::{Deserialize, Serialize};
use std::fs;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyItem {
    pub category: String,
    pub app: String,
    pub description: String,
    pub size_bytes: u64,
    pub path: String,
    pub risk_level: String,
}

pub fn scan_privacy() -> Vec<PrivacyItem> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut items = Vec::new();

    // Recent files lists
    let recent_items = home.join("Library/Application Support/com.apple.sharedfilelist");
    if recent_items.exists() {
        let size = dir_size(&recent_items);
        items.push(PrivacyItem {
            category: "Recent Activity".to_string(),
            app: "macOS".to_string(),
            description: "List of recently opened files and applications".to_string(),
            size_bytes: size,
            path: recent_items.to_string_lossy().to_string(),
            risk_level: "Medium".to_string(),
        });
    }

    // Chrome history
    let chrome_history =
        home.join("Library/Application Support/Google/Chrome/Default/History");
    if chrome_history.exists() {
        let size = fs::metadata(&chrome_history).map(|m| m.len()).unwrap_or(0);
        items.push(PrivacyItem {
            category: "Browser History".to_string(),
            app: "Google Chrome".to_string(),
            description: "Complete browsing history with timestamps".to_string(),
            size_bytes: size,
            path: chrome_history.to_string_lossy().to_string(),
            risk_level: "High".to_string(),
        });
    }

    // Safari history
    let safari_history = home.join("Library/Safari/History.db");
    if safari_history.exists() {
        let size = fs::metadata(&safari_history).map(|m| m.len()).unwrap_or(0);
        items.push(PrivacyItem {
            category: "Browser History".to_string(),
            app: "Safari".to_string(),
            description: "Complete Safari browsing history".to_string(),
            size_bytes: size,
            path: safari_history.to_string_lossy().to_string(),
            risk_level: "High".to_string(),
        });
    }

    // Cookies
    let chrome_cookies =
        home.join("Library/Application Support/Google/Chrome/Default/Cookies");
    if chrome_cookies.exists() {
        let size = fs::metadata(&chrome_cookies).map(|m| m.len()).unwrap_or(0);
        items.push(PrivacyItem {
            category: "Cookies".to_string(),
            app: "Google Chrome".to_string(),
            description: "Tracking cookies from websites".to_string(),
            size_bytes: size,
            path: chrome_cookies.to_string_lossy().to_string(),
            risk_level: "High".to_string(),
        });
    }

    // Spotlight suggestions
    let spotlight = home.join("Library/Suggestions");
    if spotlight.exists() {
        let size = dir_size(&spotlight);
        items.push(PrivacyItem {
            category: "Search History".to_string(),
            app: "Spotlight".to_string(),
            description: "Search suggestions and history".to_string(),
            size_bytes: size,
            path: spotlight.to_string_lossy().to_string(),
            risk_level: "Low".to_string(),
        });
    }

    // QuickLook thumbnails
    let quicklook = home.join("Library/Caches/com.apple.QuickLook.thumbnailcache");
    if quicklook.exists() {
        let size = dir_size(&quicklook);
        items.push(PrivacyItem {
            category: "Thumbnails".to_string(),
            app: "QuickLook".to_string(),
            description: "Cached previews of files you've viewed".to_string(),
            size_bytes: size,
            path: quicklook.to_string_lossy().to_string(),
            risk_level: "Low".to_string(),
        });
    }

    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    items
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
