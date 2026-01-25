//! Scenario Profile Manager
//!
//! Automatically switches system profiles based on running applications.
//! Monitors processes and applies configured rules for power profiles, Aura effects, etc.

use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// A scenario rule that triggers profile changes
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScenarioRule {
    /// Unique ID for the rule
    pub id: String,
    /// User-friendly name
    pub name: String,
    /// Process name to match (e.g., "firefox", "cyberpunk2077")
    pub process_name: String,
    /// Power profile to apply ("Silent", "Balanced", "Performance", "Turbo")
    pub power_profile: Option<String>,
    /// Aura mode to apply
    pub aura_mode: Option<AuraSettings>,
    /// Fan profile name
    pub fan_profile: Option<String>,
    /// Priority (higher = more important, 0-100)
    pub priority: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuraSettings {
    pub mode: String,  // "Static", "Breathe", "Rainbow", etc.
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub brightness: u8,
}

/// Configuration for scenario profiles
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub enabled: bool,
    pub rules: Vec<ScenarioRule>,
    pub default_profile: Option<String>,
    pub check_interval_ms: u64,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Vec::new(),
            default_profile: Some("Balanced".to_string()),
            check_interval_ms: 2000, // Check every 2 seconds
        }
    }
}

/// Manages scenario profile switching
pub struct ScenarioManager {
    pub config: Arc<RwLock<ScenarioConfig>>,
    config_path: PathBuf,
    active_rule: Arc<RwLock<Option<String>>>, // Currently active rule ID
}

impl ScenarioManager {
    /// Create a new scenario manager
    pub fn new(config_path: PathBuf) -> Self {
        let config = Self::load_config(&config_path).unwrap_or_default();
        
        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            active_rule: Arc::new(RwLock::new(None)),
        }
    }

    /// Load configuration from file
    fn load_config(path: &PathBuf) -> Result<ScenarioConfig, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: ScenarioConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(ScenarioConfig::default())
        }
    }

    /// Save configuration to file
    pub async fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = self.config.read().await;
        let content = toml::to_string_pretty(&*config)?;
        
        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// Add a new scenario rule
    pub async fn add_rule(&self, rule: ScenarioRule) {
        let mut config = self.config.write().await;
        config.rules.push(rule);
    }

    /// Remove a scenario rule by ID
    pub async fn remove_rule(&self, id: &str) {
        let mut config = self.config.write().await;
        config.rules.retain(|r| r.id != id);
    }

    /// Get all rules
    pub async fn get_rules(&self) -> Vec<ScenarioRule> {
        let config = self.config.read().await;
        config.rules.clone()
    }

    /// Enable/disable scenario system
    pub async fn set_enabled(&self, enabled: bool) {
        let mut config = self.config.write().await;
        config.enabled = enabled;
    }

    /// Check running processes and apply matching rule
    pub async fn check_and_apply(&self) -> Option<ScenarioRule> {
        let config = self.config.read().await;
        
        if !config.enabled {
            return None;
        }

        // Get list of running processes
        let processes = Self::get_running_processes();
        
        // Sort rules by priority (highest first)
        let mut sorted_rules = config.rules.clone();
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Find first matching rule
        for rule in sorted_rules {
            if processes.iter().any(|p| p.contains(&rule.process_name)) {
                let mut active = self.active_rule.write().await;
                
                // Only apply if different from current
                if active.as_ref() != Some(&rule.id) {
                    info!("Scenario triggered: {} (process: {})", rule.name, rule.process_name);
                    *active = Some(rule.id.clone());
                    return Some(rule);
                }
                
                return None; // Already active
            }
        }

        // No rules matched - revert to default if needed
        let mut active = self.active_rule.write().await;
        if active.is_some() {
            info!("No scenario rules match, reverting to default profile");
            *active = None;
            
            // Return a pseudo-rule for default profile
            if let Some(default_profile) = &config.default_profile {
                return Some(ScenarioRule {
                    id: "default".to_string(),
                    name: "Default".to_string(),
                    process_name: String::new(),
                    power_profile: Some(default_profile.clone()),
                    aura_mode: None,
                    fan_profile: None,
                    priority: 0,
                });
            }
        }

        None
    }

    /// Get list of running process names from /proc
    fn get_running_processes() -> Vec<String> {
        let mut processes = Vec::new();
        
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let path = entry.path();
                
                // Check if directory name is a number (PID)
                if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        if name_str.chars().all(|c| c.is_numeric()) {
                            // Read process cmdline
                            let cmdline_path = path.join("cmdline");
                            if let Ok(cmdline) = fs::read_to_string(cmdline_path) {
                                // Extract executable name
                                if let Some(exe) = cmdline.split('\0').next() {
                                    if !exe.is_empty() {
                                        // Get base name without path
                                        if let Some(base) = exe.split('/').last() {
                                            processes.push(base.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        debug!("Found {} running processes", processes.len());
        processes
    }

    /// Start background monitoring loop
    pub async fn start_monitoring(
        self: Arc<Self>,
        profile_callback: impl Fn(ScenarioRule) + Send + 'static,
    ) {
        let config = self.config.read().await;
        let check_interval_ms = config.check_interval_ms;
        drop(config);

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(check_interval_ms));
            
            loop {
                ticker.tick().await;
                
                if let Some(rule) = self.check_and_apply().await {
                    profile_callback(rule);
                }
            }
        });
        
        info!("Scenario manager monitoring started");
    }
}
