//! Settings page setup - Application configuration
//!
//! Connects the Settings page UI to the application config.

use std::sync::{Arc, Mutex};

use config_traits::StdConfig;
use log::{debug, info};
use slint::ComponentHandle;

use crate::config::Config;
use crate::{MainWindow, SettingsData};

/// Initial setup for settings page - loads current config into UI
pub fn setup_settings_page(ui: &MainWindow, config: Arc<Mutex<Config>>) {
    let cfg = config.lock().unwrap();
    let settings = ui.global::<SettingsData>();

    // Application settings
    settings.set_run_in_background(cfg.run_in_background);
    settings.set_startup_in_background(cfg.startup_in_background);
    settings.set_enable_tray_icon(cfg.enable_tray_icon);
    settings.set_start_fullscreen(cfg.start_fullscreen);

    // Notification settings
    settings.set_notifications_enabled(cfg.notifications.enabled);
    settings.set_notify_gpu_status(cfg.notifications.show_gpu_status);
    settings.set_notify_profile_changes(cfg.notifications.show_profile_changes);
    settings.set_notify_charging_status(cfg.notifications.show_charging_status);

    // Monitoring settings
    settings.set_monitoring_enabled(cfg.monitoring.enabled);
    settings.set_update_interval_ms(cfg.monitoring.update_interval_ms as i32);
    settings.set_show_gpu_metrics(cfg.monitoring.show_gpu_metrics);
    settings.set_show_power_metrics(cfg.monitoring.show_power_metrics);

    // Power commands
    settings.set_ac_command(cfg.ac_command.clone().into());
    settings.set_bat_command(cfg.bat_command.clone().into());

    drop(cfg);

    info!("Settings page initialized with config");
}

/// Setup callbacks for settings page
pub fn setup_settings_page_callbacks(ui: &MainWindow, config: Arc<Mutex<Config>>) {
    let handle = ui.as_weak();

    // Save settings callback
    let config_save = config.clone();
    let handle_save = handle.clone();
    ui.global::<SettingsData>().on_cb_save_settings(move || {
        if let Some(ui) = handle_save.upgrade() {
            let settings = ui.global::<SettingsData>();
            let mut cfg = config_save.lock().unwrap();

            // Application settings
            cfg.run_in_background = settings.get_run_in_background();
            cfg.startup_in_background = settings.get_startup_in_background();
            cfg.enable_tray_icon = settings.get_enable_tray_icon();
            cfg.start_fullscreen = settings.get_start_fullscreen();

            // Notification settings
            cfg.notifications.enabled = settings.get_notifications_enabled();
            cfg.notifications.show_gpu_status = settings.get_notify_gpu_status();
            cfg.notifications.show_profile_changes = settings.get_notify_profile_changes();
            cfg.notifications.show_charging_status = settings.get_notify_charging_status();

            // Monitoring settings
            cfg.monitoring.enabled = settings.get_monitoring_enabled();
            cfg.monitoring.update_interval_ms = settings.get_update_interval_ms() as u64;
            cfg.monitoring.show_gpu_metrics = settings.get_show_gpu_metrics();
            cfg.monitoring.show_power_metrics = settings.get_show_power_metrics();

            // Power commands
            cfg.ac_command = settings.get_ac_command().to_string();
            cfg.bat_command = settings.get_bat_command().to_string();

            // Save to file
            cfg.write();
            debug!("Config saved successfully");
            ui.invoke_show_toast("Settings saved".into());
        }
    });

    // Reset to defaults callback
    let config_reset = config.clone();
    let handle_reset = handle.clone();
    ui.global::<SettingsData>().on_cb_reset_defaults(move || {
        if let Some(ui) = handle_reset.upgrade() {
            let default_cfg = Config::default();
            let settings = ui.global::<SettingsData>();

            // Application settings
            settings.set_run_in_background(default_cfg.run_in_background);
            settings.set_startup_in_background(default_cfg.startup_in_background);
            settings.set_enable_tray_icon(default_cfg.enable_tray_icon);
            settings.set_start_fullscreen(default_cfg.start_fullscreen);

            // Notification settings
            settings.set_notifications_enabled(default_cfg.notifications.enabled);
            settings.set_notify_gpu_status(default_cfg.notifications.show_gpu_status);
            settings.set_notify_profile_changes(default_cfg.notifications.show_profile_changes);
            settings.set_notify_charging_status(default_cfg.notifications.show_charging_status);

            // Monitoring settings
            settings.set_monitoring_enabled(default_cfg.monitoring.enabled);
            settings.set_update_interval_ms(default_cfg.monitoring.update_interval_ms as i32);
            settings.set_show_gpu_metrics(default_cfg.monitoring.show_gpu_metrics);
            settings.set_show_power_metrics(default_cfg.monitoring.show_power_metrics);

            // Power commands
            settings.set_ac_command(default_cfg.ac_command.clone().into());
            settings.set_bat_command(default_cfg.bat_command.clone().into());

            // Update config
            let mut cfg = config_reset.lock().unwrap();
            *cfg = default_cfg;

            // Save to file
            cfg.write();

            ui.invoke_show_toast("Settings reset to defaults".into());
        }
    });

    // Individual toggle callbacks - these update config immediately
    let config_bg = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_run_in_background(move |value| {
            let mut cfg = config_bg.lock().unwrap();
            cfg.run_in_background = value;
            debug!("run_in_background = {}", value);
        });

    let config_startup = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_startup_in_background(move |value| {
            let mut cfg = config_startup.lock().unwrap();
            cfg.startup_in_background = value;
            debug!("startup_in_background = {}", value);
        });

    let config_tray = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_tray_icon(move |value| {
            let mut cfg = config_tray.lock().unwrap();
            cfg.enable_tray_icon = value;
            debug!("enable_tray_icon = {}", value);
        });

    let config_notif = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_notifications(move |value| {
            let mut cfg = config_notif.lock().unwrap();
            cfg.notifications.enabled = value;
            debug!("notifications.enabled = {}", value);
        });

    let config_gpu = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_notify_gpu(move |value| {
            let mut cfg = config_gpu.lock().unwrap();
            cfg.notifications.show_gpu_status = value;
            debug!("notifications.show_gpu_status = {}", value);
        });

    let config_profile = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_notify_profile(move |value| {
            let mut cfg = config_profile.lock().unwrap();
            cfg.notifications.show_profile_changes = value;
            debug!("notifications.show_profile_changes = {}", value);
        });

    let config_charging = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_notify_charging(move |value| {
            let mut cfg = config_charging.lock().unwrap();
            cfg.notifications.show_charging_status = value;
            debug!("notifications.show_charging_status = {}", value);
        });

    let config_mon = config.clone();
    ui.global::<SettingsData>()
        .on_cb_toggle_monitoring(move |value| {
            let mut cfg = config_mon.lock().unwrap();
            cfg.monitoring.enabled = value;
            debug!("monitoring.enabled = {}", value);
        });

    let config_interval = config.clone();
    ui.global::<SettingsData>()
        .on_cb_set_update_interval(move |value| {
            let mut cfg = config_interval.lock().unwrap();
            cfg.monitoring.update_interval_ms = value as u64;
            debug!("monitoring.update_interval_ms = {}", value);
        });

    let config_ac = config.clone();
    ui.global::<SettingsData>().on_cb_set_ac_command(move |value| {
        let mut cfg = config_ac.lock().unwrap();
        cfg.ac_command = value.to_string();
        debug!("ac_command = {}", value);
    });

    let config_bat = config.clone();
    ui.global::<SettingsData>()
        .on_cb_set_bat_command(move |value| {
            let mut cfg = config_bat.lock().unwrap();
            cfg.bat_command = value.to_string();
            debug!("bat_command = {}", value);
        });
}
