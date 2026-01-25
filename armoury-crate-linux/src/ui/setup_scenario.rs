//! Scenario profiles page setup
//!
//! Auto-switch power profiles and Aura modes based on running applications

use log::{debug};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;
use uuid::Uuid;

use crate::scenario_manager::{AuraSettings, ScenarioManager, ScenarioRule};
use crate::{MainWindow, ScenarioData, ScenarioRuleSlint};

/// Setup the scenario profiles page
pub fn setup_scenario_page(ui: &MainWindow, manager: Arc<ScenarioManager>) {
    let handle = ui.as_weak();
    let manager_clone = manager.clone();

    // Load existing rules from config
    refresh_rules_ui(&handle, manager.clone());

    // Handle add rule button
    let handle_add = handle.clone();
    let manager_add = manager.clone();
    ui.global::<ScenarioData>()
        .on_add_scenario_rule(move |process, profile, aura_mode| {
            let process_str = process.to_string();
            let profile_str = profile.to_string();

            debug!("Adding scenario rule: {} -> {}, Aura: {}", process_str, profile_str, aura_mode);

            let rule = ScenarioRule {
                id: Uuid::new_v4().to_string(),
                name: format!("{} profile", process_str),
                process_name: process_str.clone(),
                power_profile: Some(profile_str.clone()),
                aura_mode: if aura_mode >= 0 {
                    Some(AuraSettings {
                        mode: "Static".to_string(),
                        color_r: 255,
                        color_g: 0,
                        color_b: 0,
                        brightness: 100,
                    })
                } else {
                    None
                },
                fan_profile: None,
                priority: 50, // Default priority
            };

            let manager_clone = manager_add.clone();
            let handle_refresh = handle_add.clone();
            let manager_refresh = manager_add.clone();
            tokio::spawn(async move {
                manager_clone.add_rule(rule).await;
                // Refresh UI after adding
                refresh_rules_ui_async(&handle_refresh, manager_refresh).await;
            });
        });

    // Handle remove rule button
    let handle_remove = handle.clone();
    let manager_remove = manager.clone();
    ui.global::<ScenarioData>()
        .on_remove_scenario_rule(move |rule_id| {
            let rule_id_str = rule_id.to_string();
            debug!("Removing scenario rule: {}", rule_id_str);

            let manager_clone = manager_remove.clone();
            let handle_refresh = handle_remove.clone();
            let manager_refresh = manager_remove.clone();
            tokio::spawn(async move {
                manager_clone.remove_rule(&rule_id_str).await;
                // Refresh UI after removing
                refresh_rules_ui_async(&handle_refresh, manager_refresh).await;
            });
        });

    // Handle enable/disable toggle
    ui.global::<ScenarioData>()
        .on_toggle_scenario_enabled(move |enabled| {
            debug!("Scenario auto-switching: {}", if enabled { "enabled" } else { "disabled" });

            let manager_clone = manager_clone.clone();
            tokio::spawn(async move {
                manager_clone.set_enabled(enabled).await;
            });
        });

    debug!("Scenario page setup complete");
}

/// Refresh the rules list in the UI (sync wrapper)
fn refresh_rules_ui(handle: &slint::Weak<MainWindow>, manager: Arc<ScenarioManager>) {
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        refresh_rules_ui_async(&handle_clone, manager).await;
    });
}

/// Refresh the rules list in the UI (async)
async fn refresh_rules_ui_async(handle: &slint::Weak<MainWindow>, manager: Arc<ScenarioManager>) {
    let rules = manager.get_rules().await;
    let config = manager.config.read().await;
    let is_enabled = config.enabled;
    drop(config);

    let handle_copy = handle.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = handle_copy.upgrade() {
            let scenario_data = ui.global::<ScenarioData>();

            // Convert rules to Slint format
            let slint_rules: Vec<ScenarioRuleSlint> = rules
                .iter()
                .map(|r| ScenarioRuleSlint {
                    id: r.id.clone().into(),
                    process_name: r.process_name.clone().into(),
                    profile: r.power_profile.clone().unwrap_or_default().into(),
                    aura_mode: -1, // Simplified for now
                    priority: r.priority as i32,
                })
                .collect();

            scenario_data.set_rules(ModelRc::new(VecModel::from(slint_rules)));
            scenario_data.set_enabled(is_enabled);
        }
    });
}
