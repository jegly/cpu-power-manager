use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub present: bool,
    pub on_ac: bool,
    pub charge_percent: f32,
    pub power_now_w: Option<f32>,
    pub status: String,
}

pub struct PowerSupplyReader;

impl PowerSupplyReader {
    pub fn read() -> BatteryInfo {
        let on_ac = Self::check_ac();
        for prefix in &["BAT", "CMB"] {
            for i in 0..5 {
                let base = format!("/sys/class/power_supply/{}{}", prefix, i);
                if Path::new(&base).exists() {
                    return Self::read_battery(&base, on_ac);
                }
            }
        }
        BatteryInfo {
            present: false,
            on_ac,
            charge_percent: 0.0,
            power_now_w: None,
            status: if on_ac { "AC Power".into() } else { "DC (No Battery)".into() },
        }
    }

    fn check_ac() -> bool {
        for name in &["AC0", "AC", "ACAD", "ADP0", "ADP1"] {
            let path = format!("/sys/class/power_supply/{}/online", name);
            if let Ok(s) = fs::read_to_string(&path) {
                return s.trim() == "1";
            }
        }
        true
    }

    fn read_battery(base: &str, on_ac: bool) -> BatteryInfo {
        let charge = fs::read_to_string(format!("{}/capacity", base))
            .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0.0f32);
        let status = fs::read_to_string(format!("{}/status", base))
            .map(|s| s.trim().to_string()).unwrap_or_else(|_| "Unknown".into());
        let power_now_w = fs::read_to_string(format!("{}/power_now", base))
            .ok().and_then(|s| s.trim().parse::<u64>().ok())
            .map(|uw| uw as f32 / 1_000_000.0)
            .or_else(|| {
                let ua = fs::read_to_string(format!("{}/current_now", base))
                    .ok()?.trim().parse::<f64>().ok()?;
                let uv = fs::read_to_string(format!("{}/voltage_now", base))
                    .ok()?.trim().parse::<f64>().ok()?;
                Some((ua * uv / 1e12) as f32)
            });
        BatteryInfo { present: true, on_ac, charge_percent: charge, power_now_w, status }
    }
}

/// Tracks CPU package power draw via Intel RAPL energy counter (two-sample delta).
pub struct RaplTracker {
    prev_energy_uj: u64,
    prev_time: Instant,
}

impl RaplTracker {
    const PATH: &'static str = "/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj";

    pub fn new() -> Self {
        Self {
            prev_energy_uj: Self::read_energy().unwrap_or(0),
            prev_time: Instant::now(),
        }
    }

    pub fn is_available() -> bool { Path::new(Self::PATH).exists() }

    /// Watts since last call. Returns None until at least one full interval has elapsed.
    pub fn get_power_w(&mut self) -> Option<f32> {
        let energy = Self::read_energy()?;
        let elapsed = self.prev_time.elapsed().as_secs_f64();
        if elapsed < 0.1 { return None; }
        let delta_uj = energy.wrapping_sub(self.prev_energy_uj);
        self.prev_energy_uj = energy;
        self.prev_time = Instant::now();
        Some((delta_uj as f64 / elapsed / 1_000_000.0) as f32)
    }

    fn read_energy() -> Option<u64> {
        fs::read_to_string(Self::PATH).ok()?.trim().parse().ok()
    }
}
