//! Configuration for Armoury Crate Linux

use std::fs::create_dir;

use config_traits::{StdConfig, StdConfigLoad1};
use serde::{Deserialize, Serialize};

const CFG_DIR: &str = "rog";
const CFG_FILE_NAME: &str = "armoury-crate-linux.cfg";

/// Notification settings for the application
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnabledNotifications {
    pub enabled: bool,
    pub show_gpu_status: bool,
    pub show_profile_changes: bool,
    pub show_charging_status: bool,
}

/// Main configuration for Armoury Crate Linux
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Theme to use: "rog_gaming" is default
    pub theme: String,
    /// Run application in background when window is closed
    pub run_in_background: bool,
    /// Start with window hidden (background mode)
    pub startup_in_background: bool,
    /// Enable system tray icon
    pub enable_tray_icon: bool,
    /// Start in fullscreen mode (for ROG Ally)
    pub start_fullscreen: bool,
    /// Fullscreen width
    pub fullscreen_width: u32,
    /// Fullscreen height
    pub fullscreen_height: u32,
    /// Default page to show on startup
    pub default_page: String,
    /// System monitoring settings
    pub monitoring: MonitoringConfig,
    /// Notification settings
    pub notifications: EnabledNotifications,
}

/// Configuration for system monitoring
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitoringConfig {
    /// Enable system monitoring
    pub enabled: bool,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
    /// How many seconds of history to keep
    pub history_duration_sec: u64,
    /// Show GPU metrics (may require nvidia driver)
    pub show_gpu_metrics: bool,
    /// Show power consumption metrics
    pub show_power_metrics: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            update_interval_ms: 500,
            history_duration_sec: 60,
            show_gpu_metrics: true,
            show_power_metrics: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "rog_gaming".to_string(),
            run_in_background: true,
            startup_in_background: false,
            enable_tray_icon: true,
            start_fullscreen: false,
            fullscreen_width: 1920,
            fullscreen_height: 1080,
            default_page: "dashboard".to_string(),
            monitoring: MonitoringConfig::default(),
            notifications: EnabledNotifications {
                enabled: true,
                show_gpu_status: true,
                show_profile_changes: true,
                show_charging_status: true,
            },
        }
    }
}

impl StdConfig for Config {
    fn new() -> Self {
        Config::default()
    }

    fn file_name(&self) -> String {
        CFG_FILE_NAME.to_owned()
    }

    fn config_dir() -> std::path::PathBuf {
        let mut path = dirs::config_dir().unwrap_or_default();

        path.push(CFG_DIR);
        if !path.exists() {
            create_dir(path.clone())
                .map_err(|e| log::error!("Could not create config dir: {e}"))
                .ok();
            log::info!("Created {path:?}");
        }
        path
    }
}

// Support loading older config versions
impl StdConfigLoad1<Config> for Config {}
