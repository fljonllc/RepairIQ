use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    pub name: String,
    pub path: String,
    pub item_type: String, // "LaunchAgent" | "LaunchDaemon" | "LoginItem" | "Registry" | "StartupFolder"
    pub enabled: bool,
    pub necessary: bool, // RepairIQ's assessment
    pub reason: String,
}

pub fn get_startup_items() -> Vec<StartupItem> {
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let mut items = Vec::new();

        // Query HKCU Run registry key
        if let Ok(output) = Command::new("reg")
            .args(["query", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("HKEY_") {
                    continue;
                }
                let parts: Vec<&str> = trimmed.splitn(3, "    ").collect();
                if parts.len() >= 3 && parts[1].trim() == "REG_SZ" {
                    let name = parts[0].trim().to_string();
                    let path = parts[2].trim().to_string();
                    let necessary = name.to_lowercase().contains("microsoft")
                        || name.to_lowercase().contains("windows")
                        || name.to_lowercase().contains("security");
                    items.push(StartupItem {
                        name: name.clone(),
                        path,
                        item_type: "Registry".to_string(),
                        enabled: true,
                        necessary,
                        reason: if necessary {
                            "Windows system startup entry".to_string()
                        } else {
                            "Third-party startup registry entry".to_string()
                        },
                    });
                }
            }
        }

        // Scan common startup folder
        let programdata_startup =
            PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Startup");
        if programdata_startup.exists() {
            if let Ok(entries) = fs::read_dir(&programdata_startup) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".lnk") || name.ends_with(".exe") {
                        let clean_name = name
                            .replace(".lnk", "")
                            .replace(".exe", "");
                        items.push(StartupItem {
                            name: clean_name,
                            path: entry.path().to_string_lossy().to_string(),
                            item_type: "StartupFolder".to_string(),
                            enabled: true,
                            necessary: false,
                            reason: "Startup folder shortcut — runs at login".to_string(),
                        });
                    }
                }
            }
        }

        // Scan user startup folder
        if let Ok(appdata) = std::env::var("APPDATA") {
            let user_startup = PathBuf::from(&appdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup");
            if user_startup.exists() {
                if let Ok(entries) = fs::read_dir(&user_startup) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".lnk") || name.ends_with(".exe") {
                            let clean_name = name
                                .replace(".lnk", "")
                                .replace(".exe", "");
                            items.push(StartupItem {
                                name: clean_name,
                                path: entry.path().to_string_lossy().to_string(),
                                item_type: "StartupFolder".to_string(),
                                enabled: true,
                                necessary: false,
                                reason: "User startup folder shortcut — runs at login".to_string(),
                            });
                        }
                    }
                }
            }
        }

        items.sort_by(|a, b| a.necessary.cmp(&b.necessary));
        items
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}
