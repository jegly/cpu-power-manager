use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::backend::profile::Profile;

pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into())).join(".config")
        });
    base.join("cpu-power-manager")
}

pub fn autostart_desktop_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/autostart/cpu-power-manager.desktop")
}

pub fn set_autostart(enable: bool) {
    let path = autostart_desktop_path();
    if enable {
        let _ = fs::create_dir_all(path.parent().unwrap());
        let content = "[Desktop Entry]\nType=Application\nName=CPU Power Manager\n\
            Exec=/usr/bin/cpu-power-manager --minimized\nIcon=cpu-power-manager\n\
            X-GNOME-Autostart-enabled=true\n";
        let _ = fs::write(&path, content);
    } else {
        let _ = fs::remove_file(&path);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub auto_tune: AutoTuneConfig,
    #[serde(default)]
    pub thermal: ThermalConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub custom_profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub auto_apply_on_startup: bool,
    #[serde(default)]
    pub last_profile: String,
    #[serde(default = "default_true")]
    pub critical_temp_notify: bool,
    #[serde(default = "default_polling_interval")]
    pub polling_interval_ms: u64,
    #[serde(default = "default_temp_unit")]
    pub temperature_unit: String,
    #[serde(default = "default_notification_level")]
    pub notification_level: String,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTuneConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ac_profile")]
    pub ac_profile: String,
    #[serde(default = "default_battery_profile")]
    pub battery_profile: String,
    #[serde(default = "default_temp_high")]
    pub temp_threshold_high: f32,
    #[serde(default = "default_temp_low")]
    pub temp_threshold_low: f32,
    #[serde(default = "default_load_high")]
    pub load_threshold_high: f32,
    #[serde(default = "default_load_low")]
    pub load_threshold_low: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalConfig {
    #[serde(default = "default_max_temp")]
    pub max_temp_celsius: f32,
    #[serde(default = "default_emergency_temp")]
    pub emergency_temp_celsius: f32,
    #[serde(default)]
    pub fan_control_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default = "default_true")]
    pub enable_graphs: bool,
    #[serde(default = "default_graph_history")]
    pub graph_history_seconds: u64,
    #[serde(default = "default_true")]
    pub show_per_core_stats: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_true")]
    pub log_to_file: bool,
    #[serde(default = "default_log_path")]
    pub log_path: String,
    #[serde(default = "default_max_log_size")]
    pub max_log_size_mb: u64,
}

fn default_true() -> bool { true }
fn default_polling_interval() -> u64 { 1000 }
fn default_temp_unit() -> String { "celsius".to_string() }
fn default_notification_level() -> String { "important".to_string() }
fn default_ac_profile() -> String { "performance".to_string() }
fn default_battery_profile() -> String { "balanced".to_string() }
fn default_temp_high() -> f32 { 80.0 }
fn default_temp_low() -> f32 { 60.0 }
fn default_load_high() -> f32 { 70.0 }
fn default_load_low() -> f32 { 30.0 }
fn default_max_temp() -> f32 { 90.0 }
fn default_emergency_temp() -> f32 { 95.0 }
fn default_graph_history() -> u64 { 300 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_path() -> String {
    format!(
        "{}/.local/share/cpu-power-manager/app.log",
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
    )
}
fn default_max_log_size() -> u64 { 10 }
fn default_ui_scale() -> String { "normal".to_string() }

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_start: true,
            start_minimized: false,
            minimize_to_tray: false,
            auto_apply_on_startup: false,
            last_profile: String::new(),
            critical_temp_notify: true,
            polling_interval_ms: 1000,
            temperature_unit: "celsius".to_string(),
            notification_level: "important".to_string(),
            ui_scale: "normal".to_string(),
        }
    }
}

impl Default for AutoTuneConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ac_profile: "performance".to_string(),
            battery_profile: "balanced".to_string(),
            temp_threshold_high: 80.0,
            temp_threshold_low: 60.0,
            load_threshold_high: 70.0,
            load_threshold_low: 30.0,
        }
    }
}

impl Default for ThermalConfig {
    fn default() -> Self {
        Self {
            max_temp_celsius: 90.0,
            emergency_temp_celsius: 95.0,
            fan_control_enabled: false,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_graphs: true,
            graph_history_seconds: 300,
            show_per_core_stats: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_to_file: true,
            log_path: default_log_path(),
            max_log_size_mb: 10,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            auto_tune: AutoTuneConfig::default(),
            thermal: ThermalConfig::default(),
            monitoring: MonitoringConfig::default(),
            logging: LoggingConfig::default(),
            custom_profiles: Vec::new(),
        }
    }
}

pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        let config = Self::load_config(&config_path)?;
        Ok(Self { config, config_path })
    }

    fn get_config_path() -> Result<PathBuf> {
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config)
        } else {
            let home =
                std::env::var("HOME").context("HOME environment variable not set")?;
            PathBuf::from(home).join(".config")
        };

        let app_config_dir = config_dir.join("cpu-power-manager");
        fs::create_dir_all(&app_config_dir).context("Failed to create config directory")?;
        Ok(app_config_dir.join("config.toml"))
    }

    fn load_config(path: &PathBuf) -> Result<Config> {
        if path.exists() {
            let config_str =
                fs::read_to_string(path).context("Failed to read config file")?;
            toml::from_str(&config_str).context("Failed to parse config file")
        } else {
            let config = Config::default();
            let config_str = toml::to_string_pretty(&config)?;
            fs::write(path, config_str)?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_str = toml::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, config_str).context("Failed to save config file")
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn get_config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    // FIX: original returned Option<&Profile> which forced callers to unwrap and
    // made the CLI `ApplyProfile` command fail to compile (expected Result).
    // Now returns Result<Profile> with a descriptive error on unknown names.
    pub fn get_profile(&self, name: &str) -> Result<Profile> {
        match name.to_lowercase().as_str() {
            "performance" => Ok(Profile::performance()),
            "balanced"    => Ok(Profile::balanced()),
            "powersave"   => Ok(Profile::powersave()),
            "silent"      => Ok(Profile::silent()),
            _ => anyhow::bail!(
                "Profile '{}' not found. Available: performance, balanced, powersave, silent",
                name
            ),
        }
    }
}
