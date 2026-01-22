//! System monitoring module for Armoury Crate Linux
//!
//! Provides real-time system metrics including:
//! - CPU usage, temperature, and frequency
//! - GPU usage and temperature (via hwmon or NVML)
//! - RAM usage
//! - Fan RPM readings
//! - Power consumption

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use log::debug;
use sysinfo::System;
use tokio::sync::RwLock;

/// Monitoring data snapshot
#[derive(Debug, Clone, Default)]
pub struct MonitoringData {
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// CPU temperature in Celsius
    pub cpu_temp: f32,
    /// Average CPU frequency in MHz
    pub cpu_freq_mhz: u64,
    /// GPU usage percentage (0-100)
    pub gpu_usage: f32,
    /// GPU temperature in Celsius
    pub gpu_temp: f32,
    /// GPU power draw in watts
    pub gpu_power_watts: f32,
    /// RAM usage percentage (0-100)
    pub ram_usage: f32,
    /// Total RAM in GB
    pub ram_total_gb: f32,
    /// RAM used in GB
    pub ram_used_gb: f32,
    /// Fan RPM readings (CPU, GPU, etc.)
    pub fan_rpm: Vec<u32>,
    /// Total power draw in watts
    pub power_draw_watts: f32,
    /// Battery percentage (None if no battery)
    pub battery_percent: Option<f32>,
    /// Whether on AC power
    pub on_ac_power: bool,
}

/// History buffer for monitoring data
#[derive(Debug, Clone)]
pub struct MonitoringHistory {
    /// Maximum number of entries to keep
    pub max_entries: usize,
    /// CPU usage history
    pub cpu_usage: Vec<f32>,
    /// CPU temperature history
    pub cpu_temp: Vec<f32>,
    /// GPU usage history
    pub gpu_usage: Vec<f32>,
    /// GPU temperature history
    pub gpu_temp: Vec<f32>,
}

impl MonitoringHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            cpu_usage: Vec::with_capacity(max_entries),
            cpu_temp: Vec::with_capacity(max_entries),
            gpu_usage: Vec::with_capacity(max_entries),
            gpu_temp: Vec::with_capacity(max_entries),
        }
    }

    pub fn push(&mut self, data: &MonitoringData) {
        // Push new data
        self.cpu_usage.push(data.cpu_usage);
        self.cpu_temp.push(data.cpu_temp);
        self.gpu_usage.push(data.gpu_usage);
        self.gpu_temp.push(data.gpu_temp);

        // Trim to max size
        if self.cpu_usage.len() > self.max_entries {
            self.cpu_usage.remove(0);
            self.cpu_temp.remove(0);
            self.gpu_usage.remove(0);
            self.gpu_temp.remove(0);
        }
    }
}

/// Cached hwmon paths for faster reading
struct HwmonPaths {
    cpu_temp_path: Option<String>,
    gpu_temp_path: Option<String>,
    gpu_busy_path: Option<String>,
    fan_paths: Vec<String>,
}

impl HwmonPaths {
    fn new() -> Self {
        let mut paths = Self {
            cpu_temp_path: None,
            gpu_temp_path: None,
            gpu_busy_path: None,
            fan_paths: Vec::new(),
        };
        paths.discover();
        paths
    }

    /// Discover hwmon paths by reading the 'name' attribute
    fn discover(&mut self) {
        let hwmon_base = Path::new("/sys/class/hwmon");

        if let Ok(entries) = std::fs::read_dir(hwmon_base) {
            for entry in entries.flatten() {
                let hwmon_path = entry.path();
                let name_path = hwmon_path.join("name");

                if let Ok(name) = std::fs::read_to_string(&name_path) {
                    let name = name.trim();
                    debug!("Found hwmon device: {} = {}", hwmon_path.display(), name);

                    match name {
                        // AMD CPU temperature sensors
                        "k10temp" | "zenpower" => {
                            // Tdie is usually temp1 for k10temp, temp2 for zenpower
                            let temp1 = hwmon_path.join("temp1_input");
                            let temp2 = hwmon_path.join("temp2_input");
                            if temp1.exists() {
                                self.cpu_temp_path = Some(temp1.to_string_lossy().to_string());
                                debug!("CPU temp path (AMD): {:?}", self.cpu_temp_path);
                            } else if temp2.exists() {
                                self.cpu_temp_path = Some(temp2.to_string_lossy().to_string());
                                debug!("CPU temp path (AMD): {:?}", self.cpu_temp_path);
                            }
                        }
                        // Intel CPU temperature sensor
                        "coretemp" => {
                            // Package temp is usually temp1
                            let temp1 = hwmon_path.join("temp1_input");
                            if temp1.exists() {
                                self.cpu_temp_path = Some(temp1.to_string_lossy().to_string());
                                debug!("CPU temp path (Intel): {:?}", self.cpu_temp_path);
                            }
                        }
                        // AMD GPU
                        "amdgpu" => {
                            let temp1 = hwmon_path.join("temp1_input");
                            if temp1.exists() {
                                self.gpu_temp_path = Some(temp1.to_string_lossy().to_string());
                                debug!("GPU temp path (AMD): {:?}", self.gpu_temp_path);
                            }
                            // GPU busy percent for usage
                            // This is in the device path, not hwmon
                            if let Some(_device) = hwmon_path.join("device").read_link().ok() {
                                let busy_path = hwmon_path
                                    .join("device")
                                    .join("gpu_busy_percent");
                                if busy_path.exists() {
                                    self.gpu_busy_path =
                                        Some(busy_path.to_string_lossy().to_string());
                                    debug!("GPU busy path: {:?}", self.gpu_busy_path);
                                }
                            }
                        }
                        // NVIDIA GPU (nouveau driver)
                        "nouveau" => {
                            let temp1 = hwmon_path.join("temp1_input");
                            if temp1.exists() {
                                self.gpu_temp_path = Some(temp1.to_string_lossy().to_string());
                                debug!("GPU temp path (nouveau): {:?}", self.gpu_temp_path);
                            }
                        }
                        // ASUS custom fan curve driver
                        "asus_custom_fan_curve" | "asus-nb-wmi" => {
                            // Look for fan inputs
                            for i in 1..=4 {
                                let fan_path = hwmon_path.join(format!("fan{}_input", i));
                                if fan_path.exists() {
                                    self.fan_paths.push(fan_path.to_string_lossy().to_string());
                                    debug!("Fan path: {}", fan_path.display());
                                }
                            }
                        }
                        _ => {
                            // Check for fans in any hwmon device
                            for i in 1..=4 {
                                let fan_path = hwmon_path.join(format!("fan{}_input", i));
                                if fan_path.exists()
                                    && !self.fan_paths.contains(
                                        &fan_path.to_string_lossy().to_string(),
                                    )
                                {
                                    self.fan_paths.push(fan_path.to_string_lossy().to_string());
                                    debug!("Fan path (other): {}", fan_path.display());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback for CPU temp if not found via hwmon name
        if self.cpu_temp_path.is_none() {
            // Try thermal zones
            for i in 0..10 {
                let tz_path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
                let tz_type = format!("/sys/class/thermal/thermal_zone{}/type", i);
                if let Ok(tz_type_content) = std::fs::read_to_string(&tz_type) {
                    let tz_type_name = tz_type_content.trim();
                    // Look for x86_pkg_temp or TCPU
                    if tz_type_name.contains("x86_pkg")
                        || tz_type_name.contains("cpu")
                        || tz_type_name.contains("CPU")
                    {
                        if Path::new(&tz_path).exists() {
                            self.cpu_temp_path = Some(tz_path);
                            debug!("CPU temp path (thermal_zone): {:?}", self.cpu_temp_path);
                            break;
                        }
                    }
                }
            }
        }

        // Last resort fallback for CPU temp
        if self.cpu_temp_path.is_none() {
            let tz0 = "/sys/class/thermal/thermal_zone0/temp";
            if Path::new(tz0).exists() {
                self.cpu_temp_path = Some(tz0.to_string());
                debug!("CPU temp path (fallback): {:?}", self.cpu_temp_path);
            }
        }
    }

    fn read_cpu_temp(&self) -> Option<f32> {
        if let Some(ref path) = self.cpu_temp_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(temp) = content.trim().parse::<f32>() {
                    return Some(temp / 1000.0);
                }
            }
        }
        None
    }

    fn read_gpu_temp(&self) -> Option<f32> {
        if let Some(ref path) = self.gpu_temp_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(temp) = content.trim().parse::<f32>() {
                    return Some(temp / 1000.0);
                }
            }
        }
        None
    }

    fn read_gpu_usage(&self) -> Option<f32> {
        if let Some(ref path) = self.gpu_busy_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(usage) = content.trim().parse::<f32>() {
                    return Some(usage);
                }
            }
        }
        None
    }

    fn read_fan_rpm(&self) -> Vec<u32> {
        self.fan_paths
            .iter()
            .filter_map(|path| {
                std::fs::read_to_string(path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
            })
            .collect()
    }
}

/// System monitor that collects hardware metrics
pub struct SystemMonitor {
    data: Arc<RwLock<MonitoringData>>,
    history: Arc<RwLock<MonitoringHistory>>,
    sys: Arc<RwLock<System>>,
    hwmon: Arc<RwLock<HwmonPaths>>,
    update_interval: Duration,
    running: Arc<RwLock<bool>>,
}

impl SystemMonitor {
    /// Create a new system monitor
    ///
    /// # Arguments
    /// * `update_interval_ms` - How often to update in milliseconds
    /// * `history_duration_sec` - How many seconds of history to keep
    pub fn new(update_interval_ms: u64, history_duration_sec: u64) -> Self {
        let max_entries = (history_duration_sec * 1000 / update_interval_ms) as usize;

        Self {
            data: Arc::new(RwLock::new(MonitoringData::default())),
            history: Arc::new(RwLock::new(MonitoringHistory::new(max_entries))),
            sys: Arc::new(RwLock::new(System::new_all())),
            hwmon: Arc::new(RwLock::new(HwmonPaths::new())),
            update_interval: Duration::from_millis(update_interval_ms),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the monitoring loop in a background task
    pub async fn start(&self) {
        let data = self.data.clone();
        let history = self.history.clone();
        let sys = self.sys.clone();
        let hwmon = self.hwmon.clone();
        let interval = self.update_interval;
        let running = self.running.clone();

        // Mark as running
        *running.write().await = true;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                // Check if we should stop
                if !*running.read().await {
                    break;
                }

                // Update system info
                {
                    let mut sys = sys.write().await;
                    sys.refresh_all();
                }

                // Collect metrics
                let new_data = {
                    let sys = sys.read().await;
                    let hwmon = hwmon.read().await;
                    collect_metrics(&sys, &hwmon)
                };

                // Update data and history
                {
                    let mut data = data.write().await;
                    *data = new_data.clone();
                }

                {
                    let mut history = history.write().await;
                    history.push(&new_data);
                }
            }
        });
    }

    /// Stop the monitoring loop
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Get the current monitoring data snapshot
    pub async fn get_data(&self) -> MonitoringData {
        self.data.read().await.clone()
    }

    /// Get the monitoring history
    pub async fn get_history(&self) -> MonitoringHistory {
        self.history.read().await.clone()
    }

    /// Check if monitoring is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// Collect system metrics from sysinfo
fn collect_metrics(sys: &System, hwmon: &HwmonPaths) -> MonitoringData {
    // Calculate CPU usage from global CPU info
    let cpu_usage = sys.global_cpu_info().cpu_usage();

    // Get CPU frequency (average across all cores)
    let cpu_freq_mhz = if !sys.cpus().is_empty() {
        sys.cpus().iter().map(|cpu| cpu.frequency()).sum::<u64>() / sys.cpus().len() as u64
    } else {
        0
    };

    // Get RAM usage
    let total_memory = sys.total_memory() as f64;
    let used_memory = sys.used_memory() as f64;
    let ram_total_gb = (total_memory / 1024.0 / 1024.0 / 1024.0) as f32;
    let ram_used_gb = (used_memory / 1024.0 / 1024.0 / 1024.0) as f32;
    let ram_usage = if total_memory > 0.0 {
        ((used_memory / total_memory) * 100.0) as f32
    } else {
        0.0
    };

    // CPU temperature from cached hwmon path
    let cpu_temp = hwmon.read_cpu_temp().unwrap_or(0.0);

    // GPU temperature and usage from cached hwmon paths
    let gpu_temp = hwmon.read_gpu_temp().unwrap_or(0.0);
    let gpu_usage = hwmon.read_gpu_usage().unwrap_or(0.0);

    // Fan RPM readings
    let fan_rpm = hwmon.read_fan_rpm();

    MonitoringData {
        cpu_usage,
        cpu_temp,
        cpu_freq_mhz,
        gpu_usage,
        gpu_temp,
        gpu_power_watts: 0.0, // Would need NVML for this
        ram_usage,
        ram_total_gb,
        ram_used_gb,
        fan_rpm,
        power_draw_watts: 0.0,
        battery_percent: read_battery_percent(),
        on_ac_power: read_ac_power_status(),
    }
}

/// Read battery percentage
fn read_battery_percent() -> Option<f32> {
    // Try common battery paths
    let battery_paths = [
        "/sys/class/power_supply/BAT0/capacity",
        "/sys/class/power_supply/BAT1/capacity",
        "/sys/class/power_supply/BATT/capacity",
    ];

    for path in battery_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(percent) = content.trim().parse::<f32>() {
                return Some(percent);
            }
        }
    }
    None
}

/// Read AC power status
fn read_ac_power_status() -> bool {
    let paths = [
        "/sys/class/power_supply/AC/online",
        "/sys/class/power_supply/AC0/online",
        "/sys/class/power_supply/ADP0/online",
        "/sys/class/power_supply/ADP1/online",
        "/sys/class/power_supply/ACAD/online",
    ];

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(status) = content.trim().parse::<u8>() {
                return status == 1;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_buffer() {
        let mut history = MonitoringHistory::new(5);
        let data = MonitoringData {
            cpu_usage: 50.0,
            cpu_temp: 60.0,
            ..Default::default()
        };

        // Add 7 entries, should keep only last 5
        for i in 0..7 {
            let mut d = data.clone();
            d.cpu_usage = i as f32 * 10.0;
            history.push(&d);
        }

        assert_eq!(history.cpu_usage.len(), 5);
        assert_eq!(history.cpu_usage[0], 20.0); // First two were dropped
    }
}
