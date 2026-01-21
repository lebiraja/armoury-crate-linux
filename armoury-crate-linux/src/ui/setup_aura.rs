//! Aura RGB page setup - LED mode, brightness, color controls
//!
//! Connects the Aura page UI to D-Bus Aura interface.

use log::{debug, error, info, warn};
use slint::ComponentHandle;

use crate::{AuraPageData, MainWindow};

/// Initial setup for aura page - fetches current values from D-Bus
pub fn setup_aura_page(ui: &MainWindow) {
    // Connect to system bus (blocking for initial setup)
    let conn = match zbus::blocking::Connection::system() {
        Ok(c) => c,
        Err(e) => {
            error!("D-Bus system connection failed: {:?}", e);
            return;
        }
    };

    // Try to get Aura proxy
    let aura = match rog_dbus::zbus_aura::AuraProxyBlocking::new(&conn) {
        Ok(a) => a,
        Err(e) => {
            warn!("AuraProxy failed: {:?}", e);
            ui.global::<AuraPageData>().set_aura_available(false);
            return;
        }
    };

    ui.global::<AuraPageData>().set_aura_available(true);
    let aura_data = ui.global::<AuraPageData>();

    // Load brightness
    if let Ok(brightness) = aura.brightness() {
        debug!("Aura brightness: {:?}", brightness);
        aura_data.set_brightness(brightness as i32);
    }

    // Load LED mode data
    if let Ok(mode_data) = aura.led_mode_data() {
        debug!("LED mode data: {:?}", mode_data);
        aura_data.set_led_mode(mode_data.mode as i32);
        aura_data.set_led_speed(mode_data.speed as i32);

        // Set color from mode data
        aura_data.set_color_r(mode_data.colour1.r as i32);
        aura_data.set_color_g(mode_data.colour1.g as i32);
        aura_data.set_color_b(mode_data.colour1.b as i32);
    }

    // Load supported modes
    if let Ok(modes) = aura.supported_basic_modes() {
        debug!("Supported modes: {:?}", modes);
        let mode_names: Vec<slint::SharedString> = modes
            .iter()
            .map(|m| format!("{:?}", m).into())
            .collect();
        aura_data.set_available_modes(slint::ModelRc::new(slint::VecModel::from(mode_names)));
    }

    info!("Aura page initialized");
}

/// Setup async callbacks for aura page
pub fn setup_aura_page_callbacks(ui: &MainWindow) {
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

        // Get Aura proxy
        let aura = match rog_dbus::zbus_aura::AuraProxy::new(&conn).await {
            Ok(a) => a,
            Err(e) => {
                warn!("AuraProxy failed: {:?}", e);
                return;
            }
        };

        let aura_copy = aura.clone();
        let handle_copy = handle.clone();

        let _ = handle.upgrade_in_event_loop(move |ui| {
            // Brightness callback
            let aura_inner = aura_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<AuraPageData>().on_cb_brightness(move |brightness| {
                let proxy = aura_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    let bright_enum = match brightness {
                        0 => rog_aura::LedBrightness::Off,
                        1 => rog_aura::LedBrightness::Low,
                        2 => rog_aura::LedBrightness::Med,
                        3 => rog_aura::LedBrightness::High,
                        _ => rog_aura::LedBrightness::Med,
                    };
                    match proxy.set_brightness(bright_enum).await {
                        Ok(_) => {
                            let msg: slint::SharedString =
                                format!("Brightness set to {:?}", bright_enum).into();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = h.upgrade() {
                                    ui.invoke_show_toast(msg);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Failed to set brightness: {:?}", e);
                        }
                    }
                });
            });

            // LED mode callback
            let aura_inner = aura_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<AuraPageData>().on_cb_led_mode(move |mode| {
                let proxy = aura_inner.clone();
                let h = handle_inner.clone();
                tokio::spawn(async move {
                    // Get current mode data and update just the mode
                    if let Ok(mut mode_data) = proxy.led_mode_data().await {
                        mode_data.mode = rog_aura::AuraModeNum::from(mode as u8);
                        match proxy.set_led_mode_data(mode_data).await {
                            Ok(_) => {
                                let msg: slint::SharedString = "LED mode updated".into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Failed to set LED mode: {:?}", e);
                            }
                        }
                    }
                });
            });

            // Color callback
            let aura_inner = aura_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<AuraPageData>()
                .on_cb_color(move |r, g, b| {
                    let proxy = aura_inner.clone();
                    let h = handle_inner.clone();
                    tokio::spawn(async move {
                        if let Ok(mut mode_data) = proxy.led_mode_data().await {
                            // Update color
                            mode_data.colour1 = rog_aura::Colour {
                                r: r as u8,
                                g: g as u8,
                                b: b as u8,
                            };
                            match proxy.set_led_mode_data(mode_data).await {
                                Ok(_) => {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ui) = h.upgrade() {
                                            ui.invoke_show_toast("Color updated".into());
                                        }
                                    });
                                }
                                Err(e) => {
                                    warn!("Failed to set color: {:?}", e);
                                }
                            }
                        }
                    });
                });

            // Speed callback
            let aura_inner = aura_copy.clone();
            ui.global::<AuraPageData>().on_cb_speed(move |speed| {
                let proxy = aura_inner.clone();
                tokio::spawn(async move {
                    if let Ok(mut mode_data) = proxy.led_mode_data().await {
                        mode_data.speed = match speed {
                            0 => rog_aura::Speed::Low,
                            1 => rog_aura::Speed::Med,
                            2 => rog_aura::Speed::High,
                            _ => rog_aura::Speed::Med,
                        };
                        let _ = proxy.set_led_mode_data(mode_data).await;
                    }
                });
            });
        });

        // Setup signal listeners
        let handle_copy = handle.clone();
        let aura_copy = aura.clone();
        tokio::spawn(async move {
            let mut stream = aura_copy.receive_led_mode_data_changed().await;
            use futures_util::StreamExt;
            while let Some(event) = stream.next().await {
                if let Ok(mode_data) = event.get().await {
                    let h = handle_copy.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h.upgrade() {
                            let aura_data = ui.global::<AuraPageData>();
                            aura_data.set_led_mode(mode_data.mode as i32);
                            aura_data.set_led_speed(mode_data.speed as i32);
                            aura_data.set_color_r(mode_data.colour1.r as i32);
                            aura_data.set_color_g(mode_data.colour1.g as i32);
                            aura_data.set_color_b(mode_data.colour1.b as i32);
                        }
                    });
                }
            }
        });
    });
}
