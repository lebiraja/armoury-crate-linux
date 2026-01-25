//! System page setup - Platform profiles, battery controls
//!
//! Connects the System page UI to D-Bus Platform interface.

use log::{debug, error, info, warn};
use rog_dbus::asus_armoury::{AsusArmouryProxy, AsusArmouryProxyBlocking};
use rog_platform::asus_armoury::FirmwareAttribute;
use slint::ComponentHandle;

use crate::zbus_proxies::{find_iface, find_iface_async};
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
    system_data.set_panel_od(false);
    system_data.set_gpu_mux_mode(0);

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

    // Load AsusArmoury attributes (Panel OD, GPU MUX)
    if let Ok(attrs) = find_iface::<AsusArmouryProxyBlocking>("xyz.ljones.AsusArmoury") {
        for attr in attrs {
            if let (Ok(name), Ok(val)) = (attr.name(), attr.current_value()) {
                match name {
                    FirmwareAttribute::PanelOverdrive => {
                        system_data.set_panel_od(val == 1);
                    }
                    FirmwareAttribute::GpuMuxMode => {
                        system_data.set_gpu_mux_mode(val);
                    }
                    _ => {}
                }
            }
        }
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

        // Get AsusArmoury proxies
        let armoury_attrs = find_iface_async::<AsusArmouryProxy>("xyz.ljones.AsusArmoury")
            .await
            .unwrap_or_default();

        // Setup callbacks inside event loop
        let platform_copy = platform.clone();
        let handle_copy = handle.clone();
        let armoury_attrs_copy = armoury_attrs.clone();

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
                        3 => rog_platform::platform::PlatformProfile::Performance, // Turbo mapped to Performance for now
                        4 => rog_platform::platform::PlatformProfile::Custom,      // Manual mapped to Custom
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

            // PPT SPL (PL1) callback
            let attrs_inner = armoury_attrs_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>().on_cb_ppt_pl1_spl(move |value| {
                let attrs = attrs_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    for attr in attrs {
                        if let Ok(name) = attr.name().await {
                            if name == FirmwareAttribute::PptPl1Spl {
                                if let Err(e) = attr.set_current_value(value as i32).await {
                                    warn!("Failed to set SPL: {:?}", e);
                                } else {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = h.upgrade() {
                                            ui.invoke_show_toast(format!("SPL set to {}W", value).into());
                                        }
                                    });
                                }
                                break;
                            }
                        }
                    }
                });
            });

            // PPT SPPT (PL2) callback
            let attrs_inner = armoury_attrs_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>().on_cb_ppt_pl2_sppt(move |value| {
                let attrs = attrs_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    for attr in attrs {
                        if let Ok(name) = attr.name().await {
                            if name == FirmwareAttribute::PptPl2Sppt {
                                if let Err(e) = attr.set_current_value(value as i32).await {
                                    warn!("Failed to set SPPT: {:?}", e);
                                } else {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = h.upgrade() {
                                            ui.invoke_show_toast(format!("SPPT set to {}W", value).into());
                                        }
                                    });
                                }
                                break;
                            }
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

            // Panel OD callback
            let attrs_inner = armoury_attrs_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>().on_cb_panel_od(move |enabled| {
                let attrs = attrs_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    for attr in attrs {
                        if let Ok(name) = attr.name().await {
                            if name == FirmwareAttribute::PanelOverdrive {
                                let val = if enabled { 1 } else { 0 };
                                if let Err(e) = attr.set_current_value(val).await {
                                    warn!("Failed to set Panel OD: {:?}", e);
                                } else {
                                    let msg: slint::SharedString = format!("Panel Overdrive {}", if enabled { "enabled" } else { "disabled" }).into();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = h.upgrade() {
                                            ui.invoke_show_toast(msg);
                                        }
                                    });
                                }
                                break;
                            }
                        }
                    }
                });
            });

            // GPU MUX callback
            let attrs_inner = armoury_attrs_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<SystemPageData>().on_cb_gpu_mux_mode(move |mode| {
                let attrs = attrs_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    for attr in attrs {
                        if let Ok(name) = attr.name().await {
                            if name == FirmwareAttribute::GpuMuxMode {
                                if let Err(e) = attr.set_current_value(mode).await {
                                    warn!("Failed to set GPU MUX: {:?}", e);
                                } else {
                                    let msg: slint::SharedString = "GPU Mode changed - Reboot required".into();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = h.upgrade() {
                                            ui.invoke_show_toast(msg);
                                        }
                                    });
                                }
                                break;
                            }
                        }
                    }
                });
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
