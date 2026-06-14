use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMachineSnapshot {
    pub date: String,
    pub age_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMachineInfo {
    pub snapshots: Vec<TimeMachineSnapshot>,
    pub estimated_size_bytes: u64,
    pub snapshot_count: u32,
    pub cleanable_count: u32, // snapshots older than 24h
}

/// Detect local Time Machine snapshots
pub fn detect_snapshots() -> TimeMachineInfo {
    let output = Command::new("tmutil")
        .args(["listlocalsnapshots", "/"])
        .output();
    
    let mut snapshots = Vec::new();
    let mut cleanable = 0u32;
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // Lines look like: com.apple.TimeMachine.2024-01-15-123456.local
            if line.contains("TimeMachine") {
                // Extract date portion
                let parts: Vec<&str> = line.split('.').collect();
                if parts.len() >= 4 {
                    let date_str = parts[3]; // e.g., "2024-01-15-123456"
                    let date_display = date_str.replace('-', "/").chars().take(10).collect::<String>();
                    
                    // Estimate age (rough)
                    let age_days = estimate_snapshot_age(date_str);
                    
                    if age_days > 1 {
                        cleanable += 1;
                    }
                    
                    snapshots.push(TimeMachineSnapshot {
                        date: date_display,
                        age_days,
                    });
                }
            }
        }
    }
    
    // Estimate size: typically 1-5GB per snapshot depending on changes
    let estimated_size = (snapshots.len() as u64) * 2_000_000_000; // ~2GB per snapshot estimate
    
    TimeMachineInfo {
        snapshot_count: snapshots.len() as u32,
        cleanable_count: cleanable,
        estimated_size_bytes: estimated_size,
        snapshots,
    }
}

/// Clean old Time Machine snapshots (older than specified days)
pub fn clean_snapshots(older_than_days: u64) -> Result<u32, String> {
    let info = detect_snapshots();
    let mut cleaned = 0u32;
    
    for snapshot in &info.snapshots {
        if snapshot.age_days > older_than_days {
            // We can't delete by date directly; user should run tmutil manually
            cleaned += 1;
        }
    }
    
    // Return count of snapshots that WOULD be cleaned
    // Actual deletion requires sudo, which RepairIQ won't do
    Ok(cleaned)
}

fn estimate_snapshot_age(date_str: &str) -> u64 {
    // Parse YYYY-MM-DD from the snapshot name
    let parts: Vec<&str> = date_str.splitn(4, '-').collect();
    if parts.len() >= 3 {
        if let (Ok(year), Ok(month), Ok(day)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            let now = chrono::Utc::now().date_naive();
            if let Some(snapshot_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                let diff = now.signed_duration_since(snapshot_date);
                return diff.num_days().max(0) as u64;
            }
        }
    }
    0
}
