use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use walkdir::WalkDir;

// ============================================================
// PUBLIC TYPES — unchanged interface
// ============================================================

/// Safety classification for each scanned item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SafetyLevel {
    Safe,
    Review,
    Archive,
    Protected,
}

impl Default for SafetyLevel {
    fn default() -> Self {
        SafetyLevel::Review
    }
}

/// A single item found during scanning — with definitive intelligence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedItem {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub category: String,
    pub subcategory: String,
    pub safety: SafetyLevel,
    pub safety_score: u8,
    pub last_accessed_days: Option<u64>,
    pub description: String,
    pub impact: String,
    pub recovery_method: String,
    pub owner: String,
    pub verdict: String,
    pub verdict_reason: String,
    pub file_count: u64,
    pub largest_files: Vec<String>,
    pub depends_on: Vec<String>,
    pub clean_command: String,
    pub recommendation: String,
    pub action_label: String,
    pub risk_level: String,
    pub time_to_rebuild: String,
    pub side_effects: String,
    pub why_here: String,
    pub reasoning: Vec<String>,
    pub confidence: u8,
    pub evidence: Vec<String>,
    pub why_recommended: String,
    pub what_if_wrong: String,
}

/// Category breakdown for the Storage Story
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBreakdown {
    pub name: String,
    pub size_bytes: u64,
    pub items: Vec<ScannedItem>,
}

/// Full scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub safe_recovery_bytes: u64,
    pub review_recovery_bytes: u64,
    pub archive_recovery_bytes: u64,
    pub categories: Vec<CategoryBreakdown>,
    pub items: Vec<ScannedItem>,
    pub scan_duration_ms: u64,
    pub health_score: u8,
    pub health_grade: String,
    pub health_factors: Vec<String>,
}

// ============================================================
// SYSTEM STATE — checked ONCE at start of scan, not per-item
// ============================================================

/// Cached system state to avoid spawning processes per-item
struct SystemState {
    docker_running: bool,
    docker_containers_active: bool,
    docker_has_volumes: bool,
    running_apps: Vec<String>,
}

impl SystemState {
    fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            let docker_running = Command::new("pgrep")
                .args(["-x", "Docker"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            let docker_containers_active = if docker_running {
                Command::new("docker")
                    .args(["ps", "-q"])
                    .output()
                    .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
                    .unwrap_or(false)
            } else {
                false
            };

            let docker_has_volumes = if docker_running {
                Command::new("docker")
                    .args(["volume", "ls", "-q"])
                    .output()
                    .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
                    .unwrap_or(false)
            } else {
                false
            };

            // Get ALL running processes in one call
            let running_apps = Command::new("ps")
                .args(["-axo", "comm"])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(|l| l.trim().to_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            Self {
                docker_running,
                docker_containers_active,
                docker_has_volumes,
                running_apps,
            }
        }

        #[cfg(target_os = "windows")]
        {
            let docker_running = Command::new("docker")
                .args(["info"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            let docker_containers_active = if docker_running {
                Command::new("docker")
                    .args(["ps", "-q"])
                    .output()
                    .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
                    .unwrap_or(false)
            } else {
                false
            };

            let docker_has_volumes = false;

            let running_apps = Command::new("tasklist")
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(|l| l.trim().to_lowercase())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            Self {
                docker_running,
                docker_containers_active,
                docker_has_volumes,
                running_apps,
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self {
                docker_running: false,
                docker_containers_active: false,
                docker_has_volumes: false,
                running_apps: vec![],
            }
        }
    }

    fn is_app_running(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();
        self.running_apps.iter().any(|a| a.contains(&name_lower))
    }
}

// ============================================================
// SINGLE-PASS DIRECTORY SCAN — all data in ONE walk
// ============================================================

/// Result of a single-pass directory walk
struct DirScanResult {
    total_size: u64,
    file_count: u64,
    largest_files: Vec<(String, u64)>,
    most_recent_days: Option<u64>,
    has_git: bool,
    has_uncommitted_changes: bool,
}

/// Walk a directory ONCE and gather ALL metrics
fn walk_directory_once(path: &Path) -> DirScanResult {
    let mut total_size = 0u64;
    let mut file_count = 0u64;
    let mut largest_files: Vec<(String, u64)> = Vec::new();
    let mut most_recent: Option<SystemTime> = None;
    let has_git = path.join(".git").exists();

    for entry in WalkDir::new(path)
        .max_depth(50)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        total_size += size;
        file_count += 1;

        // Track top 5 largest
        if largest_files.len() < 5 || size > largest_files.last().map(|f| f.1).unwrap_or(0) {
            let name = entry.file_name().to_string_lossy().to_string();
            largest_files.push((name, size));
            largest_files.sort_by(|a, b| b.1.cmp(&a.1));
            largest_files.truncate(5);
        }

        // Track most recent modification
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                most_recent = Some(match most_recent {
                    Some(prev) => prev.max(modified),
                    None => modified,
                });
            }
        }
    }

    // Check git status only if .git exists
    let has_uncommitted_changes = if has_git {
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output()
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    let most_recent_days = most_recent.and_then(|t| {
        SystemTime::now()
            .duration_since(t)
            .ok()
            .map(|d| d.as_secs() / 86400)
    });

    DirScanResult {
        total_size,
        file_count,
        largest_files,
        most_recent_days,
        has_git,
        has_uncommitted_changes,
    }
}

/// Format largest files for display
fn format_largest_files(files: &[(String, u64)]) -> Vec<String> {
    files
        .iter()
        .map(|(name, size)| format!("{} — {}", name, format_size(*size)))
        .collect()
}

// ============================================================
// INTELLIGENCE TYPES
// ============================================================

#[derive(Default)]
struct ItemIntel {
    description: String,
    impact: String,
    recovery_method: String,
    owner: String,
    safety_score: u8,
    safety: SafetyLevel,
    verdict: String,
    verdict_reason: String,
    depends_on: Vec<String>,
    clean_command: String,
    recommendation: String,
    action_label: String,
    risk_level: String,
    time_to_rebuild: String,
    side_effects: String,
    why_here: String,
    reasoning: Vec<String>,
    confidence: u8,
    evidence: Vec<String>,
    why_recommended: String,
    what_if_wrong: String,
}
// ============================================================
// UTILITY FUNCTIONS
// ============================================================

fn get_disk_info() -> (u64, u64, u64) {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("df")
            .args(["-k", "/"])
            .output()
            .unwrap_or_else(|_| panic!("Failed to run df"));

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        if lines.len() >= 2 {
            let parts: Vec<&str> = lines[1].split_whitespace().collect();
            if parts.len() >= 4 {
                let total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                let used = parts[2].parse::<u64>().unwrap_or(0) * 1024;
                let free = parts[3].parse::<u64>().unwrap_or(0) * 1024;
                return (total, used, free);
            }
        }
        (0, 0, 0)
    }

    #[cfg(target_os = "windows")]
    {
        // Use wmic on Windows
        let output = Command::new("wmic")
            .args(["logicaldisk", "where", "DeviceID='C:'", "get", "Size,FreeSpace", "/format:csv"])
            .output()
            .unwrap_or_else(|_| panic!("Failed to get disk info"));

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let free = parts[1].trim().parse::<u64>().unwrap_or(0);
                let total = parts[2].trim().parse::<u64>().unwrap_or(0);
                if total > 0 {
                    return (total, total - free, free);
                }
            }
        }
        (0, 0, 0)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        (0, 0, 0)
    }
}

/// Get platform-specific scan targets
fn get_scan_targets(home: &Path) -> Vec<(&'static str, &'static str, PathBuf)> {
    #[cfg(target_os = "macos")]
    {
        vec![
            ("System Data", "Caches", home.join("Library/Caches")),
            ("System Data", "Application Support", home.join("Library/Application Support")),
            ("System Data", "Logs", home.join("Library/Logs")),
            ("System Data", "Saved State", home.join("Library/Saved Application State")),
            ("Developer", "Xcode DerivedData", home.join("Library/Developer/Xcode/DerivedData")),
            ("Developer", "Xcode Archives", home.join("Library/Developer/Xcode/Archives")),
            ("Developer", "CoreSimulator", home.join("Library/Developer/CoreSimulator")),
            ("Developer", "Cargo Registry", home.join(".cargo/registry")),
            ("Developer", "npm Cache", home.join(".npm")),
            ("Docker", "Docker Data", home.join("Library/Containers/com.docker.docker")),
            ("Downloads", "Downloads", home.join("Downloads")),
            ("Virtual Machines", "UTM VMs", home.join("Library/Containers/com.utmapp.UTM/Data/Documents")),
            ("Messages", "Messages Data", home.join("Library/Messages")),
            ("Music", "Music Library", home.join("Music")),
            ("Documents", "Documents", home.join("Documents")),
            ("Desktop", "Desktop", home.join("Desktop")),
            ("Applications", "Applications", PathBuf::from("/Applications")),
            ("Trash", "Trash", home.join(".Trash")),
        ]
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = home.join("AppData");
        vec![
            ("System Data", "Temp Files", PathBuf::from("C:\\Windows\\Temp")),
            ("System Data", "Local Cache", appdata.join("Local\\Temp")),
            ("System Data", "App Cache", appdata.join("Local\\Microsoft")),
            ("Developer", "Cargo Registry", home.join(".cargo\\registry")),
            ("Developer", "npm Cache", appdata.join("Local\\npm-cache")),
            ("Developer", "nuget Cache", appdata.join("Local\\NuGet")),
            ("Docker", "Docker Data", appdata.join("Local\\Docker")),
            ("Downloads", "Downloads", home.join("Downloads")),
            ("Documents", "Documents", home.join("Documents")),
            ("Desktop", "Desktop", home.join("Desktop")),
            ("Applications", "Programs", PathBuf::from("C:\\Program Files")),
            ("Trash", "Recycle Bin", PathBuf::from("C:\\$Recycle.Bin")),
        ]
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        vec![
            ("Downloads", "Downloads", home.join("Downloads")),
            ("Documents", "Documents", home.join("Documents")),
            ("Desktop", "Desktop", home.join("Desktop")),
        ]
    }
}

fn days_since_access(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let accessed = metadata.accessed().ok()?;
    let now = SystemTime::now();
    let duration = now.duration_since(accessed).ok()?;
    Some(duration.as_secs() / 86400)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

fn calculate_confidence(signals: &[bool]) -> u8 {
    if signals.is_empty() {
        return 50;
    }
    let positive = signals.iter().filter(|&&s| s).count();
    let total = signals.len();
    let base = (positive as f64 / total as f64 * 100.0) as u8;
    base.min(99)
}

fn extract_app_name(name: &str) -> String {
    if (name.starts_with("com.") || name.starts_with("org.") || name.starts_with("io."))
        && name.contains('.')
    {
        let parts: Vec<&str> = name.split('.').collect();
        if let Some(last) = parts.last() {
            if !last.is_empty() {
                let capitalized = format!("{}{}", &last[..1].to_uppercase(), &last[1..]);
                return capitalized;
            }
        }
    }
    let clean = name
        .replace('-', " ")
        .replace('_', " ")
        .replace(".app", "")
        .replace(".savedState", "");
    if clean.is_empty() {
        return name.to_string();
    }
    clean
}

/// Check if path contains user-created files (not just caches/configs)
fn contains_user_data(path: &Path) -> bool {
    let user_indicators = [
        "documents", "photos", "pictures", "videos", "music",
        "desktop", "downloads", ".docx", ".pdf", ".xlsx",
        ".psd", ".sketch", ".fig",
    ];
    let path_str = path.to_string_lossy().to_lowercase();
    for indicator in &user_indicators {
        if path_str.contains(indicator) {
            return true;
        }
    }
    false
}

// ============================================================
// INTELLIGENCE ENGINE — uses cached SystemState
// ============================================================

fn analyze_path(path: &Path, category: &str, state: &SystemState, scan_result: Option<&DirScanResult>) -> ItemIntel {
    let path_str = path.to_string_lossy().to_lowercase();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let name_lower = name.to_lowercase();

    // === PROTECTED — NEVER TOUCH ===
    if path_str.contains("/system/") || path_str.contains("/usr/") || path_str.contains("/bin/") || path_str.contains("/sbin/") {
        return ItemIntel {
            description: "macOS system file — required for your computer to boot".into(),
            impact: "YOUR MAC WILL NOT START. This is a core operating system file.".into(),
            recovery_method: "Requires full macOS reinstallation from Recovery Mode (⌘+R on boot)".into(),
            owner: "macOS".into(),
            safety_score: 1,
            safety: SafetyLevel::Protected,
            verdict: "🚫 DO NOT DELETE".into(),
            verdict_reason: "This is a core macOS system file. Deleting it will break your computer.".into(),
            depends_on: vec!["macOS".to_string()],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    if path_str.contains(".ssh") {
        return ItemIntel {
            description: "SSH keys — your identity for GitHub, servers, and remote access".into(),
            impact: "You will be LOCKED OUT of GitHub, all remote servers, and any service using SSH key authentication.".into(),
            recovery_method: "You must generate new keys and manually re-add them to every service. Some access may be permanently lost.".into(),
            owner: "SSH / Git / Your servers".into(),
            safety_score: 1,
            safety: SafetyLevel::Protected,
            verdict: "🚫 DO NOT DELETE".into(),
            verdict_reason: "These are your authentication keys. Deleting them locks you out of remote services.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    if path_str.contains(".gnupg") {
        return ItemIntel {
            description: "GPG encryption keys — used for signing and encrypting".into(),
            impact: "Any GPG-encrypted files become PERMANENTLY UNREADABLE. Git commit signing breaks.".into(),
            recovery_method: "Cannot recover private keys. Encrypted data is lost forever.".into(),
            owner: "GPG / Git".into(),
            safety_score: 1,
            safety: SafetyLevel::Protected,
            verdict: "🚫 DO NOT DELETE".into(),
            verdict_reason: "Private encryption keys cannot be regenerated. You will lose access to encrypted data permanently.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    if path_str.contains("keychain") {
        return ItemIntel {
            description: "macOS Keychain — stores ALL your passwords and certificates".into(),
            impact: "Every saved password gone: WiFi, websites, app licenses, certificates, Apple ID tokens.".into(),
            recovery_method: "Cannot recover. Every password must be manually re-entered or reset.".into(),
            owner: "macOS".into(),
            safety_score: 1,
            safety: SafetyLevel::Protected,
            verdict: "🚫 DO NOT DELETE".into(),
            verdict_reason: "This contains all your saved passwords. There is no way to recover them.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    if path_str.contains("com.apple.") && (path_str.contains("/application support/") || path_str.contains("/preferences/")) {
        return ItemIntel {
            description: format!("Apple system data — {}", name),
            impact: "May break built-in macOS features (iCloud, Mail, Messages, etc.)".into(),
            recovery_method: "Some regenerates on reboot, some requires macOS reset.".into(),
            owner: "macOS / Apple".into(),
            safety_score: 2,
            safety: SafetyLevel::Protected,
            verdict: "🚫 DO NOT DELETE".into(),
            verdict_reason: "Apple system data. macOS depends on this for core functionality.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === XCODE DERIVEDDATA ===
    if path_str.contains("deriveddata") {
        let xcode_running = state.is_app_running("xcode");
        let (verdict, reason) = if xcode_running {
            ("✅ YES — Clean it (close Xcode first)".to_string(),
             "Xcode is currently open. Close it first, then clean. It rebuilds in 2-5 minutes on next open.".to_string())
        } else {
            ("✅ YES — Clean it now".to_string(),
             "Xcode is not running. This is purely build cache. It rebuilds automatically next time you compile. Zero data loss.".to_string())
        };

        return ItemIntel {
            description: "Xcode build cache — compiled code and indexes. NOT your source code.".into(),
            impact: "None. Your projects and source code are untouched. First build after cleaning takes 2-5 minutes instead of seconds.".into(),
            recovery_method: "Automatic. Open any project in Xcode and it rebuilds instantly. No action needed.".into(),
            owner: "Xcode".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === NODE_MODULES ===
    if path_str.contains("node_modules") {
        return ItemIntel {
            description: "Downloaded JavaScript packages — NOT your code, just library copies".into(),
            impact: "None. Your source code, your work, your configs — all untouched. These are just downloaded copies of open-source libraries.".into(),
            recovery_method: "Run 'npm install' in the project folder. Takes 10-30 seconds. Everything comes back identical.".into(),
            owner: "npm / Node.js".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "These are downloaded library copies. Your code is separate. 'npm install' restores everything in seconds.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === DOCKER ===
    if path_str.contains("docker") || path_str.contains("com.docker") {
        if path_str.contains("overlay") || path_str.contains("/data/") || path_str.contains("com.docker.docker") {
            let (verdict, reason, score) = if !state.docker_running {
                ("✅ YES — Clean it now".to_string(),
                 "Docker is not running. This is all cached data. When you start Docker again, it re-downloads what it needs.".to_string(),
                 10u8)
            } else if state.docker_containers_active {
                ("⚠️ STOP CONTAINERS FIRST".to_string(),
                 "Docker has running containers RIGHT NOW. Stop them first with 'docker stop $(docker ps -q)' then clean. Or run 'docker system prune -a' to clean only unused data.".to_string(),
                 5)
            } else if state.docker_has_volumes {
                ("✅ YES — But run 'docker system prune -a' instead".to_string(),
                 "Docker is running but no containers are active. You have named volumes (may contain database data). Use 'docker system prune -a' to safely clean only unused data.".to_string(),
                 8)
            } else {
                ("✅ YES — Clean it now".to_string(),
                 "Docker is running but nothing is active — no containers, no important volumes. This is all just cached layers taking up space.".to_string(),
                 9)
            };

            return ItemIntel {
                description: "Docker container images, layers, and build cache".into(),
                impact: if state.docker_containers_active {
                    "Active containers will be destroyed. Stop them first.".into()
                } else {
                    "None. No containers are running. Images re-download automatically when needed.".into()
                },
                recovery_method: "'docker pull <image>' re-downloads any image. 'docker compose up' rebuilds your stack.".into(),
                owner: "Docker Desktop".into(),
                safety_score: score,
                safety: if score >= 8 { SafetyLevel::Safe } else { SafetyLevel::Review },
                verdict,
                verdict_reason: reason,
                depends_on: vec![],
                clean_command: String::new(),
                ..Default::default()
            };
        }
    }

    // === GENERAL CACHES ===
    if path_str.contains("/caches/") {
        let app_name = extract_app_name(&name);
        let app_running = state.is_app_running(&name_lower.replace("com.", "").replace('.', " "));
        let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(999);

        let (verdict, reason) = if days > 30 {
            ("✅ YES — Clean it now".to_string(),
             format!("This cache hasn't been updated in {} days. The app hasn't needed it. It regenerates if the app ever needs it again.", days))
        } else if app_running {
            ("✅ YES — But quit the app first".to_string(),
             format!("{} is currently running. Quit it first, then clean. Cache rebuilds next time you open it.", app_name))
        } else {
            ("✅ YES — Clean it now".to_string(),
             format!("{} is not running. This cache regenerates automatically next time you open the app. Zero data loss.", app_name))
        };

        return ItemIntel {
            description: format!("Temporary cache for {} — speeds up the app but contains no personal data", app_name),
            impact: format!("{} may take a few extra seconds to start next time while it rebuilds its cache. All your settings, logins, and data are stored elsewhere.", app_name),
            recovery_method: format!("Automatic. Just open {}. It rebuilds its own cache. You do nothing.", app_name),
            owner: app_name,
            safety_score: 9,
            safety: SafetyLevel::Safe,
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === LOGS ===
    if path_str.contains("/logs/") {
        let app_name = extract_app_name(&name);
        return ItemIntel {
            description: format!("Log files from {} — text records of past activity", app_name),
            impact: "None. Logs are historical records for debugging. Deleting them affects nothing about how your apps work.".into(),
            recovery_method: "New logs are created fresh automatically. Old logs are never needed unless you're debugging a past issue.".into(),
            owner: app_name,
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Log files are purely historical text. They affect nothing. Apps create new ones automatically.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === TRASH ===
    if path_str.contains(".trash") {
        return ItemIntel {
            description: "Files you already deleted — waiting in Trash for final removal".into(),
            impact: "None. You already decided to delete these. This just makes it permanent.".into(),
            recovery_method: "Use Recovery Vault to keep them accessible for 7-30 more days if you're not sure.".into(),
            owner: "You".into(),
            safety_score: 9,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "You already deleted these files. They're sitting in Trash doing nothing but wasting space.".into(),
            depends_on: vec![],
            clean_command: "rm -rf ~/.Trash/*".to_string(),
            ..Default::default()
        };
    }

    // === RUST TARGET ===
    if path_str.contains("/target/debug") || path_str.contains("/target/release") {
        return ItemIntel {
            description: "Rust compiled output — binary files generated from your source code".into(),
            impact: "None. Your source code (.rs files) is untouched. Next 'cargo build' recompiles (1-5 min for large projects).".into(),
            recovery_method: "Run 'cargo build' in the project. Everything recompiles from your source code.".into(),
            owner: "Rust / Cargo".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "These are compiled binaries, not your code. Your .rs source files are in a completely different location. 'cargo build' recreates these.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === SWIFT .build ===
    if path_str.contains("/.build/") {
        return ItemIntel {
            description: "Swift Package Manager build cache — compiled dependencies".into(),
            impact: "None. Source code untouched. Rebuilds automatically on next compile.".into(),
            recovery_method: "Run 'swift build' or open in Xcode. Automatic.".into(),
            owner: "Swift / SPM".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Build cache only. Your Swift source code is separate and untouched.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === COCOAPODS ===
    if path_str.contains("cocoapods") {
        return ItemIntel {
            description: "CocoaPods cache — downloaded iOS library copies".into(),
            impact: "None. Your Podfile still exists. Run 'pod install' and everything comes back.".into(),
            recovery_method: "Run 'pod install' in your project. Takes 1-2 minutes.".into(),
            owner: "CocoaPods".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Cached library downloads. Your project's Podfile defines what to re-download. Nothing personal in here.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === NPM CACHE ===
    if path_str.contains("/.npm/") || path_str.contains("/_cacache") {
        return ItemIntel {
            description: "npm's global download cache — copies of every package you've ever installed".into(),
            impact: "None. This is npm's internal cache to speed up installs. Removing it just means slightly slower next install.".into(),
            recovery_method: "Automatic. npm rebuilds this cache as you install packages. No manual action needed.".into(),
            owner: "npm".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "This is npm's internal speed cache. It contains nothing unique — just copies of packages from the internet.".into(),
            depends_on: vec!["npm".to_string(), "Node.js projects".to_string()],
            clean_command: "npm cache clean --force".to_string(),
            ..Default::default()
        };
    }

    // === YARN CACHE ===
    if path_str.contains("/.yarn/cache") || path_str.contains("/yarn/cache") {
        return ItemIntel {
            description: "Yarn package cache — offline copies of JavaScript packages".into(),
            impact: "None. Yarn re-downloads packages on next install. Slightly slower installs until cache rebuilds.".into(),
            recovery_method: "Automatic. Yarn rebuilds cache on next 'yarn install'.".into(),
            owner: "Yarn".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Downloaded package copies. Nothing unique. Yarn re-downloads from the internet as needed.".into(),
            depends_on: vec!["Yarn".to_string()],
            clean_command: "yarn cache clean".to_string(),
            ..Default::default()
        };
    }

    // === GRADLE ===
    if path_str.contains("/.gradle/") {
        return ItemIntel {
            description: "Gradle build cache — Java/Android compiled artifacts and dependencies".into(),
            impact: "None. Source code untouched. Android Studio re-downloads and recompiles on next build.".into(),
            recovery_method: "Open project in Android Studio or run 'gradle build'. Everything rebuilds.".into(),
            owner: "Gradle / Android Studio".into(),
            safety_score: 9,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Build cache and downloaded dependencies. Your Java/Kotlin source code is separate.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === PYCACHE ===
    if path_str.contains("__pycache__") {
        return ItemIntel {
            description: "Python bytecode cache — pre-compiled .pyc files".into(),
            impact: "None. Python recreates these instantly when you run any script.".into(),
            recovery_method: "Automatic. Python regenerates .pyc files on next run. Instant.".into(),
            owner: "Python".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Bytecode cache. Python recreates these in milliseconds. They exist only to speed up imports.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === CARGO REGISTRY ===
    if path_str.contains(".cargo/registry") {
        return ItemIntel {
            description: "Cargo's crate registry — downloaded Rust package source code".into(),
            impact: "None. Your projects are untouched. Cargo re-downloads crates from crates.io on next build.".into(),
            recovery_method: "Automatic. Cargo downloads needed crates on next 'cargo build'. Internet required.".into(),
            owner: "Rust / Cargo".into(),
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "Downloaded crate source code from the internet. Nothing unique. Cargo re-fetches on demand.".into(),
            depends_on: vec!["Rust projects".to_string()],
            clean_command: "cargo cache --autoclean".to_string(),
            ..Default::default()
        };
    }

    // === XCODE CORESIMULATOR ===
    if path_str.contains("coresimulator") {
        let xcode_running = state.is_app_running("xcode");
        let (verdict, reason) = if xcode_running {
            ("⚠️ CLOSE XCODE FIRST".to_string(),
             "Xcode is currently open and may be using simulator data. Close Xcode, then clean.".to_string())
        } else {
            ("✅ YES — Clean it (but expect re-downloads)".to_string(),
             "Simulator runtimes will need to be re-downloaded (5-8GB per iOS version). If you do iOS development, this takes time but causes no data loss.".to_string())
        };

        return ItemIntel {
            description: "iOS Simulator runtimes and device data — test devices for iOS development".into(),
            impact: "You'll need to re-download simulator runtimes from Apple (5-8GB each). No source code is affected.".into(),
            recovery_method: "Xcode > Settings > Platforms > Download. Takes 10-20 min per runtime on fast internet.".into(),
            owner: "Xcode".into(),
            safety_score: 7,
            safety: SafetyLevel::Review,
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === XCODE ARCHIVES ===
    if path_str.contains("xcode") && path_str.contains("archives") {
        let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(0);
        let (verdict, reason) = if days > 90 {
            ("✅ YES — Clean it".to_string(),
             format!("These archives are {} days old. If you haven't submitted to the App Store recently, they're just taking up space. You can rebuild from source anytime.", days))
        } else {
            ("⚠️ KEEP if you submit to App Store".to_string(),
             "These are recent builds. If you submit iOS/Mac apps to the App Store, you may need these for crash symbolication. If you don't publish apps, clean them.".to_string())
        };

        return ItemIntel {
            description: "Xcode Archives — compiled app bundles for App Store submission".into(),
            impact: "If you publish to the App Store, you lose the ability to debug crashes from these specific builds. Source code is NOT affected.".into(),
            recovery_method: "Rebuild from source: Product > Archive in Xcode. Takes 5-10 minutes.".into(),
            owner: "Xcode".into(),
            safety_score: if days > 90 { 8 } else { 5 },
            safety: if days > 90 { SafetyLevel::Safe } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === APPLICATION SUPPORT ===
    if path_str.contains("/application support/") {
        let app_name = extract_app_name(&name);
        let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(0);
        let app_running = state.is_app_running(&name_lower.replace("com.", "").replace('.', " "));

        let safe_app_data = ["crashreporter", "caches", "crashpad", "gpuinfo"];
        for safe in &safe_app_data {
            if name_lower.contains(safe) {
                return ItemIntel {
                    description: format!("Crash/diagnostic data for {} — not user data", app_name),
                    impact: "None. This is crash reporting data, not your settings or files.".into(),
                    recovery_method: "Regenerates automatically if the app crashes again.".into(),
                    owner: app_name,
                    safety_score: 9,
                    safety: SafetyLevel::Safe,
                    verdict: "✅ YES — Clean it now".into(),
                    verdict_reason: "Crash/diagnostic data only. Contains no personal settings or files.".into(),
                    depends_on: vec![],
                    clean_command: String::new(),
                    ..Default::default()
                };
            }
        }

        let (verdict, reason, score) = if days > 180 && !app_running {
            ("✅ YES — You haven't used this in 6+ months".to_string(),
             format!("{} hasn't been used in {} days. Its settings are stale. If you ever reopen it, you'll just log in again.", app_name, days),
             8u8)
        } else if app_running {
            ("🚫 NO — app is currently running".to_string(),
             format!("{} is running right now. Deleting its data while running could corrupt it. Quit the app first if you want to clean this.", app_name),
             3)
        } else {
            ("⚠️ WILL RESET APP — You'll need to log in again".to_string(),
             format!("{} will lose its settings and login state. The app still works but opens like a fresh install. If you use this app regularly, you'll need to reconfigure it.", app_name),
             5)
        };

        return ItemIntel {
            description: format!("{} — stores login sessions, preferences, and local data", app_name),
            impact: format!("{} will reset to a fresh state. You'll need to log in again and redo any preferences.", app_name),
            recovery_method: format!("Re-open {} and log in again. Preferences need to be manually reconfigured.", app_name),
            owner: app_name,
            safety_score: score,
            safety: if score >= 8 { SafetyLevel::Safe } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === SAVED APPLICATION STATE ===
    if path_str.contains("saved application state") {
        let app_name = extract_app_name(&name);
        return ItemIntel {
            description: format!("Window positions and tabs for {} — just UI state", app_name),
            impact: format!("None. {} will open fresh instead of restoring your last window positions. That's it.", app_name),
            recovery_method: "Automatic. The app recreates this the moment you use it.".into(),
            owner: app_name,
            safety_score: 10,
            safety: SafetyLevel::Safe,
            verdict: "✅ YES — Clean it now".into(),
            verdict_reason: "This only stores window positions and tab state. Zero actual data. The app recreates it instantly.".into(),
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === APPLICATIONS ===
    if category == "Applications" {
        let days = days_since_access(path).unwrap_or(0);
        let app_running = state.is_app_running(&name_lower.replace(".app", ""));

        let (verdict, reason, score) = if app_running {
            ("🚫 NO — Currently running".to_string(),
             format!("{} is running right now. You can't remove a running application.", name),
             2u8)
        } else if days > 180 {
            ("✅ YES — You haven't opened this in 6+ months".to_string(),
             format!("You last opened {} over {} days ago. If you haven't needed it in 6 months, you don't need it. Re-download from App Store anytime.", name, days),
             8)
        } else if days > 30 {
            ("⚠️ PROBABLY SAFE — Not used in a while".to_string(),
             format!("You haven't opened {} in {} days. If you don't remember why you have it, it's probably safe to remove.", name, days),
             6)
        } else {
            ("⚠️ RECENTLY USED — Keep it unless you're sure".to_string(),
             format!("You opened {} within the last month. You're probably still using it.", name),
             4)
        };

        return ItemIntel {
            description: format!("{} — installed application", name),
            impact: "The app disappears. Re-download from App Store or developer website if you need it later.".into(),
            recovery_method: "Re-download from the Mac App Store or the developer's website.".into(),
            owner: name.clone(),
            safety_score: score,
            safety: if score >= 8 { SafetyLevel::Safe } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === VIRTUAL MACHINES ===
    if category == "Virtual Machines" {
        let days = days_since_access(path).unwrap_or(0);
        let name_lower = name.to_lowercase();
        let is_utm = name_lower.contains(".utm");

        let (verdict, reason, score) = if days > 90 {
            ("📦 ARCHIVE — VM not used in 3+ months".to_string(),
             format!("This virtual machine hasn't been opened in {} days. It's taking significant space. Archive to external drive or delete if no longer needed.", days),
             7u8)
        } else if days > 30 {
            ("⚠️ REVIEW — VM not used in {} days".to_string(),
             format!("VM last used {} days ago. If you're done with this project, archiving frees major space.", days),
             5)
        } else {
            ("⚠️ ACTIVE — Recently used VM".to_string(),
             format!("Used {} days ago. This is an active virtual machine.", days),
             3)
        };

        return ItemIntel {
            description: format!("{} — Virtual Machine disk image", name),
            impact: "The entire VM and its contents are removed. Any work inside the VM is lost.".into(),
            recovery_method: "Cannot recover unless backed up. VMs must be recreated from scratch or restored from a backup.".into(),
            owner: if is_utm { "UTM".to_string() } else { "Virtual Machine".to_string() },
            safety_score: score,
            safety: if score >= 7 { SafetyLevel::Archive } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec!["UTM app".to_string()],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === MESSAGES ===
    if category == "Messages" {
        let is_attachments = name.to_lowercase().contains("attach") || name.to_lowercase().contains("media");
        
        let (verdict, reason, score) = if is_attachments {
            ("⚠️ REVIEW — Message attachments".to_string(),
             "These are photos, videos, and files people sent you via iMessage. Removing them clears them from Messages but they may not be recoverable.".to_string(),
             4u8)
        } else {
            ("🚫 NO — Message database".to_string(),
             "This contains your actual message history. Deleting it removes all your conversations permanently.".to_string(),
             2)
        };

        return ItemIntel {
            description: format!("{} — iMessage/SMS data", name),
            impact: if is_attachments {
                "Attachments (photos/videos sent to you) are removed from Messages. Text conversations remain.".into()
            } else {
                "ALL message history is permanently deleted. Cannot be undone.".into()
            },
            recovery_method: "Cannot recover. Messages are not re-downloadable. Only iCloud backup can restore them.".into(),
            owner: "Messages / iMessage".to_string(),
            safety_score: score,
            safety: SafetyLevel::Review,
            verdict,
            verdict_reason: reason,
            depends_on: vec!["Messages.app".to_string(), "iCloud".to_string()],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === MUSIC ===
    if category == "Music" {
        let days = days_since_access(path).unwrap_or(0);
        let name_lower = name.to_lowercase();
        let is_garageband = name_lower.contains("garageband");
        let is_logic = name_lower.contains("logic");

        let (verdict, reason, score) = if is_garageband || is_logic {
            if days > 180 {
                ("📦 ARCHIVE — Old music project".to_string(),
                 format!("This {} project hasn't been opened in {} days. Archive to external drive to free space.", if is_garageband { "GarageBand" } else { "Logic Pro" }, days),
                 6u8)
            } else {
                ("⚠️ REVIEW — Music production project".to_string(),
                 "This contains your music production work. Only remove if you have a backup.".to_string(),
                 3)
            }
        } else if days > 180 {
            ("⚠️ REVIEW — Not accessed in 6+ months".to_string(),
             format!("Music file/folder not accessed in {} days. Check if you still listen to this.", days),
             5)
        } else {
            ("⚠️ ACTIVE — Your music library".to_string(),
             "This is part of your active music collection.".to_string(),
             3)
        };

        return ItemIntel {
            description: format!("{} — Music library content", name),
            impact: "Music files or projects are removed. Downloaded music can be re-downloaded from Apple Music. Original recordings cannot.".into(),
            recovery_method: "Apple Music tracks re-download automatically. Original recordings need a backup.".into(),
            owner: "Music / Apple Music".to_string(),
            safety_score: score,
            safety: if score >= 6 { SafetyLevel::Archive } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec!["Music.app".to_string()],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === DOWNLOADS ===
    if category == "Downloads" {
        let days = days_since_access(path).unwrap_or(0);

        let (verdict, reason, score) = if days > 90 {
            ("✅ YES — Not opened in 3+ months".to_string(),
             format!("You downloaded this {} days ago and haven't opened it since. It's either a one-time download or something you forgot about.", days),
             9u8)
        } else if days > 30 {
            ("✅ PROBABLY — Not used in a while".to_string(),
             format!("Downloaded and not opened in {} days. Most downloads are one-time-use (installers, attachments). Safe to clean.", days),
             7)
        } else {
            ("⚠️ RECENTLY USED — Check before cleaning".to_string(),
             format!("Accessed {} days ago. You may still be actively using this file.", days),
             4)
        };

        return ItemIntel {
            description: format!("{} — downloaded file", name),
            impact: "The file is removed. If it was an installer you already ran, no impact. If it's a document you need, you lose it.".into(),
            recovery_method: "Re-download from the original source (email, website, etc).".into(),
            owner: "You (Downloads)".into(),
            safety_score: score,
            safety: if score >= 7 { SafetyLevel::Safe } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === DOCUMENTS / DESKTOP ===
    if category == "Documents" || category == "Desktop" {
        let days = days_since_access(path).unwrap_or(0);
        let has_git = scan_result.map(|r| r.has_git).unwrap_or_else(|| path.join(".git").exists());
        let has_changes = scan_result.map(|r| r.has_uncommitted_changes).unwrap_or(false);

        let (verdict, reason, score) = if has_changes {
            ("🚫 NO — Has uncommitted code changes".to_string(),
             "This project has uncommitted Git changes. You have work that hasn't been pushed. DO NOT delete.".to_string(),
             2u8)
        } else if has_git && days > 180 {
            ("📦 ARCHIVE — Move to external drive".to_string(),
             format!("Git project not touched in {} days. All changes are committed. Safe to archive to external storage — your code lives on GitHub/remote.", days),
             7)
        } else if days > 365 {
            ("📦 ARCHIVE — Not opened in over a year".to_string(),
             format!("Not accessed in {} days (over a year). Move to an external drive rather than deleting — this is your personal data.", days),
             6)
        } else if days > 180 {
            ("📦 ARCHIVE — Consider moving to external drive".to_string(),
             format!("Not accessed in {} days. This is YOUR data — we recommend archiving to an external drive rather than deleting.", days),
             5)
        } else {
            ("🚫 NO — This is your active data".to_string(),
             format!("Accessed {} days ago. This is your personal/work data. Keep it.", days),
             2)
        };

        return ItemIntel {
            description: format!("{} — your personal files", name),
            impact: "THIS IS YOUR DATA. It cannot be recovered from the internet. Only delete if you have a backup.".into(),
            recovery_method: "Time Machine backup, iCloud, or external drive backup only. Cannot be re-downloaded.".into(),
            owner: "You".into(),
            safety_score: score,
            safety: if score >= 7 { SafetyLevel::Archive } else if score >= 5 { SafetyLevel::Archive } else { SafetyLevel::Review },
            verdict,
            verdict_reason: reason,
            depends_on: vec![],
            clean_command: String::new(),
            ..Default::default()
        };
    }

    // === DEFAULT FALLBACK ===
    let app_name = extract_app_name(&name);
    let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(0);

    let (verdict, reason, score) = if days > 180 {
        ("⚠️ PROBABLY SAFE — Inactive for 6+ months".to_string(),
         format!("This hasn't been modified in {} days. Likely safe to remove, but review contents if unsure.", days),
         6u8)
    } else {
        ("⚠️ REVIEW — Unknown purpose".to_string(),
         "We couldn't determine exactly what this is. Open the folder to review before deciding.".to_string(),
         5)
    };

    ItemIntel {
        description: format!("{} — review contents before removing", name),
        impact: "Unknown. Check what's inside before deciding.".into(),
        recovery_method: "Depends on contents. Use Recovery Vault for safety.".into(),
        owner: app_name,
        safety_score: score,
        safety: SafetyLevel::Review,
        verdict,
        verdict_reason: reason,
        depends_on: vec![],
        clean_command: String::new(),
        ..Default::default()
    }
}
// ============================================================
// FILL ADVISOR FIELDS — same logic, uses cached state
// ============================================================

fn fill_advisor_fields(mut intel: ItemIntel, path: &Path, category: &str, state: &SystemState, scan_result: Option<&DirScanResult>) -> ItemIntel {
    let path_str = path.to_string_lossy().to_lowercase();
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    // If already filled, return as-is
    if !intel.recommendation.is_empty() {
        return intel;
    }

    match intel.safety {
        SafetyLevel::Protected => {
            intel.recommendation = "Do Not Touch".to_string();
            intel.action_label = String::new();
            intel.risk_level = "Critical".to_string();
            intel.time_to_rebuild = "Requires macOS reinstall".to_string();
            intel.side_effects = "System will not function".to_string();
            intel.why_here = "macOS requires this for basic operation.".to_string();
            intel.reasoning = vec![
                "System-critical file".to_string(),
                "Cannot be rebuilt without reinstallation".to_string(),
                "Active system dependency".to_string(),
            ];
            intel.confidence = 99;
            intel.evidence = vec![
                "✓ System-critical component".to_string(),
                "✓ Active dependency detected".to_string(),
                "✓ Cannot be regenerated".to_string(),
                "✓ Contains irreplaceable data".to_string(),
            ];
            intel.why_recommended = "This is a system-critical or irreplaceable item. RepairIQ detected active dependencies and no safe way to remove it.".to_string();
            intel.what_if_wrong = "N/A — RepairIQ will not allow removal of this item.".to_string();
        }
        SafetyLevel::Archive => {
            intel.recommendation = "Archive".to_string();
            intel.action_label = "Archive Project".to_string();
            intel.risk_level = "High".to_string();
            intel.time_to_rebuild = "Cannot rebuild — this is your original work".to_string();
            intel.side_effects = "Your files are permanently removed unless backed up".to_string();
            intel.why_here = format!("This is your personal file/project stored on {}.", category);
            intel.reasoning = vec![
                "Contains your personal data".to_string(),
                "Cannot be re-downloaded".to_string(),
                "No automatic backup detected".to_string(),
            ];
            let has_git = scan_result.map(|r| r.has_git).unwrap_or_else(|| path.join(".git").exists());
            let has_changes = scan_result.map(|r| r.has_uncommitted_changes).unwrap_or(false);
            let days = days_since_access(path).unwrap_or(0);
            let signals = vec![days > 180, !has_changes, has_git];
            intel.confidence = calculate_confidence(&signals);
            intel.evidence = vec![
                if days > 180 { format!("✓ Not accessed in {} days", days) } else { format!("✗ Recently accessed ({} days ago)", days) },
                if !has_changes { "✓ No uncommitted changes".to_string() } else { "✗ Has uncommitted changes".to_string() },
                if has_git { "✓ Has version control backup".to_string() } else { "✗ No version control detected".to_string() },
            ];
            intel.why_recommended = "This appears to be inactive personal data. RepairIQ recommends archiving to external storage rather than deleting, because this content cannot be re-downloaded.".to_string();
            intel.what_if_wrong = "If you still need this data, it remains in the Recovery Vault. Nothing is permanently deleted without your explicit confirmation.".to_string();
        }
        SafetyLevel::Safe => {
            if path_str.contains("docker") || path_str.contains("com.docker") {
                let signals = vec![
                    !state.docker_running || !state.docker_containers_active,
                    true,
                    true,
                    !state.docker_containers_active,
                ];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !state.docker_running { "✓ Docker is not running".to_string() } else if !state.docker_containers_active { "✓ No active containers".to_string() } else { "✗ Active containers detected".to_string() },
                    "✓ Auto-generated cache — recreates on demand".to_string(),
                    "✓ No personal files — only downloaded images".to_string(),
                    if !state.docker_containers_active { "✓ No active dependencies".to_string() } else { "✗ Containers depend on this data".to_string() },
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = if state.docker_containers_active { "Low" } else { "None" }.to_string();
                intel.time_to_rebuild = "1-5 minutes (re-downloads images on demand)".to_string();
                intel.side_effects = "Docker will re-download container images when you need them next".to_string();
                intel.why_here = "You build software. Docker stores container images, build layers, and cached dependencies. This grows every time you pull or build an image.".to_string();
                intel.reasoning = vec![
                    "Regenerates automatically".to_string(),
                    "No personal files inside".to_string(),
                    if !state.docker_containers_active { "No active containers detected".to_string() } else { "Active containers detected — stop them first".to_string() },
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("node_modules") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only downloaded libraries".to_string(),
                    "✓ Your source code is in a separate folder".to_string(),
                    "✓ Rebuilds with 'npm install' in seconds".to_string(),
                    "✓ No login credentials or personal data".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "10-30 seconds (npm install)".to_string();
                intel.side_effects = "None — your code is untouched".to_string();
                intel.why_here = "Every npm project downloads its own copy of libraries. This is a duplicate download, not your work.".to_string();
                intel.reasoning = vec![
                    "Contains only downloaded libraries".to_string(),
                    "Your source code is elsewhere".to_string(),
                    "Rebuilds with one command".to_string(),
                    "No login or personal data".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("deriveddata") {
                let xcode_running = state.is_app_running("xcode");
                let signals = vec![!xcode_running, true, true, true];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !xcode_running { "✓ Xcode is not running".to_string() } else { "✗ Xcode is currently open".to_string() },
                    "✓ Auto-generated build cache — recreates on compile".to_string(),
                    "✓ No personal files — only compiled binaries".to_string(),
                    "✓ Source code stored separately".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "2-5 minutes (Xcode rebuilds on open)".to_string();
                intel.side_effects = "Xcode will recompile your project on next open".to_string();
                intel.why_here = "Xcode generates compiled code and indexes every time you build. This is purely build output, not your source code.".to_string();
                intel.reasoning = vec![
                    "Contains only compiled output".to_string(),
                    "Your .swift source files are untouched".to_string(),
                    "Xcode rebuilds automatically".to_string(),
                    "No personal data or settings".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/.npm/") || path_str.contains("/_cacache") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only cached package downloads".to_string(),
                    "✓ Rebuilds automatically on next install".to_string(),
                    "✓ No personal data or credentials".to_string(),
                    "✓ No effect on existing projects".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Instant (rebuilds as you install packages)".to_string();
                intel.side_effects = "npm installs will be slightly slower until cache rebuilds".to_string();
                intel.why_here = "npm keeps a copy of every package you've ever installed to speed up future installs. It accumulates over time.".to_string();
                intel.reasoning = vec![
                    "Contains only cached downloads".to_string(),
                    "Rebuilds automatically on next install".to_string(),
                    "No personal data".to_string(),
                    "No effect on existing projects".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains(".cargo/registry") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only downloaded crate sources".to_string(),
                    "✓ Your Rust code is untouched".to_string(),
                    "✓ Cargo re-downloads on demand".to_string(),
                    "✓ Nothing unique or personal".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "1-2 minutes (Cargo re-downloads on build)".to_string();
                intel.side_effects = "First Rust build will be slower while crates download".to_string();
                intel.why_here = "Cargo downloads crate source code from crates.io. This is the local cache of those downloads.".to_string();
                intel.reasoning = vec![
                    "Contains only downloaded crate sources".to_string(),
                    "Your Rust code is untouched".to_string(),
                    "Cargo re-downloads on demand".to_string(),
                    "Nothing unique or personal".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/target/debug") || path_str.contains("/target/release") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only compiled binaries".to_string(),
                    "✓ Source code (.rs files) is untouched".to_string(),
                    "✓ Rebuilds with cargo build".to_string(),
                    "✓ No personal data".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "1-5 minutes (cargo build)".to_string();
                intel.side_effects = "Next build will do a full compile instead of incremental".to_string();
                intel.why_here = "Rust compiles your code into binary files stored here. This is output, not source.".to_string();
                intel.reasoning = vec![
                    "Contains only compiled binaries".to_string(),
                    "Source code (.rs files) is untouched".to_string(),
                    "Rebuilds with cargo build".to_string(),
                    "No personal data".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/caches/") {
                let app_name_lower = name.to_lowercase().replace("com.", "").replace('.', " ");
                let app_running = state.is_app_running(&app_name_lower);
                let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(999);
                let signals = vec![!app_running, true, true, days > 7];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !app_running { "✓ App is not running".to_string() } else { "✗ App is currently running".to_string() },
                    "✓ Auto-generated cache — recreates on use".to_string(),
                    "✓ No personal files in cache data".to_string(),
                    if days > 7 { format!("✓ Not recently used ({} days)", days) } else { format!("✗ Recently used ({} days ago)", days) },
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Automatic (app rebuilds cache on use)".to_string();
                intel.side_effects = format!("{} may take a moment longer to start next time", name);
                intel.why_here = format!("{} stores temporary data here to load faster. It grows over time and is never automatically cleaned.", name);
                intel.reasoning = vec![
                    "Regenerates automatically".to_string(),
                    "No personal files detected".to_string(),
                    "No active processes using it".to_string(),
                    "Backup not required".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/logs/") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only historical text records".to_string(),
                    "✓ No impact on app functionality".to_string(),
                    "✓ New logs generated automatically".to_string(),
                    "✓ No personal data at risk".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Instant (new logs created automatically)".to_string();
                intel.side_effects = "You lose historical debugging info, which you likely don't need".to_string();
                intel.why_here = "Applications write log files to record what they do. These accumulate forever.".to_string();
                intel.reasoning = vec![
                    "Contains only historical text records".to_string(),
                    "No impact on app functionality".to_string(),
                    "New logs generated automatically".to_string(),
                    "No personal data at risk".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains(".trash") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ You already decided to delete these".to_string(),
                    "✓ No active use detected".to_string(),
                    "✓ Just occupying space unnecessarily".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Remove".to_string();
                intel.risk_level = "Low".to_string();
                intel.time_to_rebuild = "N/A — you already deleted these".to_string();
                intel.side_effects = "None — these files are already discarded".to_string();
                intel.why_here = "Files you dragged to trash stay here until you empty it, consuming disk space.".to_string();
                intel.reasoning = vec![
                    "You already decided to delete these".to_string(),
                    "No active use".to_string(),
                    "Just occupying space unnecessarily".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("cocoapods") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only cached library downloads".to_string(),
                    "✓ Podfile defines what to restore".to_string(),
                    "✓ Rebuilds with pod install".to_string(),
                    "✓ No personal data".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "1-2 minutes (pod install)".to_string();
                intel.side_effects = "Next pod install will download from network instead of cache".to_string();
                intel.why_here = "CocoaPods caches every iOS library you've ever used. This grows with each project.".to_string();
                intel.reasoning = vec![
                    "Contains only cached library downloads".to_string(),
                    "Podfile defines what to restore".to_string(),
                    "Rebuilds with pod install".to_string(),
                    "No personal data".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/.gradle/") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only build artifacts".to_string(),
                    "✓ Source code is separate".to_string(),
                    "✓ Re-downloads automatically".to_string(),
                    "✓ No personal data".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "2-5 minutes (gradle syncs on open)".to_string();
                intel.side_effects = "Android Studio will re-sync and re-download dependencies".to_string();
                intel.why_here = "Gradle caches compiled artifacts and downloaded dependencies for Java/Android projects.".to_string();
                intel.reasoning = vec![
                    "Contains only build artifacts".to_string(),
                    "Source code is separate".to_string(),
                    "Re-downloads automatically".to_string(),
                    "No personal data".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("__pycache__") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Auto-generated bytecode".to_string(),
                    "✓ Recreated instantly on run".to_string(),
                    "✓ No personal data".to_string(),
                    "✓ Zero impact on functionality".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Instant (Python recreates on run)".to_string();
                intel.side_effects = "None — Python regenerates these in milliseconds".to_string();
                intel.why_here = "Python pre-compiles scripts to bytecode for faster loading. These are auto-generated.".to_string();
                intel.reasoning = vec![
                    "Auto-generated bytecode".to_string(),
                    "Recreated instantly on run".to_string(),
                    "No personal data".to_string(),
                    "Zero impact on functionality".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/.build/") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only build output".to_string(),
                    "✓ Source code is separate".to_string(),
                    "✓ Rebuilds automatically".to_string(),
                    "✓ No personal data".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "1-3 minutes (swift build)".to_string();
                intel.side_effects = "Next build will be a full compile".to_string();
                intel.why_here = "Swift Package Manager stores compiled dependencies here.".to_string();
                intel.reasoning = vec![
                    "Contains only build output".to_string(),
                    "Source code is separate".to_string(),
                    "Rebuilds automatically".to_string(),
                    "No personal data".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("/.yarn/cache") || path_str.contains("/yarn/cache") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only cached packages".to_string(),
                    "✓ Yarn re-downloads on demand".to_string(),
                    "✓ No personal data".to_string(),
                    "✓ No effect on projects".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "10-30 seconds (yarn install)".to_string();
                intel.side_effects = "Next yarn install downloads from network".to_string();
                intel.why_here = "Yarn keeps offline copies of packages to speed up future installs.".to_string();
                intel.reasoning = vec![
                    "Contains only cached packages".to_string(),
                    "Yarn re-downloads on demand".to_string(),
                    "No personal data".to_string(),
                    "No effect on projects".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if path_str.contains("saved application state") {
                intel.confidence = 99;
                intel.evidence = vec![
                    "✓ Contains only UI state".to_string(),
                    "✓ Recreated instantly".to_string(),
                    "✓ No personal data".to_string(),
                    "✓ Zero functionality impact".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Instant (recreated on app launch)".to_string();
                intel.side_effects = "Apps open fresh instead of restoring last window state".to_string();
                intel.why_here = "macOS saves window positions and UI state for every app you use.".to_string();
                intel.reasoning = vec![
                    "Contains only UI state".to_string(),
                    "Recreated instantly".to_string(),
                    "No personal data".to_string(),
                    "Zero functionality impact".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else if category == "Applications" {
                let days = days_since_access(path).unwrap_or(0);
                let signals = vec![days > 180, true, true];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if days > 180 { format!("✓ Not opened in {} days", days) } else { format!("✗ Recently used ({} days ago)", days) },
                    "✓ Can be re-downloaded anytime".to_string(),
                    "✓ App data stored separately".to_string(),
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Remove".to_string();
                intel.risk_level = "Low".to_string();
                intel.time_to_rebuild = "Re-download from App Store or developer website".to_string();
                intel.side_effects = "The app will no longer be available until re-downloaded".to_string();
                intel.why_here = format!("You installed {} at some point. It takes up space whether you use it or not.", name);
                intel.reasoning = vec![
                    "Can be re-downloaded anytime".to_string(),
                    "Not recently used".to_string(),
                    "App data may be stored separately".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            } else {
                let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(999);
                let signals = vec![true, true, days > 7];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    "✓ Regenerates automatically".to_string(),
                    "✓ No personal files detected".to_string(),
                    if days > 7 { format!("✓ Not modified in {} days", days) } else { "✗ Recently modified".to_string() },
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "None".to_string();
                intel.time_to_rebuild = "Automatic".to_string();
                intel.side_effects = "None expected".to_string();
                intel.why_here = format!("{} stores cached data here that accumulates over time.", name);
                intel.reasoning = vec![
                    "Regenerates automatically".to_string(),
                    "No personal files detected".to_string(),
                    "Safe to remove".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data regenerates automatically, contains no personal files, and has no active dependencies. Cleaning reclaims space with zero permanent loss.".to_string();
                intel.what_if_wrong = "If our analysis is wrong, the Recovery Vault keeps this data for 14 days. You can restore it with one click.".to_string();
            }
        }
        SafetyLevel::Review => {
            if category == "Downloads" {
                let days = days_since_access(path).unwrap_or(0);
                let signals = vec![days > 90, true];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if days > 90 { format!("✓ Not opened in {} days", days) } else { format!("✗ Recently accessed ({} days ago)", days) },
                    "✗ Cannot be auto-regenerated".to_string(),
                ];
                intel.recommendation = "Review First".to_string();
                intel.action_label = "Remove".to_string();
                intel.risk_level = "Medium".to_string();
                intel.time_to_rebuild = "Re-download from original source if available".to_string();
                intel.side_effects = "File is permanently removed — check if you still need it".to_string();
                intel.why_here = "You downloaded this file from the internet. Downloads accumulate and are often forgotten.".to_string();
                intel.reasoning = vec![
                    "Downloaded file — may be one-time use".to_string(),
                    "Check if you still need it".to_string(),
                    "Cannot be auto-regenerated".to_string(),
                ];
                intel.why_recommended = "This appears to be inactive personal data. RepairIQ recommends reviewing before removal, because this content may not be re-downloadable.".to_string();
                intel.what_if_wrong = "If you still need this data, it remains in the Recovery Vault. Nothing is permanently deleted without your explicit confirmation.".to_string();
            } else if path_str.contains("coresimulator") {
                let xcode_running = state.is_app_running("xcode");
                let signals = vec![!xcode_running, true, true];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !xcode_running { "✓ Xcode is not running".to_string() } else { "✗ Xcode is currently open".to_string() },
                    "✓ Re-downloadable from Apple".to_string(),
                    "✓ No personal project data".to_string(),
                ];
                intel.recommendation = "Review First".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = "Low".to_string();
                intel.time_to_rebuild = "10-20 minutes (re-downloads from Apple)".to_string();
                intel.side_effects = "Simulator runtimes need to be re-downloaded before iOS testing".to_string();
                intel.why_here = "Xcode downloads iOS simulator runtimes (5-8GB each) to test apps on virtual devices.".to_string();
                intel.reasoning = vec![
                    "Large but re-downloadable".to_string(),
                    "Only needed for iOS development".to_string(),
                    "Takes time to re-download".to_string(),
                ];
                intel.why_recommended = "RepairIQ verified this data can be re-downloaded but requires significant time. Review whether you actively use iOS simulators.".to_string();
                intel.what_if_wrong = "If you still need this data, it remains in the Recovery Vault. Nothing is permanently deleted without your explicit confirmation.".to_string();
            } else if path_str.contains("/application support/") {
                let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(0);
                let app_name_lower = name.to_lowercase().replace("com.", "").replace('.', " ");
                let app_running = state.is_app_running(&app_name_lower);
                let signals = vec![!app_running, days > 30];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !app_running { "✓ App is not running".to_string() } else { "✗ App is currently running".to_string() },
                    if days > 30 { format!("✓ Not modified in {} days", days) } else { format!("✗ Recently modified ({} days ago)", days) },
                ];
                intel.recommendation = "Review First".to_string();
                intel.action_label = "Remove".to_string();
                intel.risk_level = "Medium".to_string();
                intel.time_to_rebuild = "App will need to be reconfigured and logged in again".to_string();
                intel.side_effects = "App resets to fresh state — logins and preferences lost".to_string();
                intel.why_here = format!("{} stores settings, login sessions, and local data here.", name);
                intel.reasoning = vec![
                    "May contain login sessions".to_string(),
                    "App preferences will be lost".to_string(),
                    "Review if app is still used".to_string(),
                ];
                intel.why_recommended = "This appears to be inactive personal data. RepairIQ recommends reviewing before removal, because this content contains app settings that cannot be auto-regenerated.".to_string();
                intel.what_if_wrong = "If you still need this data, it remains in the Recovery Vault. Nothing is permanently deleted without your explicit confirmation.".to_string();
            } else {
                let days = scan_result.and_then(|r| r.most_recent_days).unwrap_or(0);
                let signals = vec![days > 30];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if days > 30 { format!("✓ Not modified in {} days", days) } else { "✗ Recently modified".to_string() },
                    "✗ Purpose not fully determined".to_string(),
                ];
                intel.recommendation = "Review First".to_string();
                intel.action_label = "Remove".to_string();
                intel.risk_level = "Medium".to_string();
                intel.time_to_rebuild = "Varies — review contents first".to_string();
                intel.side_effects = "Unknown — inspect before removing".to_string();
                intel.why_here = format!("{} may contain important data. Review before deciding.", name);
                intel.reasoning = vec![
                    "Purpose not fully determined".to_string(),
                    "Review contents before removing".to_string(),
                    "Use Recovery Vault for safety".to_string(),
                ];
                intel.why_recommended = "RepairIQ could not fully determine the purpose of this item. Manual review recommended before removal.".to_string();
                intel.what_if_wrong = "If you still need this data, it remains in the Recovery Vault. Nothing is permanently deleted without your explicit confirmation.".to_string();
            }
        }
    }

    intel
}
// ============================================================
// PARALLEL SCAN — uses rayon for directory-level parallelism
// ============================================================

/// Scan a specific directory and return items — PARALLEL per entry
fn scan_directory(base_path: &Path, category: &str, subcategory: &str, state: &SystemState) -> Vec<ScannedItem> {
    if !base_path.exists() {
        return Vec::new();
    }

    let entries: Vec<_> = match fs::read_dir(base_path) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(_) => return Vec::new(),
    };

    // Separate entries into directories and small files for grouping
    let mut dir_entries: Vec<fs::DirEntry> = Vec::new();
    let mut small_files_by_type: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

    for entry in entries {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Skip hidden files unless they're special
        if name.starts_with('.')
            && !name.starts_with(".Trash")
            && !name.starts_with(".docker")
            && category != "Developer"
        {
            continue;
        }

        if path.is_dir() {
            dir_entries.push(entry);
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size < 1_048_576 {
                // Group small files by extension
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_else(|| "other".to_string());
                small_files_by_type.entry(ext).or_default().push((path, size));
            } else {
                // Large files get treated like directories
                dir_entries.push(entry);
            }
        }
    }

    // Process directories/large files in PARALLEL using rayon
    let mut items: Vec<ScannedItem> = dir_entries
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Single-pass walk for directories
            let (size_bytes, file_count, largest_files_raw, scan_result) = if path.is_dir() {
                let result = walk_directory_once(&path);
                let size = result.total_size;
                let fc = result.file_count;
                let lf = result.largest_files.clone();
                (size, fc, lf, Some(result))
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                (size, 1u64, vec![], None)
            };

            // Skip items < 1MB
            if size_bytes < 1_048_576 {
                return None;
            }

            let intel = analyze_path(&path, category, state, scan_result.as_ref());
            let intel = fill_advisor_fields(intel, &path, category, state, scan_result.as_ref());
            let last_accessed_days = days_since_access(&path);
            let largest_files = format_largest_files(&largest_files_raw);

            Some(ScannedItem {
                path: path.to_string_lossy().to_string(),
                name,
                size_bytes,
                category: category.to_string(),
                subcategory: subcategory.to_string(),
                safety: intel.safety,
                safety_score: intel.safety_score,
                last_accessed_days,
                description: intel.description,
                impact: intel.impact,
                recovery_method: intel.recovery_method,
                owner: intel.owner,
                verdict: intel.verdict,
                verdict_reason: intel.verdict_reason,
                file_count,
                largest_files,
                depends_on: intel.depends_on,
                clean_command: intel.clean_command,
                recommendation: intel.recommendation,
                action_label: intel.action_label,
                risk_level: intel.risk_level,
                time_to_rebuild: intel.time_to_rebuild,
                side_effects: intel.side_effects,
                why_here: intel.why_here,
                reasoning: intel.reasoning,
                confidence: intel.confidence,
                evidence: intel.evidence,
                why_recommended: intel.why_recommended,
                what_if_wrong: intel.what_if_wrong,
            })
        })
        .collect();

    // Now create grouped items for small files (only if group total > 5MB)
    for (ext, files) in &small_files_by_type {
        let total_size: u64 = files.iter().map(|(_, s)| *s).sum();
        let count = files.len();

        if total_size < 5_242_880 {
            continue;
        }

        let type_name = match ext.as_str() {
            "png" | "jpg" | "jpeg" | "heic" | "webp" | "gif" | "tiff" => "Screenshots & Images",
            "pdf" => "PDF Documents",
            "mp4" | "mov" | "avi" | "mkv" => "Videos",
            "mp3" | "wav" | "flac" | "m4a" | "aac" => "Audio Files",
            "zip" | "gz" | "tar" | "rar" | "7z" | "dmg" => "Archives & Installers",
            "doc" | "docx" | "xls" | "xlsx" | "pptx" => "Office Documents",
            "txt" | "md" | "rtf" => "Text Files",
            _ => "Other Files",
        };

        let group_name = format!("{} ({} .{} files)", type_name, count, ext);
        let group_path = base_path.to_string_lossy().to_string();

        let (safety, score, verdict, reason) = match ext.as_str() {
            "png" | "jpg" | "jpeg" | "heic" | "webp" | "gif" | "tiff" => {
                if category == "Desktop" || category == "Downloads" {
                    (SafetyLevel::Review, 6u8,
                     format!("⚠️ REVIEW — {} {} files on your {}", count, ext, category),
                     format!("These are {} image files totaling {}. Many may be screenshots you no longer need. Review before bulk-deleting.", count, format_size(total_size)))
                } else {
                    (SafetyLevel::Review, 5,
                     "⚠️ REVIEW — Check if you need these".to_string(),
                     format!("{} image files. May include personal photos.", count))
                }
            }
            "zip" | "gz" | "tar" | "rar" | "7z" | "dmg" => {
                (SafetyLevel::Safe, 8,
                 format!("✅ YES — {} old archives/installers", count),
                 "These are compressed archives and installer packages. Usually one-time-use files you've already extracted or installed.".to_string())
            }
            "mp4" | "mov" | "avi" | "mkv" => {
                (SafetyLevel::Review, 4,
                 format!("⚠️ REVIEW — {} video files ({})", count, format_size(total_size)),
                 "Video files may be personal recordings or downloads. Check before removing.".to_string())
            }
            "mp3" | "wav" | "flac" | "m4a" | "aac" => {
                (SafetyLevel::Review, 4,
                 format!("⚠️ REVIEW — {} audio files ({})", count, format_size(total_size)),
                 "Audio files may be personal music or recordings. Check before removing.".to_string())
            }
            "pdf" => {
                (SafetyLevel::Review, 5,
                 format!("⚠️ REVIEW — {} PDF documents", count),
                 "PDF documents may contain important information. Review before removing.".to_string())
            }
            _ => {
                (SafetyLevel::Review, 5,
                 format!("⚠️ REVIEW — {} .{} files", count, ext),
                 format!("{} files of type .{}. Review contents.", count, ext))
            }
        };

        let description = format!("{} .{} files totaling {}", count, ext, format_size(total_size));

        let examples: Vec<String> = files
            .iter()
            .take(5)
            .map(|(p, s)| {
                let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                format!("{} — {}", fname, format_size(*s))
            })
            .collect();

        items.push(ScannedItem {
            path: group_path,
            name: group_name,
            size_bytes: total_size,
            category: category.to_string(),
            subcategory: subcategory.to_string(),
            safety: safety.clone(),
            safety_score: score,
            last_accessed_days: None,
            description,
            impact: if safety == SafetyLevel::Safe {
                "None. These are typically one-time-use files.".to_string()
            } else {
                "Check if any of these files contain personal data you want to keep.".to_string()
            },
            recovery_method: "Cannot be recovered unless backed up. Use Recovery Vault.".to_string(),
            owner: "You".to_string(),
            verdict: verdict.clone(),
            verdict_reason: reason,
            file_count: count as u64,
            largest_files: examples,
            depends_on: vec![],
            clean_command: String::new(),
            recommendation: if score >= 8 { "Clean".to_string() } else { "Review First".to_string() },
            action_label: if score >= 8 { "Remove".to_string() } else { "Review".to_string() },
            risk_level: if score >= 8 { "None".to_string() } else { "Low".to_string() },
            time_to_rebuild: "N/A — cannot be rebuilt".to_string(),
            side_effects: "Files are permanently removed (use Recovery Vault for safety)".to_string(),
            why_here: format!("You have {} .{} files in your {} folder. These accumulated over time from screenshots, downloads, or app exports.", count, ext, category),
            reasoning: if score >= 8 {
                vec!["Typically one-time-use files".to_string(), "Archives/installers already used".to_string(), format!("Total: {} across {} files", format_size(total_size), count)]
            } else {
                vec!["May contain personal data".to_string(), "Review before removing".to_string(), format!("Total: {} across {} files", format_size(total_size), count)]
            },
            confidence: if score >= 8 { 85 } else { 60 },
            evidence: if score >= 8 {
                vec!["✓ Typically disposable file type".to_string(), format!("✓ {} files, likely accumulated over time", count)]
            } else {
                vec![format!("⚠ {} files — may include personal content", count), "⚠ Review recommended before batch removal".to_string()]
            },
            why_recommended: if score >= 8 {
                "These are typically installer packages or archives you've already used. They take up space with no ongoing purpose.".to_string()
            } else {
                format!("You have {} .{} files that collectively use {}. Review them to decide which to keep.", count, ext, format_size(total_size))
            },
            what_if_wrong: "Recovery Vault keeps these for 14 days. You can restore any of them with one click.".to_string(),
        });
    }

    items
}

// ============================================================
// PUBLIC API — same interface as before
// ============================================================

/// Public wrappers for lib.rs drill-down
pub fn classify_item(path: &Path) -> SafetyLevel {
    let state = SystemState::detect();
    analyze_path(path, "", &state, None).safety
}

pub fn describe_item(path: &Path) -> String {
    let state = SystemState::detect();
    analyze_path(path, "", &state, None).description
}

pub fn get_item_intel(
    path: &Path,
    category: &str,
) -> (
    SafetyLevel, u8, String, String, String, String, String, String,
    Vec<String>, String, String, String, String, String, String, String,
    Vec<String>, u8, Vec<String>, String, String,
) {
    let state = SystemState::detect();
    let intel = fill_advisor_fields(
        analyze_path(path, category, &state, None),
        path,
        category,
        &state,
        None,
    );
    (
        intel.safety,
        intel.safety_score,
        intel.description,
        intel.impact,
        intel.recovery_method,
        intel.owner,
        intel.verdict,
        intel.verdict_reason,
        intel.depends_on,
        intel.clean_command,
        intel.recommendation,
        intel.action_label,
        intel.risk_level,
        intel.time_to_rebuild,
        intel.side_effects,
        intel.why_here,
        intel.reasoning,
        intel.confidence,
        intel.evidence,
        intel.why_recommended,
        intel.what_if_wrong,
    )
}

// ============================================================
// HEALTH SCORE CALCULATION
// ============================================================

fn calculate_health_score(result: &ScanResult, total: u64, free: u64) -> (u8, String, Vec<String>) {
    let mut score: i32 = 100;
    let mut factors = Vec::new();

    let free_pct = if total > 0 {
        (free as f64 / total as f64 * 100.0) as u32
    } else {
        100
    };

    if free_pct < 5 {
        score -= 40;
        factors.push("Critical: Less than 5% free space".to_string());
    } else if free_pct < 10 {
        score -= 25;
        factors.push("Low: Less than 10% free space".to_string());
    } else if free_pct < 20 {
        score -= 10;
        factors.push("Below recommended 20% free space".to_string());
    } else {
        factors.push("✓ Healthy free space level".to_string());
    }

    // Safe recovery as % of total
    let safe_pct = if total > 0 {
        (result.safe_recovery_bytes as f64 / total as f64 * 100.0) as u32
    } else {
        0
    };
    if safe_pct > 15 {
        score -= 20;
        factors.push(format!("{}% of disk is cleanable cache data", safe_pct));
    } else if safe_pct > 5 {
        score -= 10;
        factors.push(format!("{}% cleanable data accumulated", safe_pct));
    } else {
        factors.push("✓ Minimal cleanable data".to_string());
    }

    // Archive items (old stuff hanging around)
    let archive_count = result.items.iter().filter(|i| i.safety == SafetyLevel::Archive).count();
    if archive_count > 5 {
        score -= 10;
        factors.push(format!("{} items should be archived", archive_count));
    } else {
        factors.push("✓ No stale items detected".to_string());
    }

    let score = score.max(0).min(100) as u8;
    let grade = match score {
        95..=100 => "A+",
        85..=94 => "A",
        70..=84 => "B",
        55..=69 => "C",
        40..=54 => "D",
        _ => "F",
    }.to_string();

    (score, grade, factors)
}

/// Main scan function — PARALLEL across all targets
pub fn perform_scan() -> ScanResult {
    let start = std::time::Instant::now();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));

    let (total_bytes, used_bytes, free_bytes) = get_disk_info();

    // System state checked ONCE
    let state = SystemState::detect();

    let scan_targets = get_scan_targets(&home);


    // Scan all targets in PARALLEL
    let all_items: Vec<ScannedItem> = scan_targets
        .par_iter()
        .flat_map(|(category, subcategory, path)| {
            scan_directory(path, category, subcategory, &state)
        })
        .collect();

    let safe_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Safe)
        .map(|i| i.size_bytes)
        .sum();

    let review_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Review)
        .map(|i| i.size_bytes)
        .sum();

    let archive_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Archive)
        .map(|i| i.size_bytes)
        .sum();

    let mut category_map: HashMap<String, Vec<ScannedItem>> = HashMap::new();
    for item in &all_items {
        category_map
            .entry(item.category.clone())
            .or_default()
            .push(item.clone());
    }

    let mut categories: Vec<CategoryBreakdown> = category_map
        .into_iter()
        .map(|(name, items)| {
            let size_bytes = items.iter().map(|i| i.size_bytes).sum();
            CategoryBreakdown { name, size_bytes, items }
        })
        .collect();

    categories.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let mut all_items = all_items;
    all_items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let scan_duration_ms = start.elapsed().as_millis() as u64;

    let mut result = ScanResult {
        total_bytes,
        used_bytes,
        free_bytes,
        safe_recovery_bytes,
        review_recovery_bytes,
        archive_recovery_bytes,
        categories,
        items: all_items,
        scan_duration_ms,
        health_score: 0,
        health_grade: String::new(),
        health_factors: Vec::new(),
    };

    let (score, grade, factors) = calculate_health_score(&result, total_bytes, free_bytes);
    result.health_score = score;
    result.health_grade = grade;
    result.health_factors = factors;

    result
}

/// Scan with progress reporting — emits progress messages via callback
pub fn perform_scan_with_progress(progress: impl Fn(String)) -> ScanResult {
    let start = std::time::Instant::now();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));

    let (total_bytes, used_bytes, free_bytes) = get_disk_info();

    progress("Checking system state...".to_string());
    let state = SystemState::detect();

    let scan_targets = get_scan_targets(&home);


    let total_targets = scan_targets.len();
    let mut all_items: Vec<ScannedItem> = Vec::new();

    // Sequential progress reporting, but parallel within each target
    for (i, (category, subcategory, path)) in scan_targets.iter().enumerate() {
        progress(format!("Scanning {}... ({}/{})", category, i + 1, total_targets));
        let items = scan_directory(path, category, subcategory, &state);
        all_items.extend(items);
    }

    progress("Calculating results...".to_string());

    let safe_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Safe)
        .map(|i| i.size_bytes)
        .sum();

    let review_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Review)
        .map(|i| i.size_bytes)
        .sum();

    let archive_recovery_bytes: u64 = all_items
        .iter()
        .filter(|i| i.safety == SafetyLevel::Archive)
        .map(|i| i.size_bytes)
        .sum();

    let mut category_map: HashMap<String, Vec<ScannedItem>> = HashMap::new();
    for item in &all_items {
        category_map
            .entry(item.category.clone())
            .or_default()
            .push(item.clone());
    }

    let mut categories: Vec<CategoryBreakdown> = category_map
        .into_iter()
        .map(|(name, items)| {
            let size_bytes = items.iter().map(|i| i.size_bytes).sum();
            CategoryBreakdown { name, size_bytes, items }
        })
        .collect();

    categories.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    all_items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let scan_duration_ms = start.elapsed().as_millis() as u64;

    progress(format!("Done! Scanned in {}ms", scan_duration_ms));

    let mut result = ScanResult {
        total_bytes,
        used_bytes,
        free_bytes,
        safe_recovery_bytes,
        review_recovery_bytes,
        archive_recovery_bytes,
        categories,
        items: all_items,
        scan_duration_ms,
        health_score: 0,
        health_grade: String::new(),
        health_factors: Vec::new(),
    };

    let (score, grade, factors) = calculate_health_score(&result, total_bytes, free_bytes);
    result.health_score = score;
    result.health_grade = grade;
    result.health_factors = factors;

    result
}
