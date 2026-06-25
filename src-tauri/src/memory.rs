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
    #[cfg(target_os = "macos")]
    {
        get_memory_info_macos()
    }

    #[cfg(target_os = "windows")]
    {
        get_memory_info_windows()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        MemoryInfo {
            total_bytes: 0,
            used_bytes: 0,
            free_bytes: 0,
            pressure: "Unknown".to_string(),
            top_consumers: vec![],
        }
    }
}

#[cfg(target_os = "macos")]
fn get_memory_info_macos() -> MemoryInfo {
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

#[cfg(target_os = "windows")]
fn get_memory_info_windows() -> MemoryInfo {
    let mut info = MemoryInfo {
        total_bytes: 0,
        used_bytes: 0,
        free_bytes: 0,
        pressure: "Unknown".to_string(),
        top_consumers: vec![],
    };

    // Get total RAM via wmic
    if let Ok(output) = Command::new("wmic")
        .args(["ComputerSystem", "get", "TotalPhysicalMemory", "/format:csv"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(total) = parts.last().unwrap_or(&"").trim().parse::<u64>() {
                    if total > 0 {
                        info.total_bytes = total;
                    }
                }
            }
        }
    }

    // Get free memory via wmic
    if let Ok(output) = Command::new("wmic")
        .args(["OS", "get", "FreePhysicalMemory", "/format:csv"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(free_kb) = parts.last().unwrap_or(&"").trim().parse::<u64>() {
                    if free_kb > 0 {
                        info.free_bytes = free_kb * 1024;
                    }
                }
            }
        }
    }

    info.used_bytes = info.total_bytes.saturating_sub(info.free_bytes);

    // Determine memory pressure based on usage percentage
    if info.total_bytes > 0 {
        let usage_pct = (info.used_bytes as f64 / info.total_bytes as f64) * 100.0;
        info.pressure = if usage_pct > 90.0 {
            "Critical".to_string()
        } else if usage_pct > 75.0 {
            "Warning".to_string()
        } else {
            "Normal".to_string()
        };
    }

    // Get top processes via tasklist
    if let Ok(output) = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes: Vec<ProcessMemory> = Vec::new();

        for line in stdout.lines() {
            // CSV format: "Image Name","PID","Session Name","Session#","Mem Usage"
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 5 {
                let name = parts[0].trim().trim_matches('"').to_string();
                let pid = parts[1]
                    .trim()
                    .trim_matches('"')
                    .parse::<u32>()
                    .unwrap_or(0);
                // Memory is like "123,456 K" — but since we split on comma,
                // we need to handle the memory field which may span parts[4..]
                let mem_str: String = parts[4..]
                    .iter()
                    .map(|s| s.trim().trim_matches('"'))
                    .collect::<Vec<&str>>()
                    .join("");
                let mem_kb: u64 = mem_str
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0);

                if mem_kb > 50_000 {
                    // Only show processes using > 50MB
                    processes.push(ProcessMemory {
                        name,
                        memory_bytes: mem_kb * 1024,
                        pid,
                    });
                }
            }
        }

        processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        processes.truncate(10);
        info.top_consumers = processes;
    }

    info
}
