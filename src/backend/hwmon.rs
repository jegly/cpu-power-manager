use std::fs;
use std::path::Path;

pub struct HwmonReader;

impl HwmonReader {
    /// First non-zero fan RPM found across all hwmon devices.
    pub fn get_fan_rpm() -> Option<u32> {
        for i in 0..16 {
            let base = format!("/sys/class/hwmon/hwmon{}", i);
            if !Path::new(&base).exists() { break; }
            for fan in 1..=8 {
                let path = format!("{}/fan{}_input", base, fan);
                if let Ok(s) = fs::read_to_string(&path) {
                    if let Ok(rpm) = s.trim().parse::<u32>() {
                        if rpm > 0 { return Some(rpm); }
                    }
                }
            }
        }
        None
    }

    /// Per-core temps from the `coretemp` kernel driver.
    /// Returns vec of (physical_core_id, temp_celsius).
    pub fn get_per_core_temps() -> Vec<(usize, f32)> {
        for i in 0..16 {
            let base = format!("/sys/class/hwmon/hwmon{}", i);
            let name_path = format!("{}/name", base);
            if fs::read_to_string(&name_path).map(|n| n.trim().to_string())
                .unwrap_or_default() != "coretemp" { continue; }

            let mut result = Vec::new();
            for j in 1..=32 {
                let label = match fs::read_to_string(format!("{}/temp{}_label", base, j)) {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let val = match fs::read_to_string(format!("{}/temp{}_input", base, j)) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if label.trim().starts_with("Core") {
                    if let (Some(id), Ok(mdeg)) = (
                        label.trim().split_whitespace().nth(1).and_then(|s| s.parse::<usize>().ok()),
                        val.trim().parse::<u32>(),
                    ) {
                        result.push((id, mdeg as f32 / 1000.0));
                    }
                }
            }
            if !result.is_empty() { return result; }
        }
        Vec::new()
    }
}
