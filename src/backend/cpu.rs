use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Tracks CPU usage via /proc/stat two-sample delta.
/// Instantiate once, call `get_usage()` each tick.
pub struct CpuUsageTracker {
    prev_idle: u64,
    prev_total: u64,
}

impl CpuUsageTracker {
    pub fn new() -> Self {
        let (idle, total) = Self::read_stat();
        Self { prev_idle: idle, prev_total: total }
    }

    /// Returns overall CPU usage % since last call. Call once per update interval.
    pub fn get_usage(&mut self) -> f32 {
        let (idle, total) = Self::read_stat();
        let diff_idle  = idle.saturating_sub(self.prev_idle);
        let diff_total = total.saturating_sub(self.prev_total);
        self.prev_idle  = idle;
        self.prev_total = total;
        if diff_total == 0 {
            return 0.0;
        }
        ((diff_total - diff_idle) as f32 / diff_total as f32 * 100.0).clamp(0.0, 100.0)
    }

    fn read_stat() -> (u64, u64) {
        let stat = fs::read_to_string("/proc/stat").unwrap_or_default();
        for line in stat.lines() {
            if line.starts_with("cpu ") {
                let vals: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if vals.len() >= 4 {
                    // fields: user nice system idle iowait irq softirq steal ...
                    let idle  = vals[3];
                    let total: u64 = vals.iter().sum();
                    return (idle, total);
                }
            }
        }
        (0, 0)
    }
}

/// Tracks per-logical-core CPU usage via /proc/stat two-sample delta.
pub struct PerCoreCpuUsageTracker {
    prev: Vec<(u64, u64)>,
}

impl PerCoreCpuUsageTracker {
    pub fn new(core_count: usize) -> Self {
        let prev = (0..core_count).map(|i| Self::read_core(i)).collect();
        Self { prev }
    }

    pub fn get_usage(&mut self) -> Vec<f32> {
        self.prev.iter_mut().enumerate().map(|(i, prev)| {
            let (idle, total) = Self::read_core(i);
            let di = idle.saturating_sub(prev.0);
            let dt = total.saturating_sub(prev.1);
            *prev = (idle, total);
            if dt == 0 { 0.0 } else { ((dt - di) as f32 / dt as f32 * 100.0).clamp(0.0, 100.0) }
        }).collect()
    }

    fn read_core(core: usize) -> (u64, u64) {
        let stat = fs::read_to_string("/proc/stat").unwrap_or_default();
        let target = format!("cpu{} ", core);
        for line in stat.lines() {
            if line.starts_with(&target) {
                let vals: Vec<u64> = line.split_whitespace().skip(1)
                    .filter_map(|s| s.parse().ok()).collect();
                if vals.len() >= 4 {
                    return (vals[3], vals.iter().sum());
                }
            }
        }
        (0, 0)
    }
}

const CPUFREQ_BASE: &str = "/sys/devices/system/cpu";
const INTEL_PSTATE_PATH: &str = "/sys/devices/system/cpu/intel_pstate";
const AMD_PSTATE_PATH: &str = "/sys/devices/system/cpu/amd_pstate";
// FIX: AMD boost path is cpufreq/boost, not under amd_pstate
const AMD_BOOST_PATH: &str = "/sys/devices/system/cpu/cpufreq/boost";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: String,
    pub vendor: String,
    pub core_count: usize,
    pub driver: CpuDriver,
    pub min_freq: u32,
    pub max_freq: u32,
    pub available_governors: Vec<String>,
    pub scaling_available_frequencies: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuDriver {
    IntelPstate,
    AmdPstate,
    AcpiCpufreq,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreStatus {
    pub core_id: usize,
    pub current_freq: u32,
    pub min_freq: u32,
    pub max_freq: u32,
    pub governor: String,
    pub online: bool,
    pub usage_percent: f32,
}

pub struct CpuManager {
    core_count: usize,
    driver: CpuDriver,
    base_path: PathBuf,
}

impl CpuManager {
    pub fn new() -> Result<Self> {
        let core_count = Self::detect_core_count()?;
        let driver = Self::detect_driver();

        log::info!("Detected {} CPU cores with {:?} driver", core_count, driver);

        Ok(Self {
            core_count,
            driver,
            base_path: PathBuf::from(CPUFREQ_BASE),
        })
    }

    fn detect_core_count() -> Result<usize> {
        // FIX: original used starts_with("cpu") + all_numeric on the remainder,
        // but "cpufreq", "cpuidle" etc. start with "cpu" too. The numeric suffix
        // check was also broken for cpu10+ because `skip(3)` leaves "10" which is
        // all numeric — that part was actually fine. The real issue is that entries
        // like "cpufreq" pass the starts_with check then fail the numeric check
        // silently, so the count ends up correct by accident. Keep the numeric
        // check but add an explicit length guard to be safe.
        let entries =
            fs::read_dir(CPUFREQ_BASE).context("Failed to read CPU directory")?;

        let count = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                // Must be exactly "cpu" + one or more digits, nothing else
                s.starts_with("cpu")
                    && s.len() > 3
                    && s[3..].chars().all(|c| c.is_ascii_digit())
            })
            .count();

        if count == 0 {
            anyhow::bail!("No CPU cores found under {}", CPUFREQ_BASE);
        }
        Ok(count)
    }

    fn detect_driver() -> CpuDriver {
        if Path::new(INTEL_PSTATE_PATH).exists() {
            CpuDriver::IntelPstate
        } else if Path::new(AMD_PSTATE_PATH).exists() {
            CpuDriver::AmdPstate
        } else {
            if let Ok(driver) = fs::read_to_string(format!(
                "{}/cpu0/cpufreq/scaling_driver",
                CPUFREQ_BASE
            )) {
                if driver.trim() == "acpi-cpufreq" {
                    return CpuDriver::AcpiCpufreq;
                }
            }
            CpuDriver::Unknown
        }
    }

    pub fn get_cpu_info(&self) -> Result<CpuInfo> {
        let model = self.read_cpu_model()?;
        let vendor = self.read_cpu_vendor()?;
        let min_freq = self.get_hardware_min_freq(0)?;
        let max_freq = self.get_hardware_max_freq(0)?;
        let available_governors = self.get_available_governors(0)?;
        let scaling_available_frequencies =
            self.get_available_frequencies(0).unwrap_or_default();

        Ok(CpuInfo {
            model,
            vendor,
            core_count: self.core_count,
            driver: self.driver,
            min_freq,
            max_freq,
            available_governors,
            scaling_available_frequencies,
        })
    }

    fn read_cpu_model(&self) -> Result<String> {
        let cpuinfo =
            fs::read_to_string("/proc/cpuinfo").context("Failed to read /proc/cpuinfo")?;
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                if let Some(model) = line.split(':').nth(1) {
                    return Ok(model.trim().to_string());
                }
            }
        }
        Ok("Unknown".to_string())
    }

    fn read_cpu_vendor(&self) -> Result<String> {
        let cpuinfo =
            fs::read_to_string("/proc/cpuinfo").context("Failed to read /proc/cpuinfo")?;
        for line in cpuinfo.lines() {
            if line.starts_with("vendor_id") {
                if let Some(vendor) = line.split(':').nth(1) {
                    return Ok(vendor.trim().to_string());
                }
            }
        }
        Ok("Unknown".to_string())
    }

    pub fn get_core_status(&self, core: usize) -> Result<CoreStatus> {
        if core >= self.core_count {
            anyhow::bail!("Core {} does not exist", core);
        }
        // Use unwrap_or for E-core fields that may not have individual sysfs files.
        let governor = self.get_governor(core)
            .or_else(|_| self.get_governor(0))
            .unwrap_or_else(|_| "unknown".to_string());
        Ok(CoreStatus {
            core_id: core,
            current_freq: self.get_frequency(core).unwrap_or(0),
            min_freq: self.get_scaling_min_freq(core).unwrap_or(0),
            max_freq: self.get_scaling_max_freq(core).unwrap_or(0),
            governor,
            online: self.is_core_online(core).unwrap_or(true),
            usage_percent: 0.0,
        })
    }

    pub fn get_all_core_status(&self) -> Result<Vec<CoreStatus>> {
        let statuses: Vec<CoreStatus> = (0..self.core_count)
            .filter_map(|core| self.get_core_status(core).ok())
            .collect();
        if statuses.is_empty() {
            anyhow::bail!("Could not read status for any CPU core");
        }
        Ok(statuses)
    }

    // ── Frequency reads ───────────────────────────────────────────────────────

    pub fn get_frequency(&self, core: usize) -> Result<u32> {
        let path = format!("{}/cpu{}/cpufreq/scaling_cur_freq", CPUFREQ_BASE, core);
        let freq_khz: u32 = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read frequency for core {}", core))?
            .trim()
            .parse()
            .context("Failed to parse frequency")?;
        Ok(freq_khz / 1000)
    }

    pub fn get_all_frequencies(&self) -> Result<Vec<u32>> {
        // On hybrid CPUs (Intel 12th gen+) some cores may not have individual
        // scaling_cur_freq — skip failures rather than aborting the whole read.
        let freqs: Vec<u32> = (0..self.core_count)
            .filter_map(|c| self.get_frequency(c).ok())
            .collect();
        if freqs.is_empty() {
            anyhow::bail!("Could not read frequency for any CPU core");
        }
        Ok(freqs)
    }

    pub fn set_frequency(&self, core: usize, freq_mhz: u32) -> Result<()> {
        self.check_write_permission()?;
        let freq_khz = freq_mhz * 1000;
        let path = format!("{}/cpu{}/cpufreq/scaling_setspeed", CPUFREQ_BASE, core);
        fs::write(&path, freq_khz.to_string()).with_context(|| {
            format!(
                "Failed to set frequency for core {}. Make sure you have root privileges.",
                core
            )
        })?;
        log::info!("Set core {} frequency to {} MHz", core, freq_mhz);
        Ok(())
    }

    pub fn set_frequency_all(&self, freq_mhz: u32) -> Result<()> {
        for core in 0..self.core_count {
            self.set_frequency(core, freq_mhz)?;
        }
        Ok(())
    }

    // ── Scaling limits ────────────────────────────────────────────────────────

    pub fn get_scaling_min_freq(&self, core: usize) -> Result<u32> {
        let path = format!("{}/cpu{}/cpufreq/scaling_min_freq", CPUFREQ_BASE, core);
        let khz: u32 = fs::read_to_string(&path)
            .context("Failed to read min frequency")?
            .trim()
            .parse()?;
        Ok(khz / 1000)
    }

    pub fn get_scaling_max_freq(&self, core: usize) -> Result<u32> {
        let path = format!("{}/cpu{}/cpufreq/scaling_max_freq", CPUFREQ_BASE, core);
        let khz: u32 = fs::read_to_string(&path)
            .context("Failed to read max frequency")?
            .trim()
            .parse()?;
        Ok(khz / 1000)
    }

    pub fn set_scaling_min_freq(&self, core: usize, freq_mhz: u32) -> Result<()> {
        self.check_write_permission()?;
        let path = format!("{}/cpu{}/cpufreq/scaling_min_freq", CPUFREQ_BASE, core);
        fs::write(&path, (freq_mhz * 1000).to_string()).with_context(|| {
            format!(
                "Failed to set min frequency for core {}. Run with sudo or enable PolicyKit.",
                core
            )
        })?;
        log::info!("Set core {} min frequency to {} MHz", core, freq_mhz);
        Ok(())
    }

    pub fn set_scaling_max_freq(&self, core: usize, freq_mhz: u32) -> Result<()> {
        self.check_write_permission()?;
        let path = format!("{}/cpu{}/cpufreq/scaling_max_freq", CPUFREQ_BASE, core);
        fs::write(&path, (freq_mhz * 1000).to_string()).with_context(|| {
            format!(
                "Failed to set max frequency for core {}. Run with sudo or enable PolicyKit.",
                core
            )
        })?;
        log::info!("Set core {} max frequency to {} MHz", core, freq_mhz);
        Ok(())
    }

    pub fn set_scaling_limits_all(&self, min_mhz: u32, max_mhz: u32) -> Result<()> {
        for core in 0..self.core_count {
            self.set_scaling_min_freq(core, min_mhz)?;
            self.set_scaling_max_freq(core, max_mhz)?;
        }
        Ok(())
    }

    // ── Hardware limits ───────────────────────────────────────────────────────

    pub fn get_hardware_min_freq(&self, core: usize) -> Result<u32> {
        let path = format!("{}/cpu{}/cpufreq/cpuinfo_min_freq", CPUFREQ_BASE, core);
        let khz: u32 = fs::read_to_string(&path)
            .context("Failed to read hardware min frequency")?
            .trim()
            .parse()?;
        Ok(khz / 1000)
    }

    pub fn get_hardware_max_freq(&self, core: usize) -> Result<u32> {
        let path = format!("{}/cpu{}/cpufreq/cpuinfo_max_freq", CPUFREQ_BASE, core);
        let khz: u32 = fs::read_to_string(&path)
            .context("Failed to read hardware max frequency")?
            .trim()
            .parse()?;
        Ok(khz / 1000)
    }

    // ── Governor ──────────────────────────────────────────────────────────────

    pub fn get_governor(&self, core: usize) -> Result<String> {
        let path = format!("{}/cpu{}/cpufreq/scaling_governor", CPUFREQ_BASE, core);
        Ok(fs::read_to_string(&path)
            .context("Failed to read governor")?
            .trim()
            .to_string())
    }

    pub fn get_all_governors(&self) -> Result<Vec<String>> {
        (0..self.core_count).map(|c| self.get_governor(c)).collect()
    }

    pub fn set_governor(&self, core: usize, governor: &str) -> Result<()> {
        self.check_write_permission()?;
        let available = self.get_available_governors(core)?;
        if !available.contains(&governor.to_string()) {
            anyhow::bail!(
                "Governor '{}' is not available. Available: {:?}",
                governor,
                available
            );
        }
        let path = format!("{}/cpu{}/cpufreq/scaling_governor", CPUFREQ_BASE, core);
        fs::write(&path, governor).with_context(|| {
            format!(
                "Failed to set governor for core {}. Run with sudo or enable PolicyKit.",
                core
            )
        })?;
        log::info!("Set core {} governor to {}", core, governor);
        Ok(())
    }

    pub fn set_governor_all(&self, governor: &str) -> Result<()> {
        let mut success = 0;
        for core in 0..self.core_count {
            match self.set_governor(core, governor) {
                Ok(_) => success += 1,
                Err(e) => log::debug!("Core {} governor not individually settable: {}", core, e),
            }
        }
        if success == 0 {
            anyhow::bail!("Could not set governor on any core — root/PolicyKit required?");
        }
        Ok(())
    }

    pub fn get_available_governors(&self, core: usize) -> Result<Vec<String>> {
        // Try the core's own file (use if-let so a read failure falls through to fallbacks)
        let path = format!("{}/cpu{}/cpufreq/scaling_available_governors", CPUFREQ_BASE, core);
        if let Ok(s) = fs::read_to_string(&path) {
            let govs: Vec<String> = s.split_whitespace().map(|s| s.to_string()).collect();
            if !govs.is_empty() {
                return Ok(govs);
            }
        }
        // Hybrid CPUs: E-cores share a policy — fall back to cpu0's file.
        if core != 0 {
            let fallback = format!("{}/cpu0/cpufreq/scaling_available_governors", CPUFREQ_BASE);
            if let Ok(s) = fs::read_to_string(&fallback) {
                let govs: Vec<String> = s.split_whitespace().map(|s| s.to_string()).collect();
                if !govs.is_empty() {
                    return Ok(govs);
                }
            }
        }
        // Intel pstate always supports exactly these two governors.
        if self.driver == CpuDriver::IntelPstate {
            return Ok(vec!["performance".to_string(), "powersave".to_string()]);
        }
        anyhow::bail!("Could not read available governors for core {}", core)
    }

    // ── Available frequencies ─────────────────────────────────────────────────

    pub fn get_available_frequencies(&self, core: usize) -> Result<Vec<u32>> {
        let path = format!(
            "{}/cpu{}/cpufreq/scaling_available_frequencies",
            CPUFREQ_BASE, core
        );
        if !Path::new(&path).exists() {
            return Ok(vec![]);
        }
        let s = fs::read_to_string(&path).context("Failed to read available frequencies")?;
        Ok(s.split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .map(|f| f / 1000)
            .collect())
    }

    // ── Turbo boost ───────────────────────────────────────────────────────────

    pub fn is_turbo_enabled(&self) -> Result<bool> {
        match self.driver {
            CpuDriver::IntelPstate => {
                let path = format!("{}/no_turbo", INTEL_PSTATE_PATH);
                let no_turbo: u8 = fs::read_to_string(&path)
                    .context("Failed to read turbo state")?
                    .trim()
                    .parse()?;
                Ok(no_turbo == 0)
            }
            // FIX: AmdPstate also uses cpufreq/boost (same as AcpiCpufreq on AMD)
            CpuDriver::AmdPstate | CpuDriver::AcpiCpufreq => {
                if Path::new(AMD_BOOST_PATH).exists() {
                    let boost: u8 = fs::read_to_string(AMD_BOOST_PATH)
                        .context("Failed to read boost state")?
                        .trim()
                        .parse()?;
                    Ok(boost == 1)
                } else {
                    Ok(false)
                }
            }
            CpuDriver::Unknown => Ok(false),
        }
    }

    pub fn set_turbo(&self, enable: bool) -> Result<()> {
        self.check_write_permission()?;

        match self.driver {
            CpuDriver::IntelPstate => {
                let path = format!("{}/no_turbo", INTEL_PSTATE_PATH);
                // Intel: no_turbo=0 means turbo ON, no_turbo=1 means turbo OFF
                fs::write(&path, if enable { "0" } else { "1" })
                    .context("Failed to set turbo state. Run with sudo or enable PolicyKit.")?;
            }
            // FIX: AmdPstate turbo is also controlled via cpufreq/boost
            CpuDriver::AmdPstate | CpuDriver::AcpiCpufreq => {
                if Path::new(AMD_BOOST_PATH).exists() {
                    fs::write(AMD_BOOST_PATH, if enable { "1" } else { "0" })
                        .context("Failed to set boost state")?;
                } else {
                    anyhow::bail!("Turbo boost control not available");
                }
            }
            CpuDriver::Unknown => {
                anyhow::bail!("Turbo control not supported for this driver")
            }
        }

        log::info!("Turbo boost {}", if enable { "enabled" } else { "disabled" });
        Ok(())
    }

    // ── EPP ───────────────────────────────────────────────────────────────────

    pub fn set_epp(&self, epp: &str) -> Result<()> {
        if self.driver != CpuDriver::IntelPstate {
            anyhow::bail!("EPP only supported on Intel Pstate driver");
        }
        self.check_write_permission()?;
        for core in 0..self.core_count {
            let path = format!(
                "{}/cpu{}/cpufreq/energy_performance_preference",
                CPUFREQ_BASE, core
            );
            if Path::new(&path).exists() {
                fs::write(&path, epp)
                    .with_context(|| format!("Failed to set EPP for core {}", core))?;
            }
        }
        log::info!("Set EPP to {}", epp);
        Ok(())
    }

    pub fn get_epp(&self, core: usize) -> Result<String> {
        let path = format!(
            "{}/cpu{}/cpufreq/energy_performance_preference",
            CPUFREQ_BASE, core
        );
        if Path::new(&path).exists() {
            Ok(fs::read_to_string(&path)?.trim().to_string())
        } else {
            anyhow::bail!("EPP not supported")
        }
    }

    // ── Core online / offline ─────────────────────────────────────────────────

    pub fn is_core_online(&self, core: usize) -> Result<bool> {
        if core == 0 {
            return Ok(true);
        }
        let path = format!("{}/cpu{}/online", CPUFREQ_BASE, core);
        if !Path::new(&path).exists() {
            return Ok(true);
        }
        let online: u8 = fs::read_to_string(&path)
            .context("Failed to read core online state")?
            .trim()
            .parse()?;
        Ok(online == 1)
    }

    pub fn set_core_online(&self, core: usize, online: bool) -> Result<()> {
        if core == 0 {
            anyhow::bail!("Cannot offline core 0");
        }
        self.check_write_permission()?;
        let path = format!("{}/cpu{}/online", CPUFREQ_BASE, core);
        fs::write(&path, if online { "1" } else { "0" })
            .with_context(|| format!("Failed to set core {} online state", core))?;
        log::info!(
            "Core {} set to {}",
            core,
            if online { "online" } else { "offline" }
        );
        Ok(())
    }

    // ── Core usage ────────────────────────────────────────────────────────────

    fn get_core_usage(&self, _core: usize) -> Result<f32> {
        // TODO: implement a proper two-sample /proc/stat delta
        // A correct implementation would read /proc/stat twice with a short
        // sleep and compute (delta_active / delta_total * 100). Returning 0.0
        // is safe for now and avoids a blocking sleep on the GTK main thread.
        Ok(0.0)
    }

    // ── Permission check ──────────────────────────────────────────────────────

    fn check_write_permission(&self) -> Result<()> {
        if !nix::unistd::Uid::effective().is_root() {
            anyhow::bail!(
                "Root privileges required. Please run with 'sudo' or configure PolicyKit:\n\
                 sudo cpu-power-manager\n\
                 \n\
                 Or install PolicyKit policy:\n\
                 sudo cp assets/com.cpupowermanager.policy /usr/share/polkit-1/actions/"
            );
        }
        Ok(())
    }

    pub fn core_count(&self) -> usize { self.core_count }
    pub fn driver(&self) -> CpuDriver { self.driver }

    /// True if this core has its own individual cpufreq files (P-core on Intel hybrid).
    pub fn is_p_core(&self, core: usize) -> bool {
        Path::new(&format!("{}/cpu{}/cpufreq/scaling_cur_freq", CPUFREQ_BASE, core)).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_manager_creation() {
        let manager = CpuManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_core_count() {
        let manager = CpuManager::new().unwrap();
        assert!(manager.core_count() > 0);
    }
}
