//! System page setup - Platform profiles, battery controls
//!
//! Connects the System page UI to D-Bus Platform interface.

use log::{debug, error, info, warn};
use slint::ComponentHandle;

use crate::{MainWindow, SystemPageData};

/// Initial setup for system page - fetches current values from D-Bus
pub fn setup_system_page(ui: &MainWindow) {
    // Connect to system bus (blocking for initial setup)
    let conn = match zbus::blocking::Connection::system() {
        Ok(c) => c,
        Err(e) => {
            error!("D-Bus system connection failed: {:?}", e);
            return;
        }
    };

    // Get platform proxy
    let platform = match rog_dbus::zbus_platform::PlatformProxyBlocking::new(&conn) {
        Ok(p) => p,
        Err(e) => {
            warn!("PlatformProxy failed: {:?}", e);
            warn!("Is asusd.service running?");
            return;
        }
    };

    let system_data = ui.global::<SystemPageData>();

    // Set defaults first
    system_data.set_charge_control_end_threshold(-1.0);
    system_data.set_charge_control_enabled(false);
    system_data.set_platform_profile(0);

    // Load platform profile
    if let Ok(profile) = platform.platform_profile() {
        debug!("Platform profile: {:?}", profile);
        system_data.set_platform_profile(profile as i32);
    }

    // Load platform profile choices
    if let Ok(choices) = platform.platform_profile_choices() {
        debug!("Platform profile choices: {:?}", choices);
        let choices_model: Vec<slint::SharedString> = choices
            .iter()
            .map(|p| format!("{:?}", p).into())
            .collect();
        system_data.set_platform_profile_choices(slint::ModelRc::new(
            slint::VecModel::from(choices_model),
        ));
    }

    // Load charge control threshold
    if let Ok(threshold) = platform.charge_control_end_threshold() {
        debug!("Charge threshold: {}", threshold);
        system_data.set_charge_control_end_threshold(threshold as f32);
        system_data.set_charge_control_enabled(threshold < 100);
    }

    info!("System page initialized");
}

/// Setup async callbacks for system page
pub fn setup_system_page_callbacks(ui: &MainWindow) {
    let handle = ui.as_weak();

    tokio::spawn(async move {
        // Create async connection
        let conn = match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                error!("D-Bus system connection failed: {:?}", e);
                return;
            }
        };

        // Get platform proxy
        let platform = match rog_dbus::zbus_platform::PlatformProxy::new(&conn).await {
            Ok(p) => p,
            Err(e) => {
                warn!("PlatformProxy failed: {:?}", e);
                return;
            }
        };

        // Setup callbacks inside event loop
        let platform_copy = platform.clone();
        let handle_copy = handle.clone();

        let _ = handle.upgrade_in_event_loop(move |ui| {
            // Platform profile callback
            let platform_inner = platform_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>().on_cb_platform_profile(move |profile| {
                let proxy = platform_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    let profile_enum = match profile {
                        0 => rog_platform::platform::PlatformProfile::Balanced,
                        1 => rog_platform::platform::PlatformProfile::Performance,
                        2 => rog_platform::platform::PlatformProfile::Quiet,
                        _ => rog_platform::platform::PlatformProfile::Balanced,
                    };
                    match proxy.set_platform_profile(profile_enum).await {
                        Ok(_) => {
                            let msg: slint::SharedString =
                                format!("Profile set to {:?}", profile_enum).into();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.invoke_show_toast(msg);
                                }
                            });
                        }
                        Err(e) => {
                            let msg: slint::SharedString = "Failed to set profile".into();
                            warn!("Failed to set platform profile: {:?}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.invoke_show_toast(msg);
                                }
                            });
                        }
                    }
                });
            });

            // Charge control callback
            let platform_inner = platform_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>()
                .on_cb_charge_control_end_threshold(move |threshold| {
                    let proxy = platform_inner.clone();
                    let h = handle_inner.clone();
                    tokio::spawn(async move {
                        match proxy.set_charge_control_end_threshold(threshold as u8).await {
                            Ok(_) => {
                                let msg: slint::SharedString =
                                    format!("Charge limit set to {}%", threshold).into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                            Err(e) => {
                                let msg: slint::SharedString = "Failed to set charge limit".into();
                                warn!("Failed to set charge limit: {:?}", e);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                        }
                    });
                });

            // Panel OD callback (placeholder - logs only since it requires AsusArmoury)
            ui.global::<SystemPageData>().on_cb_panel_od(move |enabled| {
                debug!("Panel OD toggle requested: {}", enabled);
                // TODO: Implement via AsusArmouryProxy
            });

            // GPU MUX callback (placeholder - logs only since it requires AsusArmoury)
            ui.global::<SystemPageData>().on_cb_gpu_mux_mode(move |mode| {
                debug!("GPU MUX mode change requested: {}", mode);
                // TODO: Implement via AsusArmouryProxy
            });
        });

        // Setup signal listeners for external changes
        let handle_copy = handle.clone();
        let platform_copy = platform.clone();
        tokio::spawn(async move {
            let mut stream = platform_copy.receive_platform_profile_changed().await;
            use futures_util::StreamExt;
            while let Some(event) = stream.next().await {
                if let Ok(profile) = event.get().await {
                    let h = handle_copy.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h.upgrade() {
                            ui.global::<SystemPageData>()
                                .set_platform_profile(profile as i32);
                        }
                    });
                }
            }
        });

        let handle_copy = handle.clone();
        let platform_copy = platform.clone();
        tokio::spawn(async move {
            let mut stream = platform_copy
                .receive_charge_control_end_threshold_changed()
                .await;
            use futures_util::StreamExt;
            while let Some(event) = stream.next().await {
                if let Ok(threshold) = event.get().await {
                    let h = handle_copy.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h.upgrade() {
                            ui.global::<SystemPageData>()
                                .set_charge_control_end_threshold(threshold as f32);
                        }
                    });
                }
            }
        });
    });
}
