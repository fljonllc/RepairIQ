use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub cycle_count: u32,
    pub max_capacity_percent: u32,
    pub condition: String,
    pub is_charging: bool,
    pub current_charge_percent: u32,
    pub health_grade: String,
    pub recommendation: String,
}

pub fn get_battery_info() -> BatteryInfo {
    let output = Command::new("system_profiler")
        .args(["SPPowerDataType", "-json"])
        .output();

    let mut info = BatteryInfo {
        cycle_count: 0,
        max_capacity_percent: 100,
        condition: "Unknown".to_string(),
        is_charging: false,
        current_charge_percent: 0,
        health_grade: "Unknown".to_string(),
        recommendation: "Could not read battery info".to_string(),
    };

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse cycle count
        if let Some(pos) = stdout.find("\"sppower_battery_cycle_count\"") {
            let after = &stdout[pos..];
            if let Some(val) = extract_number(after) {
                info.cycle_count = val;
            }
        }

        // Parse max capacity
        if let Some(pos) = stdout.find("\"sppower_battery_max_capacity\"") {
            let after = &stdout[pos..];
            if let Some(val) = extract_number(after) {
                info.max_capacity_percent = val;
            }
        }

        // Parse condition
        if stdout.contains("Normal") {
            info.condition = "Normal".to_string();
        } else if stdout.contains("Service") {
            info.condition = "Service Recommended".to_string();
        }

        // Parse charging
        info.is_charging = stdout.contains("\"Charging\"") || stdout.contains("AC Power");
    }

    // Also try pmset for current charge
    if let Ok(pmset) = Command::new("pmset").args(["-g", "batt"]).output() {
        let stdout = String::from_utf8_lossy(&pmset.stdout);
        for line in stdout.lines() {
            if line.contains('%') {
                if let Some(pct_pos) = line.find('%') {
                    let before = &line[..pct_pos];
                    let digits: String = before
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                    if let Ok(pct) = digits.parse::<u32>() {
                        info.current_charge_percent = pct;
                    }
                }
            }
        }
    }

    // Calculate grade
    info.health_grade = match info.max_capacity_percent {
        90..=100 => "Excellent".to_string(),
        80..=89 => "Good".to_string(),
        70..=79 => "Fair".to_string(),
        _ => "Poor".to_string(),
    };

    info.recommendation = match info.max_capacity_percent {
        90..=100 => format!(
            "Battery is healthy at {}% capacity with {} cycles.",
            info.max_capacity_percent, info.cycle_count
        ),
        80..=89 => format!(
            "Battery has degraded slightly to {}% ({} cycles). Normal wear.",
            info.max_capacity_percent, info.cycle_count
        ),
        70..=79 => format!(
            "Battery at {}% capacity ({} cycles). Consider replacement within a year.",
            info.max_capacity_percent, info.cycle_count
        ),
        _ => format!(
            "Battery significantly degraded at {}% ({} cycles). Replacement recommended.",
            info.max_capacity_percent, info.cycle_count
        ),
    };

    info
}

fn extract_number(text: &str) -> Option<u32> {
    let colon_pos = text.find(':')?;
    let after_colon = &text[colon_pos + 1..];
    let digits: String = after_colon
        .chars()
        .take(20)
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}
