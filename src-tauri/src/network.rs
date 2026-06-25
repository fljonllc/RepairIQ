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
    connections.dedup_by(|a, b| a.process_name == b.process_name && a.remote_address == b.remote_address);
    connections
}
