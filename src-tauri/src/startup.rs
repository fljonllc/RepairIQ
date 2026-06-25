use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    pub name: String,
    pub path: String,
    pub item_type: String, // "LaunchAgent" | "LaunchDaemon" | "LoginItem"
    pub enabled: bool,
    pub necessary: bool, // RepairIQ's assessment
    pub reason: String,
}

pub fn get_startup_items() -> Vec<StartupItem> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut items = Vec::new();

    // User LaunchAgents
    let user_agents = home.join("Library/LaunchAgents");
    if user_agents.exists() {
        if let Ok(entries) = fs::read_dir(&user_agents) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".plist") {
                    let clean_name = name.replace(".plist", "");
                    let necessary =
                        clean_name.contains("apple") || clean_name.contains("com.apple");
                    items.push(StartupItem {
                        name: clean_name.clone(),
                        path: entry.path().to_string_lossy().to_string(),
                        item_type: "LaunchAgent".to_string(),
                        enabled: true,
                        necessary,
                        reason: if necessary {
                            "Apple system service".to_string()
                        } else {
                            "Third-party background service".to_string()
                        },
                    });
                }
            }
        }
    }

    // System LaunchAgents
    let sys_agents = PathBuf::from("/Library/LaunchAgents");
    if sys_agents.exists() {
        if let Ok(entries) = fs::read_dir(&sys_agents) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".plist") {
                    let clean_name = name.replace(".plist", "");
                    let necessary = clean_name.contains("com.apple");
                    items.push(StartupItem {
                        name: clean_name.clone(),
                        path: entry.path().to_string_lossy().to_string(),
                        item_type: "LaunchAgent".to_string(),
                        enabled: true,
                        necessary,
                        reason: if necessary {
                            "Apple system service".to_string()
                        } else {
                            "Third-party startup item — may slow boot".to_string()
                        },
                    });
                }
            }
        }
    }

    items.sort_by(|a, b| a.necessary.cmp(&b.necessary));
    items
}
