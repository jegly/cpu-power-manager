use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct PowerManager;

impl PowerManager {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Returns true if the system is connected to AC power.
    /// Falls back to checking multiple common AC supply paths.
    pub fn is_on_ac_power(&self) -> Result<bool> {
        // FIX: original used bool::then() incorrectly — the outer .exists() check
        // returned a bool, and .then(|| ...) on that bool only runs the closure when
        // the bool is true, but the inner closure could still return None if the file
        // read or parse failed, leaving Option<Option<bool>> which .flatten() collapsed
        // to None, causing an Err instead of a meaningful fallback. Rewritten clearly.

        let ac_paths = [
            "/sys/class/power_supply/AC/online",
            "/sys/class/power_supply/AC0/online",
            "/sys/class/power_supply/ACAD/online",
        ];

        for path in &ac_paths {
            if Path::new(path).exists() {
                let value = fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path, e))?;
                let online: u8 = value
                    .trim()
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Failed to parse AC state from {}: {}", path, e))?;
                return Ok(online == 1);
            }
        }

        // No AC supply node found — assume AC (desktop / no battery info)
        log::warn!("No AC power supply node found; assuming AC power");
        Ok(true)
    }
}
