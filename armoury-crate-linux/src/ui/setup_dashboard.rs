//! Dashboard page setup - Real-time system monitoring
//!
//! Displays CPU/GPU temp, usage, RAM, fan speeds, and power metrics

use log::{debug};
use slint::ComponentHandle;
use std::sync::Arc;
use std::time::Duration;

use crate::monitoring::SystemMonitor;
use crate::{DashboardData, MainWindow};

/// Setup the dashboard page with real-time monitoring
pub fn setup_dashboard_page(ui: &MainWindow, monitor: Arc<SystemMonitor>) {
    let handle = ui.as_weak();
    let monitor_clone = monitor.clone();

    // Start the monitoring loop if not already running
    tokio::spawn(async move {
        monitor_clone.start().await;
    });

    // Update UI with monitoring data
    tokio::spawn(async move {
        // Update faster for smoother UI (500ms)
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;

            // Get current monitoring data
            let data = monitor.get_data().await;

            // Get history for graphs
            let history = monitor.get_history().await;

            let handle_copy = handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_copy.upgrade() {
                    let dashboard = ui.global::<DashboardData>();

                    // Update current values
                    dashboard.set_cpu_usage(data.cpu_usage);
                    dashboard.set_cpu_temp(data.cpu_temp);
                    dashboard.set_cpu_freq_ghz(data.cpu_freq_mhz as f32 / 1000.0);

                    dashboard.set_gpu_usage(data.gpu_usage);
                    dashboard.set_gpu_temp(data.gpu_temp);
                    dashboard.set_gpu_power_watts(data.gpu_power_watts);
                    // GPU Frequency requires updated monitoring struct, assuming it's available or mocked for now
                    // dashboard.set_gpu_freq_mhz(data.gpu_freq_mhz as f32);

                    dashboard.set_ram_usage_percent(data.ram_usage);
                    dashboard.set_ram_total_gb(data.ram_total_gb);
                    dashboard.set_ram_used_gb(data.ram_used_gb);

                    // Storage metrics
                    dashboard.set_disk_usage_percent(data.disk_usage);
                    dashboard.set_disk_total_gb(data.disk_total_gb);
                    dashboard.set_disk_used_gb(data.disk_used_gb);

                    // Fan speeds
                    if !data.fan_rpm.is_empty() {
                        dashboard.set_fan1_rpm(data.fan_rpm.get(0).copied().unwrap_or(0) as i32);
                        if data.fan_rpm.len() > 1 {
                            dashboard.set_fan2_rpm(data.fan_rpm.get(1).copied().unwrap_or(0) as i32);
                        }
                    }

                    // Power and battery
                    dashboard.set_power_watts(data.power_draw_watts);
                    dashboard.set_battery_percent(data.battery_percent.unwrap_or(100.0));
                    dashboard.set_on_ac_power(data.on_ac_power);

                    // Update history graphs (convert to VecModel for Slint)
                    dashboard.set_cpu_temp_history(slint::ModelRc::new(slint::VecModel::from(
                        history.cpu_temp.clone(),
                    )));
                    dashboard.set_gpu_temp_history(slint::ModelRc::new(slint::VecModel::from(
                        history.gpu_temp.clone(),
                    )));
                }
            });
        }
    });

    debug!("Dashboard monitoring started");
}
