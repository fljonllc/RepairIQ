use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub process_name: String,
    pub pid: String,
    pub remote_address: String,
    pub status: String,
}

pub fn get_active_connections() -> Vec<NetworkConnection> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("lsof").args(["-i", "-n", "-P"]).output();

        let mut connections = Vec::new();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    let name = parts[0].to_string();
                    let pid = parts[1].to_string();
                    let remote = parts.last().unwrap_or(&"").to_string();
                    let status = if parts.len() > 9 {
                        parts[9].to_string()
                    } else {
                        "ESTABLISHED".to_string()
                    };

                    if status.contains("ESTABLISHED") || status.contains("LISTEN") {
                        connections.push(NetworkConnection {
                            process_name: name,
                            pid,
                            remote_address: remote,
                            status,
                        });
                    }
                }
            }
        }

        connections.sort_by(|a, b| a.process_name.cmp(&b.process_name));
        connections.dedup_by(|a, b| {
            a.process_name == b.process_name && a.remote_address == b.remote_address
        });
        connections
    }

    #[cfg(target_os = "windows")]
    {
        let mut connections = Vec::new();

        if let Ok(output) = Command::new("netstat").args(["-b", "-n"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();
            let mut i = 0;

            while i < lines.len() {
                let line = lines[i].trim();

                // netstat -b output format:
                //   TCP    192.168.1.5:54321    93.184.216.34:443    ESTABLISHED
                //  [chrome.exe]
                if line.starts_with("TCP") || line.starts_with("UDP") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let remote_address = parts[2].to_string();
                        let status = parts[3].to_string();

                        // Next line should be the process name in brackets
                        let process_name = if i + 1 < lines.len() {
                            let next_line = lines[i + 1].trim();
                            if next_line.starts_with('[') && next_line.ends_with(']') {
                                i += 1;
                                next_line[1..next_line.len() - 1].to_string()
                            } else {
                                "Unknown".to_string()
                            }
                        } else {
                            "Unknown".to_string()
                        };

                        if status == "ESTABLISHED" || status == "LISTENING" {
                            connections.push(NetworkConnection {
                                process_name,
                                pid: String::new(),
                                remote_address,
                                status,
                            });
                        }
                    }
                }

                i += 1;
            }
        }

        connections.sort_by(|a, b| a.process_name.cmp(&b.process_name));
        connections.dedup_by(|a, b| {
            a.process_name == b.process_name && a.remote_address == b.remote_address
        });
        connections
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Vec::new()
    }
}
