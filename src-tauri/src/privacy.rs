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
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;

        let mut items = Vec::new();
        let localappdata = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        let appdata = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();

        // Chrome history
        let chrome_history = localappdata.join("Google\\Chrome\\User Data\\Default\\History");
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

        // Edge history
        let edge_history = localappdata.join("Microsoft\\Edge\\User Data\\Default\\History");
        if edge_history.exists() {
            let size = fs::metadata(&edge_history).map(|m| m.len()).unwrap_or(0);
            items.push(PrivacyItem {
                category: "Browser History".to_string(),
                app: "Microsoft Edge".to_string(),
                description: "Complete Edge browsing history".to_string(),
                size_bytes: size,
                path: edge_history.to_string_lossy().to_string(),
                risk_level: "High".to_string(),
            });
        }

        // Firefox profiles
        let firefox_profiles = appdata.join("Mozilla\\Firefox\\Profiles");
        if firefox_profiles.exists() {
            if let Ok(entries) = fs::read_dir(&firefox_profiles) {
                for entry in entries.flatten() {
                    let places = entry.path().join("places.sqlite");
                    if places.exists() {
                        let size = fs::metadata(&places).map(|m| m.len()).unwrap_or(0);
                        items.push(PrivacyItem {
                            category: "Browser History".to_string(),
                            app: "Mozilla Firefox".to_string(),
                            description: "Firefox browsing history and bookmarks database".to_string(),
                            size_bytes: size,
                            path: places.to_string_lossy().to_string(),
                            risk_level: "High".to_string(),
                        });
                        break; // Only report the first profile
                    }
                }
            }
        }

        // Chrome cookies
        let chrome_cookies = localappdata.join("Google\\Chrome\\User Data\\Default\\Cookies");
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

        // Edge cookies
        let edge_cookies = localappdata.join("Microsoft\\Edge\\User Data\\Default\\Cookies");
        if edge_cookies.exists() {
            let size = fs::metadata(&edge_cookies).map(|m| m.len()).unwrap_or(0);
            items.push(PrivacyItem {
                category: "Cookies".to_string(),
                app: "Microsoft Edge".to_string(),
                description: "Tracking cookies from websites".to_string(),
                size_bytes: size,
                path: edge_cookies.to_string_lossy().to_string(),
                risk_level: "High".to_string(),
            });
        }

        // Recent files
        let recent = appdata.join("Microsoft\\Windows\\Recent");
        if recent.exists() {
            let size = dir_size(&recent);
            items.push(PrivacyItem {
                category: "Recent Activity".to_string(),
                app: "Windows".to_string(),
                description: "List of recently opened files and documents".to_string(),
                size_bytes: size,
                path: recent.to_string_lossy().to_string(),
                risk_level: "Medium".to_string(),
            });
        }

        items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        items
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
