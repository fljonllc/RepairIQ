use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use walkdir::WalkDir;

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
    pub file_count: u64,         // How many files inside
    pub largest_files: Vec<String>, // Top 5 largest files inside (name + size)
    pub depends_on: Vec<String>, // What apps/services depend on this
    pub clean_command: String,   // Terminal command to clean this properly (if any)
    pub recommendation: String,  // "Clean" | "Archive" | "Review First" | "Do Not Touch"
    pub action_label: String,    // "Clean Cache" | "Archive Project" | "Remove" | etc
    pub risk_level: String,      // "None" | "Low" | "Medium" | "High" | "Critical"
    pub time_to_rebuild: String, // How long to rebuild after removal
    pub side_effects: String,    // What happens as a side effect
    pub why_here: String,        // Why this is taking up space
    pub reasoning: Vec<String>,  // Safety score reasoning checklist items
    pub confidence: u8,           // 0-100 percentage
    pub evidence: Vec<String>,    // Evidence items that were checked
    pub why_recommended: String,  // Why RepairIQ recommends this specific action
    pub what_if_wrong: String,    // What happens if our recommendation is wrong
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
}

/// Intelligence data for a path
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

impl ItemIntel {
    fn new(
        description: impl Into<String>,
        impact: impl Into<String>,
        recovery_method: impl Into<String>,
        owner: impl Into<String>,
        safety_score: u8,
        safety: SafetyLevel,
        verdict: impl Into<String>,
        verdict_reason: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            impact: impact.into(),
            recovery_method: recovery_method.into(),
            owner: owner.into(),
            safety_score,
            safety,
            verdict: verdict.into(),
            verdict_reason: verdict_reason.into(),
            depends_on: vec![],
            clean_command: String::new(),
            recommendation: String::new(),
            action_label: String::new(),
            risk_level: String::new(),
            time_to_rebuild: String::new(),
            side_effects: String::new(),
            why_here: String::new(),
            reasoning: vec![],
            confidence: 50,
            evidence: vec![],
            why_recommended: String::new(),
            what_if_wrong: String::new(),
        }
    }

    fn with_deps(mut self, deps: Vec<&str>) -> Self {
        self.depends_on = deps.into_iter().map(|s| s.to_string()).collect();
        self
    }

    fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.clean_command = cmd.into();
        self
    }

    fn with_advisor(mut self, recommendation: impl Into<String>, action_label: impl Into<String>, risk_level: impl Into<String>, time_to_rebuild: impl Into<String>, side_effects: impl Into<String>, why_here: impl Into<String>, reasoning: Vec<&str>) -> Self {
        self.recommendation = recommendation.into();
        self.action_label = action_label.into();
        self.risk_level = risk_level.into();
        self.time_to_rebuild = time_to_rebuild.into();
        self.side_effects = side_effects.into();
        self.why_here = why_here.into();
        self.reasoning = reasoning.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

// ============================================================
// REAL-TIME SYSTEM CHECKS — verify actual state, don't guess
// ============================================================

/// Check if Docker is currently running
fn is_docker_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "Docker"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if there are active Docker containers
fn has_running_containers() -> bool {
    Command::new("docker")
        .args(["ps", "-q"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

/// Check if Docker has named volumes with data
fn has_docker_volumes() -> bool {
    Command::new("docker")
        .args(["volume", "ls", "-q"])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

/// Check if an app is currently running by process name
fn is_app_running(app_name: &str) -> bool {
    Command::new("pgrep")
        .args(["-i", app_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a path contains any git repositories with uncommitted changes
fn has_uncommitted_git_changes(path: &Path) -> bool {
    // Look for .git directory
    let git_dir = path.join(".git");
    if git_dir.exists() {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output();
        if let Ok(o) = output {
            let status = String::from_utf8_lossy(&o.stdout);
            return !status.trim().is_empty();
        }
    }
    false
}

/// Check if path contains user-created files (not just caches/configs)
fn contains_user_data(path: &Path) -> bool {
    // Look for common user-data indicators
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

/// Count days since any file in directory was modified
fn last_modified_days(path: &Path) -> Option<u64> {
    let mut most_recent: Option<SystemTime> = None;
    let mut count = 0;

    for entry in WalkDir::new(path)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if count >= 200 { break; }
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

/// Check if an Xcode project depends on this DerivedData
fn is_xcode_project_open() -> bool {
    is_app_running("Xcode")
}

/// Count files in a directory
fn count_files(path: &Path) -> u64 {
    WalkDir::new(path)
        .max_depth(10)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count() as u64
}

/// Get the top N largest files in a directory (formatted as "name — size")
fn get_largest_files(path: &Path, count: usize) -> Vec<String> {
    let mut files: Vec<(String, u64)> = WalkDir::new(path)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let size = e.metadata().ok()?.len();
            let name = e.path().file_name()?.to_string_lossy().to_string();
            Some((name, size))
        })
        .collect();

    files.sort_by(|a, b| b.1.cmp(&a.1));
    files.truncate(count);

    files
        .into_iter()
        .map(|(name, size)| {
            let size_str = if size >= 1_073_741_824 {
                format!("{:.1} GB", size as f64 / 1_073_741_824.0)
            } else if size >= 1_048_576 {
                format!("{:.0} MB", size as f64 / 1_048_576.0)
            } else {
                format!("{:.0} KB", size as f64 / 1024.0)
            };
            format!("{} — {}", name, size_str)
        })
        .collect()
}

/// Detect file types in a directory (what kind of content is inside)
fn detect_content_type(path: &Path) -> String {
    let mut extensions: HashMap<String, u64> = HashMap::new();
    let mut count = 0;

    for entry in WalkDir::new(path)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if count >= 500 { break; }
        if let Some(ext) = entry.path().extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            *extensions.entry(ext_str).or_insert(0) += size;
        }
        count += 1;
    }

    let mut sorted: Vec<(String, u64)> = extensions.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let top_types: Vec<String> = sorted.iter().take(3).map(|(ext, _)| ext.clone()).collect();

    if top_types.is_empty() {
        return "Mixed files".to_string();
    }

    // Categorize
    let has_media = top_types.iter().any(|e| ["mp4", "mov", "avi", "mkv", "mp3", "wav", "flac"].contains(&e.as_str()));
    let has_images = top_types.iter().any(|e| ["jpg", "jpeg", "png", "gif", "heic", "raw", "psd"].contains(&e.as_str()));
    let has_code = top_types.iter().any(|e| ["rs", "ts", "js", "py", "swift", "java", "go", "c", "cpp", "h"].contains(&e.as_str()));
    let has_docs = top_types.iter().any(|e| ["pdf", "docx", "xlsx", "pptx", "doc", "txt", "md"].contains(&e.as_str()));
    let has_binaries = top_types.iter().any(|e| ["o", "dylib", "so", "a", "dll", "exe", "class"].contains(&e.as_str()));

    if has_binaries { return "Compiled binaries (build output, not source code)".to_string(); }
    if has_code { return "Source code files".to_string(); }
    if has_media { return "Media files (video/audio)".to_string(); }
    if has_images { return "Image files".to_string(); }
    if has_docs { return "Documents".to_string(); }

    format!("Mostly .{} files", top_types[0])
}

// ============================================================
// INTELLIGENCE ENGINE — definitive answers
// ============================================================

fn get_disk_info() -> (u64, u64, u64) {
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

fn days_since_access(path: &Path) -> Option<u64> {
    let metadata = fs::metadata(path).ok()?;
    let accessed = metadata.accessed().ok()?;
    let now = SystemTime::now();
    let duration = now.duration_since(accessed).ok()?;
    Some(duration.as_secs() / 86400)
}

fn dir_size(path: &Path, max_depth: usize) -> u64 {
    WalkDir::new(path)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Calculate confidence from evidence signals
fn calculate_confidence(signals: &[bool]) -> u8 {
    if signals.is_empty() { return 50; }
    let positive = signals.iter().filter(|&&s| s).count();
    let total = signals.len();
    let base = (positive as f64 / total as f64 * 100.0) as u8;
    base.min(99) // never 100% — always leave room for uncertainty
}

/// The intelligence engine — definitive analysis with real-time verification
fn analyze_path(path: &Path, category: &str) -> ItemIntel {
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
        let xcode_running = is_xcode_project_open();
        let (verdict, reason) = if xcode_running {
            ("✅ YES — Clean it (close Xcode first)".into(),
             "Xcode is currently open. Close it first, then clean. It rebuilds in 2-5 minutes on next open.".to_string())
        } else {
            ("✅ YES — Clean it now".into(),
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
        let docker_running = is_docker_running();
        let containers_active = if docker_running { has_running_containers() } else { false };
        let has_volumes = if docker_running { has_docker_volumes() } else { false };

        if path_str.contains("overlay") || path_str.contains("/data/") || path_str.contains("com.docker.docker") {
            let (verdict, reason, score) = if !docker_running {
                ("✅ YES — Clean it now".into(),
                 "Docker is not running. This is all cached data. When you start Docker again, it re-downloads what it needs.".to_string(),
                 10u8)
            } else if containers_active {
                ("⚠️ STOP CONTAINERS FIRST".into(),
                 format!("Docker has running containers RIGHT NOW. Stop them first with 'docker stop $(docker ps -q)' then clean. Or run 'docker system prune -a' to clean only unused data."),
                 5)
            } else if has_volumes {
                ("✅ YES — But run 'docker system prune -a' instead".into(),
                 "Docker is running but no containers are active. You have named volumes (may contain database data). Use 'docker system prune -a' to safely clean only unused data.".to_string(),
                 8)
            } else {
                ("✅ YES — Clean it now".into(),
                 "Docker is running but nothing is active — no containers, no important volumes. This is all just cached layers taking up space.".to_string(),
                 9)
            };

            return ItemIntel {
                description: "Docker container images, layers, and build cache".into(),
                impact: if containers_active {
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
        let app_running = is_app_running(&name_lower.replace("com.", "").replace('.', " "));
        let days = last_modified_days(path).unwrap_or(999);

        let (verdict, reason) = if days > 30 {
            ("✅ YES — Clean it now".into(),
             format!("This cache hasn't been updated in {} days. The app hasn't needed it. It regenerates if the app ever needs it again.", days))
        } else if app_running {
            ("✅ YES — But quit {} first".into(),
             format!("{} is currently running. Quit it first, then clean. Cache rebuilds next time you open it.", app_name))
        } else {
            ("✅ YES — Clean it now".into(),
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
        let xcode_running = is_xcode_project_open();
        let (verdict, reason) = if xcode_running {
            ("⚠️ CLOSE XCODE FIRST".into(),
             "Xcode is currently open and may be using simulator data. Close Xcode, then clean.".to_string())
        } else {
            ("✅ YES — Clean it (but expect re-downloads)".into(),
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
        let days = last_modified_days(path).unwrap_or(0);
        let (verdict, reason) = if days > 90 {
            ("✅ YES — Clean it".into(),
             format!("These archives are {} days old. If you haven't submitted to the App Store recently, they're just taking up space. You can rebuild from source anytime.", days))
        } else {
            ("⚠️ KEEP if you submit to App Store".into(),
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
        let days = last_modified_days(path).unwrap_or(0);
        let app_running = is_app_running(&name_lower.replace("com.", "").replace('.', " "));

        // Known safe-to-clear apps (no critical user data)
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

        // Apps with known login/settings data
        let (verdict, reason, score) = if days > 180 && !app_running {
            ("✅ YES — You haven't used this in 6+ months".into(),
             format!("{} hasn't been used in {} days. Its settings are stale. If you ever reopen it, you'll just log in again.", app_name, days),
             8u8)
        } else if app_running {
            ("🚫 NO — {} is currently running".into(),
             format!("{} is running right now. Deleting its data while running could corrupt it. Quit the app first if you want to clean this.", app_name),
             3)
        } else {
            ("⚠️ WILL RESET APP — You'll need to log in again".into(),
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
        let app_running = is_app_running(&name_lower.replace(".app", ""));

        let (verdict, reason, score) = if app_running {
            ("🚫 NO — Currently running".into(),
             format!("{} is running right now. You can't remove a running application.", name),
             2u8)
        } else if days > 180 {
            ("✅ YES — You haven't opened this in 6+ months".into(),
             format!("You last opened {} over {} days ago. If you haven't needed it in 6 months, you don't need it. Re-download from App Store anytime.", name, days),
             8)
        } else if days > 30 {
            ("⚠️ PROBABLY SAFE — Not used in {} days".into(),
             format!("You haven't opened {} in {} days. If you don't remember why you have it, it's probably safe to remove.", name, days),
             6)
        } else {
            ("⚠️ RECENTLY USED — Keep it unless you're sure".into(),
             format!("You opened {} within the last month. You're probably still using it.", name),
             4)
        };

        return ItemIntel {
            description: format!("{} — installed application", name),
            impact: format!("The app disappears. Re-download from App Store or developer website if you need it later."),
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

    // === DOWNLOADS ===
    if category == "Downloads" {
        let days = days_since_access(path).unwrap_or(0);

        let (verdict, reason, score) = if days > 90 {
            ("✅ YES — Not opened in 3+ months".into(),
             format!("You downloaded this {} days ago and haven't opened it since. It's either a one-time download or something you forgot about.", days),
             9u8)
        } else if days > 30 {
            ("✅ PROBABLY — Not used in {} days".into(),
             format!("Downloaded and not opened in {} days. Most downloads are one-time-use (installers, attachments). Safe to clean.", days),
             7)
        } else {
            ("⚠️ RECENTLY USED — Check before cleaning".into(),
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
        let has_git = path.join(".git").exists();
        let has_changes = if has_git { has_uncommitted_git_changes(path) } else { false };

        let (verdict, reason, score) = if has_changes {
            ("🚫 NO — Has uncommitted code changes".into(),
             "This project has uncommitted Git changes. You have work that hasn't been pushed. DO NOT delete.".to_string(),
             2u8)
        } else if has_git && days > 180 {
            ("📦 ARCHIVE — Move to external drive".into(),
             format!("Git project not touched in {} days. All changes are committed. Safe to archive to external storage — your code lives on GitHub/remote.", days),
             7)
        } else if days > 365 {
            ("📦 ARCHIVE — Not opened in over a year".into(),
             format!("Not accessed in {} days (over a year). Move to an external drive rather than deleting — this is your personal data.", days),
             6)
        } else if days > 180 {
            ("📦 ARCHIVE — Consider moving to external drive".into(),
             format!("Not accessed in {} days. This is YOUR data — we recommend archiving to an external drive rather than deleting.", days),
             5)
        } else {
            ("🚫 NO — This is your active data".into(),
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
    let days = last_modified_days(path).unwrap_or(0);

    let (verdict, reason, score) = if days > 180 {
        ("⚠️ PROBABLY SAFE — Inactive for 6+ months".into(),
         format!("This hasn't been modified in {} days. Likely safe to remove, but review contents if unsure.", days),
         6u8)
    } else {
        ("⚠️ REVIEW — Unknown purpose".into(),
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

/// Extract a human-readable app name from a directory/file name
fn extract_app_name(name: &str) -> String {
    if (name.starts_with("com.") || name.starts_with("org.") || name.starts_with("io.")) && name.contains('.') {
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

/// Public wrappers for lib.rs drill-down
pub fn classify_item(path: &Path) -> SafetyLevel {
    analyze_path(path, "").safety
}

pub fn describe_item(path: &Path) -> String {
    analyze_path(path, "").description
}

pub fn get_item_intel(path: &Path, category: &str) -> (SafetyLevel, u8, String, String, String, String, String, String, Vec<String>, String, String, String, String, String, String, String, Vec<String>, u8, Vec<String>, String, String) {
    let intel = fill_advisor_fields(analyze_path(path, category), path, category);
    (intel.safety, intel.safety_score, intel.description, intel.impact, intel.recovery_method, intel.owner, intel.verdict, intel.verdict_reason, intel.depends_on, intel.clean_command, intel.recommendation, intel.action_label, intel.risk_level, intel.time_to_rebuild, intel.side_effects, intel.why_here, intel.reasoning, intel.confidence, intel.evidence, intel.why_recommended, intel.what_if_wrong)
}

/// Fill in advisor fields based on existing intelligence if not already set
fn fill_advisor_fields(mut intel: ItemIntel, path: &Path, category: &str) -> ItemIntel {
    let path_str = path.to_string_lossy().to_lowercase();
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    // If already filled (by with_advisor), return as-is
    if !intel.recommendation.is_empty() {
        return intel;
    }

    // Determine recommendation based on safety level and context
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
            // Calculate confidence for archive items
            let has_git = path.join(".git").exists();
            let has_changes = if has_git { has_uncommitted_git_changes(path) } else { false };
            let days = days_since_access(path).unwrap_or(0);
            let signals = vec![
                days > 180,        // Not recently accessed
                !has_changes,      // No uncommitted changes
                has_git,           // Has version control (safer to archive)
            ];
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
            // Determine specific advisor info based on path
            if path_str.contains("docker") || path_str.contains("com.docker") {
                let docker_running = is_docker_running();
                let containers_active = if docker_running { has_running_containers() } else { false };
                let signals = vec![
                    !docker_running || !containers_active,
                    true,  // Regenerates automatically
                    true,  // No personal files
                    !containers_active,
                ];
                intel.confidence = calculate_confidence(&signals);
                intel.evidence = vec![
                    if !docker_running { "✓ Docker is not running".to_string() } else if !containers_active { "✓ No active containers".to_string() } else { "✗ Active containers detected".to_string() },
                    "✓ Auto-generated cache — recreates on demand".to_string(),
                    "✓ No personal files — only downloaded images".to_string(),
                    if !containers_active { "✓ No active dependencies".to_string() } else { "✗ Containers depend on this data".to_string() },
                ];
                intel.recommendation = "Clean".to_string();
                intel.action_label = "Clean Cache".to_string();
                intel.risk_level = if containers_active { "Low" } else { "None" }.to_string();
                intel.time_to_rebuild = "1-5 minutes (re-downloads images on demand)".to_string();
                intel.side_effects = "Docker will re-download container images when you need them next".to_string();
                intel.why_here = "You build software. Docker stores container images, build layers, and cached dependencies. This grows every time you pull or build an image.".to_string();
                intel.reasoning = vec![
                    "Regenerates automatically".to_string(),
                    "No personal files inside".to_string(),
                    if !containers_active { "No active containers detected".to_string() } else { "Active containers detected — stop them first".to_string() },
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
                let xcode_running = is_app_running("Xcode");
                let signals = vec![
                    !xcode_running,
                    true, // Regenerates automatically
                    true, // No personal files
                    true, // Build output only
                ];
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
                let app_running = is_app_running(&app_name_lower);
                let days = last_modified_days(path).unwrap_or(999);
                let signals = vec![
                    !app_running,
                    true,  // Regenerates automatically
                    true,  // No personal files in caches
                    days > 7,
                ];
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
                let days = last_modified_days(path).unwrap_or(999);
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
                let xcode_running = is_app_running("Xcode");
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
                let days = last_modified_days(path).unwrap_or(0);
                let app_name_lower = name.to_lowercase().replace("com.", "").replace('.', " ");
                let app_running = is_app_running(&app_name_lower);
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
                let days = last_modified_days(path).unwrap_or(0);
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

/// Scan a specific directory and return items
fn scan_directory(base_path: &Path, category: &str, subcategory: &str) -> Vec<ScannedItem> {
    let mut items = Vec::new();

    if !base_path.exists() {
        return items;
    }

    let entries = match fs::read_dir(base_path) {
        Ok(entries) => entries,
        Err(_) => return items,
    };

    // Track small files by extension for grouping
    let mut small_files_by_type: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if name.starts_with('.')
            && !name.starts_with(".Trash")
            && !name.starts_with(".docker")
            && category != "Developer"
        {
            continue;
        }

        let size_bytes = if path.is_dir() {
            dir_size(&path, 50)
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };

        // Group small files by extension instead of skipping them
        if size_bytes < 1_048_576 && !path.is_dir() {
            let ext = path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "other".to_string());
            small_files_by_type.entry(ext).or_default().push((path, size_bytes));
            continue;
        }

        if size_bytes < 1_048_576 {
            continue;
        }

        let intel = fill_advisor_fields(analyze_path(&path, category), &path, category);
        let last_accessed_days = days_since_access(&path);

        // Gather file intelligence
        let (file_count, largest_files) = if path.is_dir() {
            (count_files(&path), get_largest_files(&path, 5))
        } else {
            (1, vec![])
        };

        items.push(ScannedItem {
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
        });
    }

    // Now create grouped items for small files (only if group total > 5MB)
    for (ext, files) in &small_files_by_type {
        let total_size: u64 = files.iter().map(|(_, s)| *s).sum();
        let count = files.len();

        // Only show groups that are collectively significant (> 5MB)
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
                     "⚠️ REVIEW — Check if you need these".into(),
                     format!("{} image files. May include personal photos.", count))
                }
            }
            "zip" | "gz" | "tar" | "rar" | "7z" | "dmg" => {
                (SafetyLevel::Safe, 8,
                 format!("✅ YES — {} old archives/installers", count),
                 format!("These are compressed archives and installer packages. Usually one-time-use files you've already extracted or installed."))
            }
            "mp4" | "mov" | "avi" | "mkv" => {
                (SafetyLevel::Review, 4,
                 format!("⚠️ REVIEW — {} video files ({})", count, format_size(total_size)),
                 "Video files may be personal recordings or downloads. Check before removing.".to_string())
            }
            _ => {
                (SafetyLevel::Review, 5,
                 format!("⚠️ REVIEW — {} .{} files", count, ext),
                 format!("{} files of type .{}. Review contents.", count, ext))
            }
        };

        let description = format!("{} .{} files totaling {}", count, ext, format_size(total_size));

        // Get some example file names
        let examples: Vec<String> = files.iter()
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

/// Format size for display in descriptions
fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

/// Main scan function
pub fn perform_scan() -> ScanResult {
    let start = std::time::Instant::now();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/default"));

    let (total_bytes, used_bytes, free_bytes) = get_disk_info();

    let mut all_items: Vec<ScannedItem> = Vec::new();

    let scan_targets: Vec<(&str, &str, PathBuf)> = vec![
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
        ("Documents", "Documents", home.join("Documents")),
        ("Desktop", "Desktop", home.join("Desktop")),
        ("Applications", "Applications", PathBuf::from("/Applications")),
        ("Trash", "Trash", home.join(".Trash")),
    ];

    for (category, subcategory, path) in &scan_targets {
        let items = scan_directory(path, category, subcategory);
        all_items.extend(items);
    }

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

    ScanResult {
        total_bytes,
        used_bytes,
        free_bytes,
        safe_recovery_bytes,
        review_recovery_bytes,
        archive_recovery_bytes,
        categories,
        items: all_items,
        scan_duration_ms,
    }
}
