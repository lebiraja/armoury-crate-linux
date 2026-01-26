//! Aura RGB page setup
//!
//! Connects the Aura page UI to D-Bus Aura interface.

use std::rc::Rc;
use log::{debug, error, warn};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::ui::show_toast;
use crate::{AuraPageData, MainWindow};
use rog_aura::{AuraDeviceType, AuraModeNum, LedBrightness};
use rog_dbus::zbus_aura::AuraProxy;

/// Convert HSV (0-1 range) to RGB (0-255 range)
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    if s <= 0.0 {
        let val = (v * 255.0) as u8;
        return (val, val, val);
    }

    let h = h * 6.0; // sector 0 to 5
    let sector = h.floor() as i32;
    let f = h - sector as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let (r, g, b) = match sector % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Find the first available Aura D-Bus interface
async fn find_aura_iface(
    conn: &zbus::Connection,
) -> Result<AuraProxy<'static>, Box<dyn std::error::Error>> {
    // Method 1: Try ObjectManager at root "/"
    debug!("Attempting Aura discovery via ObjectManager at /");
    if let Ok(f) = zbus::fdo::ObjectManagerProxy::new(conn, "xyz.ljones.Asusd", "/").await {
        if let Ok(interfaces) = f.get_managed_objects().await {
            for (path, iface_map) in interfaces.iter() {
                if iface_map.contains_key("xyz.ljones.Aura") {
                    let aura_path: zbus::zvariant::OwnedObjectPath = path.clone().into();
                    debug!("Found Aura interface via ObjectManager(/) at: {}", aura_path);
                    let proxy = AuraProxy::builder(conn)
                        .path(aura_path)?
                        .build()
                        .await?;
                    return Ok(proxy);
                }
            }
        }
    }

    // Method 2: Try ObjectManager at "/xyz/ljones/aura" (some versions might put it here)
    debug!("Attempting Aura discovery via ObjectManager at /xyz/ljones/aura");
    if let Ok(f) = zbus::fdo::ObjectManagerProxy::new(conn, "xyz.ljones.Asusd", "/xyz/ljones/aura").await {
        if let Ok(interfaces) = f.get_managed_objects().await {
            for (path, iface_map) in interfaces.iter() {
                if iface_map.contains_key("xyz.ljones.Aura") {
                    let aura_path: zbus::zvariant::OwnedObjectPath = path.clone().into();
                    debug!("Found Aura interface via ObjectManager(/xyz/ljones/aura) at: {}", aura_path);
                    let proxy = AuraProxy::builder(conn)
                        .path(aura_path)?
                        .build()
                        .await?;
                    return Ok(proxy);
                }
            }
        }
    }

    // Method 3: Fallback to Introspection on /xyz/ljones/aura
    // This is useful if ObjectManager is not implemented or failing
    debug!("Attempting Aura discovery via Introspection on /xyz/ljones/aura");
    let intro = zbus::fdo::IntrospectableProxy::builder(conn)
        .destination("xyz.ljones.Asusd")?
        .path("/xyz/ljones/aura")?
        .build()
        .await?;
    let xml = intro.introspect().await?;

    for line in xml.lines() {
        let line = line.trim();
        // Look for <node name="DEVICE_ID"/> or <node name="DEVICE_ID">
        if line.starts_with("<node") && line.contains("name=\"") {
            if let Some(start) = line.find("name=\"") {
                let rest = &line[start + 6..];
                if let Some(end) = rest.find('"') {
                    let name = &rest[..end];
                    // Skip standard D-Bus nodes if any and empty names
                    if !name.is_empty() && !name.starts_with('.') {
                        let path_str = format!("/xyz/ljones/aura/{}", name);
                        debug!("Found potential Aura node via introspection: {}", path_str);

                        // Try to connect to this path
                        if let Ok(aura_path) = zbus::zvariant::ObjectPath::try_from(path_str) {
                            // Try to build proxy - if interface exists, this should succeed
                            let proxy = AuraProxy::builder(conn)
                                .path(aura_path)?
                                .build()
                                .await?;

                            // Verify responsiveness by getting brightness
                            if let Ok(_) = proxy.brightness().await {
                                debug!("Confirmed working Aura proxy via introspection");
                                return Ok(proxy);
                            }
                        }
                    }
                }
            }
        }
    }

    Err("No Aura interfaces found via ObjectManager or Introspection".into())
}

pub fn setup_aura_page(ui: &MainWindow) {
    let handle_weak = ui.as_weak();

    tokio::spawn(async move {
        // Set loading state
        let h = handle_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = h.upgrade() {
                ui.global::<AuraPageData>().set_is_loading(true);
                ui.global::<AuraPageData>().set_error_message("".into());
            }
        });

        let conn = match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Failed to connect to D-Bus: {:?}", e);
                error!("{}", err_msg);
                let h = handle_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.global::<AuraPageData>().set_is_loading(false);
                        ui.global::<AuraPageData>().set_error_message(err_msg.into());
                    }
                });
                return;
            }
        };

        let aura = match find_aura_iface(&conn).await {
            Ok(proxy) => proxy,
            Err(e) => {
                let err_msg = format!("Aura interface not available: {}", e);
                warn!("{}", err_msg);
                let h = handle_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = h.upgrade() {
                        ui.global::<AuraPageData>().set_aura_available(false);
                        ui.global::<AuraPageData>().set_is_loading(false);
                        ui.global::<AuraPageData>().set_error_message("No Aura keyboard found".into());
                    }
                });
                return;
            }
        };

        // Get supported modes
        let supported_modes = aura.supported_basic_modes().await.unwrap_or_default();
        debug!("Supported modes: {:?}", supported_modes);

        // Detect Device Type
        if let Ok(device_type) = aura.device_type().await {
            match device_type {
                AuraDeviceType::LaptopKeyboardPre2021 => {
                    debug!("Device Type: Laptop Keyboard (Pre-2021)");
                }
                AuraDeviceType::LaptopKeyboard2021 => {
                    debug!("Device Type: Modern Laptop Keyboard (2021+)");
                }
                AuraDeviceType::LaptopKeyboardTuf => {
                    debug!("Device Type: TUF Laptop Keyboard");
                }
                _ => {
                    warn!("Device Type: Unknown or other ({:?})", device_type);
                }
            }
        } else {
            warn!("Failed to query Aura device type");
        }

        // Prepare mode enabled flags (indices 0..=12)
        let mut mode_flags = Vec::new();
        // Modes 0-8, 10-12 (skip 9 usually)
        // We'll just map 0..=12 to check all slots
        for i in 0..=12 {
            let mode_enum: AuraModeNum = (i as i32).into();
            mode_flags.push(supported_modes.contains(&mode_enum));
        }

        // 1. Load current brightness
        let brightness_val = if let Ok(brightness) = aura.brightness().await {
            brightness as i32
        } else {
            2 // Default Med
        };

        // 2. Load current LED mode and data
        let mode_data_res = aura.led_mode_data().await;

        let h = handle_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            let mode_rc = ModelRc::from(Rc::new(VecModel::from(mode_flags)));

            if let Some(ui) = h.upgrade() {
                let data = ui.global::<AuraPageData>();

                data.set_aura_available(true);
                data.set_mode_enabled(mode_rc);
                data.set_brightness(brightness_val);

                if let Ok(mode_data) = mode_data_res {
                    data.set_led_mode(i32::from(mode_data.mode));
                    data.set_led_speed(mode_data.speed as i32);
                    data.set_color_r(mode_data.colour1.r as i32);
                    data.set_color_g(mode_data.colour1.g as i32);
                    data.set_color_b(mode_data.colour1.b as i32);
                }

                data.set_is_loading(false);
            }
        });

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

                // HSV Color callback - converts HSV to RGB and sets color
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_hsv_color(move |hue, sat, val| {
                    let p = proxy.clone();
                    let h_inner = h.clone();
                    tokio::spawn(async move {
                        // HSV to RGB conversion
                        let (r, g, b) = hsv_to_rgb(hue, sat, val);

                        // Update UI with converted RGB values
                        let h_ui = h_inner.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = h_ui.upgrade() {
                                ui.global::<AuraPageData>().set_color_r(r as i32);
                                ui.global::<AuraPageData>().set_color_g(g as i32);
                                ui.global::<AuraPageData>().set_color_b(b as i32);
                            }
                        });

                        if let Ok(mut mode_data) = p.led_mode_data().await {
                            mode_data.colour1 = rog_aura::Colour { r, g, b };
                            let _ = p.set_led_mode_data(mode_data).await;
                        }
                    });
                });

                // Save preset callback
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_save_color_preset(move |index| {
                    let h_inner = h.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h_inner.upgrade() {
                            let data = ui.global::<AuraPageData>();
                            let r = data.get_color_r();
                            let g = data.get_color_g();
                            let b = data.get_color_b();

                            // Update the preset at index
                            let color = slint::Color::from_rgb_u8(r as u8, g as u8, b as u8);
                            let mut presets: Vec<slint::Color> = data.get_saved_presets().iter().collect();
                            let mut filled: Vec<bool> = data.get_preset_filled().iter().collect();

                            if (index as usize) < presets.len() {
                                presets[index as usize] = color;
                                filled[index as usize] = true;

                                data.set_saved_presets(ModelRc::from(Rc::new(VecModel::from(presets))));
                                data.set_preset_filled(ModelRc::from(Rc::new(VecModel::from(filled))));
                                data.set_selected_preset(index);
                            }
                        }
                    });
                });

                // Load preset callback
                let proxy = aura_copy.clone();
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_load_color_preset(move |index| {
                    let p = proxy.clone();
                    let h_inner = h.clone();

                    // First, get the color from UI in the event loop
                    if let Some(ui) = h_inner.upgrade() {
                        let data = ui.global::<AuraPageData>();
                        let presets: Vec<slint::Color> = data.get_saved_presets().iter().collect();
                        let filled: Vec<bool> = data.get_preset_filled().iter().collect();

                        if (index as usize) < presets.len() && filled[index as usize] {
                            let color = presets[index as usize];
                            let r = color.red();
                            let g = color.green();
                            let b = color.blue();

                            data.set_color_r(r as i32);
                            data.set_color_g(g as i32);
                            data.set_color_b(b as i32);
                            data.set_selected_preset(index);

                            // Now apply to hardware
                            tokio::spawn(async move {
                                if let Ok(mut mode_data) = p.led_mode_data().await {
                                    mode_data.colour1 = rog_aura::Colour { r, g, b };
                                    let _ = p.set_led_mode_data(mode_data).await;
                                }
                            });
                        }
                    }
                });

                // Delete preset callback
                let h = handle_for_callbacks.clone();
                ui.global::<AuraPageData>().on_cb_delete_color_preset(move |index| {
                    let h_inner = h.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h_inner.upgrade() {
                            let data = ui.global::<AuraPageData>();
                            let mut presets: Vec<slint::Color> = data.get_saved_presets().iter().collect();
                            let mut filled: Vec<bool> = data.get_preset_filled().iter().collect();

                            if (index as usize) < presets.len() {
                                presets[index as usize] = slint::Color::from_argb_u8(0, 0, 0, 0);
                                filled[index as usize] = false;

                                data.set_saved_presets(ModelRc::from(Rc::new(VecModel::from(presets))));
                                data.set_preset_filled(ModelRc::from(Rc::new(VecModel::from(filled))));

                                if data.get_selected_preset() == index {
                                    data.set_selected_preset(-1);
                                }
                            }
                        }
                    });
                });

                // Mode name callback
                ui.global::<AuraPageData>().on_get_mode_name(|mode_val| {
                    let mode_name = match mode_val {
                        0 => "Static",
                        1 => "Breathing",
                        2 => "Color Cycle",
                        3 => "Rainbow",
                        4 => "Starry Night",
                        5 => "Rain",
                        6 => "Reactive",
                        7 => "Laser",
                        8 => "Ripple",
                        10 => "Pulse",
                        11 => "Comet",
                        12 => "Strobing",
                        _ => "Unknown",
                    };
                    mode_name.into()
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
