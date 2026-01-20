//! System monitoring module for Armoury Crate Linux
//!
//! Provides real-time system metrics including:
//! - CPU usage, temperature, and frequency
//! - GPU usage and temperature (via hwmon or NVML)
//! - RAM usage
//! - Fan RPM readings
//! - Power consumption

use std::sync::Arc;
use std::time::Duration;

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

/// System monitor that collects hardware metrics
pub struct SystemMonitor {
    data: Arc<RwLock<MonitoringData>>,
    history: Arc<RwLock<MonitoringHistory>>,
    sys: Arc<RwLock<System>>,
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
            update_interval: Duration::from_millis(update_interval_ms),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the monitoring loop in a background task
    pub async fn start(&self) {
        let data = self.data.clone();
        let history = self.history.clone();
        let sys = self.sys.clone();
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
                    collect_metrics(&sys)
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
fn collect_metrics(sys: &System) -> MonitoringData {
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

    // CPU temperature - try to read from hwmon
    let cpu_temp = read_cpu_temperature().unwrap_or(0.0);

    // GPU temperature and usage - try to read from hwmon
    let (gpu_temp, gpu_usage) = read_gpu_metrics().unwrap_or((0.0, 0.0));

    // Fan RPM readings
    let fan_rpm = read_fan_rpm().unwrap_or_default();

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

/// Read CPU temperature from hwmon
fn read_cpu_temperature() -> Option<f32> {
    // Try common hwmon paths for CPU temperature
    let paths = [
        "/sys/class/hwmon/hwmon0/temp1_input",
        "/sys/class/hwmon/hwmon1/temp1_input",
        "/sys/class/hwmon/hwmon2/temp1_input",
        "/sys/class/thermal/thermal_zone0/temp",
    ];

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(temp) = content.trim().parse::<f32>() {
                // hwmon reports in millidegrees, thermal_zone in millidegrees too
                return Some(temp / 1000.0);
            }
        }
    }
    None
}

/// Read GPU temperature and usage from hwmon or NVIDIA
fn read_gpu_metrics() -> Option<(f32, f32)> {
    // Try NVIDIA first via hwmon
    // Look for amdgpu or nvidia-smi paths
    let gpu_temp_paths = [
        "/sys/class/hwmon/hwmon1/temp1_input",
        "/sys/class/hwmon/hwmon2/temp1_input",
        "/sys/class/hwmon/hwmon3/temp1_input",
    ];

    let mut temp = 0.0;
    for path in gpu_temp_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(t) = content.trim().parse::<f32>() {
                temp = t / 1000.0;
                break;
            }
        }
    }

    // GPU usage is harder to get without NVML
    // Return 0 for now
    Some((temp, 0.0))
}

/// Read fan RPM from hwmon
fn read_fan_rpm() -> Option<Vec<u32>> {
    let mut rpms = Vec::new();

    // Look for fan inputs in hwmon
    for hwmon_id in 0..10 {
        for fan_id in 1..5 {
            let path = format!("/sys/class/hwmon/hwmon{}/fan{}_input", hwmon_id, fan_id);
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(rpm) = content.trim().parse::<u32>() {
                    rpms.push(rpm);
                }
            }
        }
    }

    if rpms.is_empty() {
        None
    } else {
        Some(rpms)
    }
}

/// Read battery percentage
fn read_battery_percent() -> Option<f32> {
    let path = "/sys/class/power_supply/BAT0/capacity";
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(percent) = content.trim().parse::<f32>() {
            return Some(percent);
        }
    }

    // Try BAT1
    let path = "/sys/class/power_supply/BAT1/capacity";
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(percent) = content.trim().parse::<f32>() {
            return Some(percent);
        }
    }

    None
}

/// Read AC power status
fn read_ac_power_status() -> bool {
    let paths = [
        "/sys/class/power_supply/AC/online",
        "/sys/class/power_supply/AC0/online",
        "/sys/class/power_supply/ADP1/online",
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
