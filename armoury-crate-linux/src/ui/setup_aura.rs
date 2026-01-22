//! Aura RGB page setup
//!
//! Connects the Aura page UI to D-Bus Aura interface.

use log::{debug, error, warn};
use slint::ComponentHandle;

use crate::ui::show_toast;
use crate::{AuraPageData, MainWindow};
use rog_aura::{AuraModeNum, LedBrightness};
use rog_dbus::zbus_aura::AuraProxy;

/// Find the first available Aura D-Bus interface
async fn find_aura_iface(
    conn: &zbus::Connection,
) -> Result<AuraProxy<'static>, Box<dyn std::error::Error>> {
    let f = zbus::fdo::ObjectManagerProxy::new(conn, "xyz.ljones.Asusd", "/").await?;
    let interfaces = f.get_managed_objects().await?;
    let mut aura_paths = Vec::new();

    for (path, iface_map) in interfaces.iter() {
        if iface_map.contains_key("xyz.ljones.Asusd.Aura") {
            aura_paths.push(path.clone());
        }
    }

    if aura_paths.is_empty() {
        return Err("No Aura interfaces found".into());
    }

    // Use the first available interface - convert to OwnedObjectPath for 'static lifetime
    let aura_path: zbus::zvariant::OwnedObjectPath = aura_paths[0].clone().into();
    debug!("Using Aura interface at: {}", aura_path);
    let proxy = AuraProxy::builder(conn)
        .path(aura_path)?
        .build()
        .await?;
    Ok(proxy)
}

pub fn setup_aura_page(ui: &MainWindow) {
    let handle_weak = ui.as_weak();

    tokio::spawn(async move {
        let conn = match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to D-Bus: {:?}", e);
                return;
            }
        };

        let aura = match find_aura_iface(&conn).await {
            Ok(proxy) => proxy,
            Err(e) => {
                warn!("Aura interface not available: {}", e);
                let h = handle_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.global::<AuraPageData>().set_aura_available(false);
                    }
                });
                return;
            }
        };

        // Mark Aura as available
        let h = handle_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = h.upgrade() {
                ui.global::<AuraPageData>().set_aura_available(true);
            }
        });

        // 1. Load current brightness
        if let Ok(brightness) = aura.brightness().await {
            let brightness_val = brightness as i32;
            let h = handle_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = h.upgrade() {
                    ui.global::<AuraPageData>().set_brightness(brightness_val);
                }
            });
        }

        // 2. Load current LED mode and data
        if let Ok(mode_data) = aura.led_mode_data().await {
            let h = handle_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = h.upgrade() {
                    // Set LED mode and properties from mode_data
                    ui.global::<AuraPageData>().set_led_mode(i32::from(mode_data.mode));
                    ui.global::<AuraPageData>().set_led_speed(mode_data.speed as i32);
                    ui.global::<AuraPageData>().set_color_r(mode_data.colour1.r as i32);
                    ui.global::<AuraPageData>().set_color_g(mode_data.colour1.g as i32);
                    ui.global::<AuraPageData>().set_color_b(mode_data.colour1.b as i32);
                }
            });
        }

        // Note: The UI currently only supports basic mode selection via cb_led_mode callback
        // Additional mode support could be added to the Slint UI in the future
    });
}

pub fn setup_aura_page_callbacks(ui: &MainWindow) {
    let handle = ui.as_weak();
    let handle_weak = handle.clone();

    tokio::spawn(async move {
        let conn = match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to D-Bus for callbacks: {:?}", e);
                return;
            }
        };

        let aura = match find_aura_iface(&conn).await {
            Ok(proxy) => proxy,
            Err(e) => {
                warn!("Aura interface not available for callbacks: {}", e);
                return;
            }
        };

        let aura_copy = aura.clone();
        let handle_for_callbacks = handle.clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = handle_weak.upgrade() {
                // Brightness callback
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_brightness(move |value| {
                    let p = proxy.clone();
                    let h_inner = h.clone();
                    tokio::spawn(async move {
                        let brightness = match value {
                            0 => LedBrightness::Off,
                            1 => LedBrightness::Low,
                            2 => LedBrightness::Med,
                            3 => LedBrightness::High,
                            _ => LedBrightness::Med,
                        };
                        let res = p.set_brightness(brightness).await;
                        show_toast(
                            format!("Brightness: {:?}", brightness).into(),
                            "Failed to set brightness".into(),
                            h_inner,
                            res,
                        );
                    });
                });

                // LED Mode callback
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_led_mode(move |mode_index| {
                    let p = proxy.clone();
                    let h_inner = h.clone();
                    tokio::spawn(async move {
                        let mode: AuraModeNum = (mode_index as i32).into();
                        // Get current mode data to preserve color and speed
                        if let Ok(mut mode_data) = p.led_mode_data().await {
                            mode_data.mode = mode;
                            let res = p.set_led_mode_data(mode_data).await;
                            show_toast(
                                format!("LED mode: {:?}", mode).into(),
                                "Failed to set LED mode".into(),
                                h_inner,
                                res,
                            );
                        }
                    });
                });

                // LED Speed callback
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_speed(move |speed_index| {
                    let p = proxy.clone();
                    let h_inner = h.clone();
                    tokio::spawn(async move {
                        let speed = match speed_index {
                            0 => rog_aura::Speed::Low,
                            1 => rog_aura::Speed::Med,
                            2 => rog_aura::Speed::High,
                            _ => rog_aura::Speed::Med,
                        };
                        if let Ok(mut mode_data) = p.led_mode_data().await {
                            mode_data.speed = speed;
                            let res = p.set_led_mode_data(mode_data).await;
                            show_toast(
                                format!("Speed: {:?}", speed).into(),
                                "Failed to set speed".into(),
                                h_inner,
                                res,
                            );
                        }
                    });
                });

                // Color callback
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_color(move |r, g, b| {
                    let p = proxy.clone();
                    let h_inner = h.clone();
                    tokio::spawn(async move {
                        if let Ok(mut mode_data) = p.led_mode_data().await {
                            mode_data.colour1 = rog_aura::Colour {
                                r: r as u8,
                                g: g as u8,
                                b: b as u8,
                            };
                            let res = p.set_led_mode_data(mode_data).await;
                            show_toast(
                                format!("Color: RGB({}, {}, {})", r, g, b).into(),
                                "Failed to set color".into(),
                                h_inner,
                                res,
                            );
                        }
                    });
                });
        }
        });

        // 4. Setup Signal Listeners for LED mode changes
        let aura_copy = aura.clone();
        let h = handle.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut stream = aura_copy.receive_led_mode_data_changed().await;
            while let Some(event) = stream.next().await {
                if let Ok(value) = event.get().await {
                    let h_inner = h.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h_inner.upgrade() {
                            // Update UI with new effect data
                            ui.global::<AuraPageData>().set_led_mode(i32::from(value.mode));
                            ui.global::<AuraPageData>().set_led_speed(value.speed as i32);
                            ui.global::<AuraPageData>().set_color_r(value.colour1.r as i32);
                            ui.global::<AuraPageData>().set_color_g(value.colour1.g as i32);
                            ui.global::<AuraPageData>().set_color_b(value.colour1.b as i32);
                        }
                    });
                }
            }
        });
    });
}
