use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub pressure: String, // "Normal" | "Warning" | "Critical"
    pub top_consumers: Vec<ProcessMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMemory {
    pub name: String,
    pub memory_bytes: u64,
    pub pid: u32,
}

pub fn get_memory_info() -> MemoryInfo {
    let mut info = MemoryInfo {
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
        pressure: "Unknown".to_string(),
        top_consumers: vec![],
    };

    // Get total RAM
    if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        let s = String::from_utf8_lossy(&output.stdout);
        info.total_bytes = s.trim().parse().unwrap_or(0);
    }

    // Get memory pressure
    if let Ok(output) = Command::new("memory_pressure").output() {
        let s = String::from_utf8_lossy(&output.stdout);
        if s.contains("normal") {
            info.pressure = "Normal".to_string();
        } else if s.contains("warn") {
            info.pressure = "Warning".to_string();
        } else if s.contains("critical") {
            info.pressure = "Critical".to_string();
        }
    }

    // Get top memory consumers via ps
    if let Ok(output) = Command::new("ps")
        .args(["-axo", "pid,rss,comm"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes: Vec<ProcessMemory> = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();
            if parts.len() >= 3 {
                let pid = parts[0].trim().parse::<u32>().unwrap_or(0);
                let rss_kb = parts[1].trim().parse::<u64>().unwrap_or(0);
                let name = parts[2]
                    .trim()
                    .split('/')
                    .last()
                    .unwrap_or(parts[2])
                    .to_string();

                if rss_kb > 50_000 {
                    // Only show processes using > 50MB
                    processes.push(ProcessMemory {
                        name,
                        memory_bytes: rss_kb * 1024,
                        pid,
                    });
                }
            }
        }

        processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        processes.truncate(10);
        info.top_consumers = processes;
        info.used_bytes = info.top_consumers.iter().map(|p| p.memory_bytes).sum();
    }

    info.free_bytes = info.total_bytes.saturating_sub(info.used_bytes);
    info
}
