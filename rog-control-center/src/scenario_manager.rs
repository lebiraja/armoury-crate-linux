use log::{info, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioRule {
    pub app_name: String, // e.g., "firefox"
    pub profile: String,  // e.g., "Silent", "Turbo"
}

pub struct ScenarioManager {
    rules: Vec<ScenarioRule>,
    active_profile: Option<String>,
}

impl ScenarioManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            active_profile: None,
        }
    }

    pub fn add_rule(&mut self, app_name: String, profile: String) {
        self.rules.push(ScenarioRule { app_name, profile });
    }

    pub fn check_processes(&mut self, active_processes: &[String]) -> Option<String> {
        // Iterate through rules and check if any matching app is running
        for rule in &self.rules {
            if active_processes.contains(&rule.app_name) {
                // Found a match
                if self.active_profile.as_ref() != Some(&rule.profile) {
                    info!("Scenario match: {} -> {}", rule.app_name, rule.profile);
                    self.active_profile = Some(rule.profile.clone());
                    return Some(rule.profile.clone());
                }
                return None; // Already active, no change needed
            }
        }
        
        // No rules matched, potentially revert to default?
        // For now, we just return None if no *new* rule is triggered.
        // In a full implementation, we'd want a "default" profile to fall back to.
        None
    }
}
