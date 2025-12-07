use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

const THERMAL_BASE: &str = "/sys/class/thermal";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZone {
    pub id: usize,
    pub type_name: String,
    pub temp_celsius: f32,
    pub trip_points: Vec<TripPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripPoint {
    pub id: usize,
    pub temp_celsius: f32,
    pub trip_type: String,
}

pub struct ThermalManager {
    zones: Vec<PathBuf>,
}

impl ThermalManager {
    pub fn new() -> Result<Self> {
        let zones = Self::discover_thermal_zones()?;
        log::info!("Discovered {} thermal zones", zones.len());
        Ok(Self { zones })
    }

    fn discover_thermal_zones() -> Result<Vec<PathBuf>> {
        let entries = fs::read_dir(THERMAL_BASE)
            .context("Failed to read thermal directory")?;
        
        let mut zones = vec![];
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("thermal_zone") {
                zones.push(entry.path());
            }
        }
        
        zones.sort();
        Ok(zones)
    }

    pub fn get_zone_count(&self) -> usize {
        self.zones.len()
    }

    pub fn get_temperature(&self, zone: usize) -> Result<f32> {
        if zone >= self.zones.len() {
            anyhow::bail!("Thermal zone {} does not exist", zone);
        }

        let temp_path = self.zones[zone].join("temp");
        let temp_millicelsius: i32 = fs::read_to_string(&temp_path)
            .context("Failed to read temperature")?
            .trim()
            .parse()
            .context("Failed to parse temperature")?;
        
        Ok(temp_millicelsius as f32 / 1000.0)
    }

    pub fn get_all_temperatures(&self) -> Result<Vec<f32>> {
        (0..self.zones.len())
            .map(|zone| self.get_temperature(zone))
            .collect()
    }

    pub fn get_zone_type(&self, zone: usize) -> Result<String> {
        if zone >= self.zones.len() {
            anyhow::bail!("Thermal zone {} does not exist", zone);
        }

        let type_path = self.zones[zone].join("type");
        Ok(fs::read_to_string(&type_path)
            .context("Failed to read zone type")?
            .trim()
            .to_string())
    }

    pub fn get_zone_info(&self, zone: usize) -> Result<ThermalZone> {
        let temp_celsius = self.get_temperature(zone)?;
        let type_name = self.get_zone_type(zone)?;
        let trip_points = self.get_trip_points(zone)?;

        Ok(ThermalZone {
            id: zone,
            type_name,
            temp_celsius,
            trip_points,
        })
    }

    pub fn get_all_zones(&self) -> Result<Vec<ThermalZone>> {
        (0..self.zones.len())
            .map(|zone| self.get_zone_info(zone))
            .collect()
    }

    fn get_trip_points(&self, zone: usize) -> Result<Vec<TripPoint>> {
        let mut trip_points = vec![];
        let mut trip_id = 0;

        loop {
            let temp_path = self.zones[zone].join(format!("trip_point_{}_temp", trip_id));
            let type_path = self.zones[zone].join(format!("trip_point_{}_type", trip_id));

            if !temp_path.exists() {
                break;
            }

            let temp_millicelsius: i32 = fs::read_to_string(&temp_path)
                .context("Failed to read trip point temperature")?
                .trim()
                .parse()
                .unwrap_or(0);

            let trip_type = fs::read_to_string(&type_path)
                .unwrap_or_else(|_| "unknown".to_string())
                .trim()
                .to_string();

            trip_points.push(TripPoint {
                id: trip_id,
                temp_celsius: temp_millicelsius as f32 / 1000.0,
                trip_type,
            });

            trip_id += 1;
        }

        Ok(trip_points)
    }

    pub fn get_max_temperature(&self) -> Result<f32> {
        let temps = self.get_all_temperatures()?;
        temps.into_iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .ok_or_else(|| anyhow::anyhow!("No thermal zones found"))
    }

    pub fn get_cpu_temperature(&self) -> Result<f32> {
        // Try to find CPU package temperature
        for (zone_id, zone_path) in self.zones.iter().enumerate() {
            let type_path = zone_path.join("type");
            if let Ok(zone_type) = fs::read_to_string(&type_path) {
                let zone_type = zone_type.trim().to_lowercase();
                if zone_type.contains("x86_pkg_temp") || 
                   zone_type.contains("cpu") ||
                   zone_type.contains("core") {
                    return self.get_temperature(zone_id);
                }
            }
        }

        // Fallback to max temperature
        self.get_max_temperature()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_manager() {
        let manager = ThermalManager::new();
        if manager.is_ok() {
            let manager = manager.unwrap();
            assert!(manager.get_zone_count() > 0);
        }
    }
}
