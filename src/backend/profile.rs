use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::backend::cpu::CpuManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub governor: String,
    pub turbo: TurboMode,
    pub min_freq_mhz: Option<u32>,
    pub max_freq_mhz: Option<u32>,
    #[serde(default)]
    pub epp: Option<String>,
    #[serde(default)]
    pub epb: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurboMode {
    Always,
    Auto,
    Never,
}

impl Profile {
    pub fn performance() -> Self {
        Self {
            name: "Performance".to_string(),
            description: "Maximum performance, highest power consumption".to_string(),
            governor: "performance".to_string(),
            turbo: TurboMode::Always,
            min_freq_mhz: None,
            max_freq_mhz: None,
            epp: Some("performance".to_string()),
            epb: Some(0),
        }
    }

    pub fn balanced() -> Self {
        Self {
            name: "Balanced".to_string(),
            description: "Balance between performance and power efficiency".to_string(),
            governor: "powersave".to_string(), // Changed from schedutil for Intel Pstate
            turbo: TurboMode::Auto,
            min_freq_mhz: None,
            max_freq_mhz: None,
            epp: Some("balance_performance".to_string()),
            epb: Some(6),
        }
    }

    pub fn powersave() -> Self {
        Self {
            name: "Power Saver".to_string(),
            description: "Maximum battery life, reduced performance".to_string(),
            governor: "powersave".to_string(),
            turbo: TurboMode::Never,
            min_freq_mhz: None,
            max_freq_mhz: Some(2400),
            epp: Some("power".to_string()),
            epb: Some(15),
        }
    }

    pub fn silent() -> Self {
        Self {
            name: "Silent".to_string(),
            description: "Quiet operation, temperature priority".to_string(),
            governor: "powersave".to_string(),
            turbo: TurboMode::Never,
            min_freq_mhz: Some(800),
            max_freq_mhz: Some(2000),
            epp: Some("power".to_string()),
            epb: Some(15),
        }
    }

    pub fn apply(&self, cpu_manager: &CpuManager) -> Result<()> {
        log::info!("Applying profile: {}", self.name);

        // Get available governors to ensure compatibility
        let available_governors = cpu_manager.get_available_governors(0)
            .context("Failed to get available governors")?;
        
        // Determine the best governor to use
        let governor_to_use = self.select_best_governor(&available_governors)?;
        
        log::debug!("Using governor: {} (requested: {}, available: {:?})", 
                   governor_to_use, self.governor, available_governors);

        // Set governor with fallback
        cpu_manager.set_governor_all(governor_to_use)
            .context("Failed to set governor")?;

        // Set turbo mode
        match self.turbo {
            TurboMode::Always => {
                cpu_manager.set_turbo(true)
                    .context("Failed to enable turbo")?;
            },
            TurboMode::Never => {
                cpu_manager.set_turbo(false)
                    .context("Failed to disable turbo")?;
            },
            TurboMode::Auto => {
                // For auto mode, enable it by default
                // Auto-tuning logic would manage it dynamically
                cpu_manager.set_turbo(true)
                    .context("Failed to set turbo to auto mode")?;
            }
        }

        // CRITICAL FIX: Always reset frequency limits to hardware defaults first!
        // This prevents "sticky" limits from previous profiles
        let hw_min = cpu_manager.get_hardware_min_freq(0)?;
        let hw_max = cpu_manager.get_hardware_max_freq(0)?;
        
        log::debug!("Resetting frequency limits to hardware defaults: {} - {} MHz", hw_min, hw_max);
        
        for core in 0..cpu_manager.core_count() {
            // E-cores on Intel hybrid share a policy and may not accept individual writes — skip silently.
            if let Err(e) = cpu_manager.set_scaling_min_freq(core, hw_min) {
                log::debug!("Core {} min freq reset skipped: {}", core, e);
            }
            if let Err(e) = cpu_manager.set_scaling_max_freq(core, hw_max) {
                log::debug!("Core {} max freq reset skipped: {}", core, e);
            }
        }

        // Apply profile-specific limits if specified
        if let Some(min_freq) = self.min_freq_mhz {
            log::debug!("Applying profile min frequency: {} MHz", min_freq);
            for core in 0..cpu_manager.core_count() {
                if let Err(e) = cpu_manager.set_scaling_min_freq(core, min_freq) {
                    log::debug!("Core {} min freq set skipped: {}", core, e);
                }
            }
        }

        if let Some(max_freq) = self.max_freq_mhz {
            log::debug!("Applying profile max frequency: {} MHz", max_freq);
            for core in 0..cpu_manager.core_count() {
                if let Err(e) = cpu_manager.set_scaling_max_freq(core, max_freq) {
                    log::debug!("Core {} max freq set skipped: {}", core, e);
                }
            }
        }

        // Set EPP (Energy Performance Preference) if supported and specified
        if let Some(ref epp) = self.epp {
            if let Err(e) = cpu_manager.set_epp(epp) {
                log::warn!("Failed to set EPP to {}: {} (may not be supported)", epp, e);
            }
        }

        log::info!("Profile '{}' applied successfully", self.name);
        Ok(())
    }

    /// Select the best available governor based on what's requested and what's available
    fn select_best_governor(&self, available: &[String]) -> Result<&str> {
        // If requested governor is available, use it
        if available.iter().any(|g| g == &self.governor) {
            return Ok(&self.governor);
        }

        // Fallback logic based on profile intent
        match self.name.as_str() {
            "Performance" => {
                // For performance profile, prefer: performance > powersave
                if available.contains(&"performance".to_string()) {
                    log::warn!("Governor '{}' not available, using 'performance'", self.governor);
                    Ok("performance")
                } else if available.contains(&"powersave".to_string()) {
                    log::warn!("Governor '{}' not available, using 'powersave' (will set EPP to performance)", self.governor);
                    Ok("powersave")
                } else {
                    anyhow::bail!("No suitable governor available for Performance profile");
                }
            },
            "Balanced" => {
                // For balanced, prefer: schedutil > ondemand > powersave > performance
                if available.contains(&"schedutil".to_string()) {
                    log::warn!("Using 'schedutil' instead of '{}'", self.governor);
                    Ok("schedutil")
                } else if available.contains(&"ondemand".to_string()) {
                    log::warn!("Using 'ondemand' instead of '{}'", self.governor);
                    Ok("ondemand")
                } else if available.contains(&"powersave".to_string()) {
                    log::info!("Using 'powersave' with balanced EPP for Balanced profile");
                    Ok("powersave")
                } else if available.contains(&"performance".to_string()) {
                    log::warn!("Using 'performance' instead of '{}'", self.governor);
                    Ok("performance")
                } else {
                    anyhow::bail!("No suitable governor available for Balanced profile");
                }
            },
            _ => {
                // For power saver and silent, prefer: powersave > conservative > ondemand
                if available.contains(&"powersave".to_string()) {
                    if &self.governor != "powersave" {
                        log::warn!("Governor '{}' not available, using 'powersave'", self.governor);
                    }
                    Ok("powersave")
                } else if available.contains(&"conservative".to_string()) {
                    log::warn!("Governor '{}' not available, using 'conservative'", self.governor);
                    Ok("conservative")
                } else if available.contains(&"ondemand".to_string()) {
                    log::warn!("Governor '{}' not available, using 'ondemand'", self.governor);
                    Ok("ondemand")
                } else {
                    anyhow::bail!("No suitable governor available for {} profile", self.name);
                }
            }
        }
    }
}

pub struct ProfileManager {
    profiles: Vec<Profile>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: vec![
                Profile::performance(),
                Profile::balanced(),
                Profile::powersave(),
                Profile::silent(),
            ],
        }
    }

    pub fn get_profiles(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn add_profile(&mut self, profile: Profile) {
        self.profiles.push(profile);
    }

    pub fn remove_profile(&mut self, name: &str) {
        self.profiles.retain(|p| p.name != name);
    }
}
