//! Fan curves page setup - Custom fan curve controls
//!
//! Connects the Fans page UI to D-Bus FanCurves interface.

use log::{debug, error, info, warn};
use rog_platform::platform::PlatformProfile;
use rog_profiles::FanCurvePU;
use slint::{ComponentHandle, Model};

use crate::{FansPageData, MainWindow, Node};

/// Convert D-Bus curve data to Slint Node array
fn curve_to_nodes(pwm: &[u8], temp: &[u8]) -> Vec<Node> {
    pwm.iter()
        .zip(temp.iter())
        .map(|(p, t)| Node {
            x: *t as f32,  // Temperature in degrees
            y: *p as f32,  // PWM value 0-255
        })
        .collect()
}

/// Convert Slint Node array back to D-Bus format
fn nodes_to_curve(nodes: &[Node]) -> ([u8; 8], [u8; 8]) {
    let mut pwm = [0u8; 8];
    let mut temp = [0u8; 8];

    for (i, node) in nodes.iter().take(8).enumerate() {
        temp[i] = node.x as u8;
        pwm[i] = node.y as u8;
    }

    (pwm, temp)
}

/// Load fan curves for a specific profile
fn load_profile_curves(
    fans: &rog_dbus::zbus_fan_curves::FanCurvesProxyBlocking,
    profile: PlatformProfile,
) -> Option<(Vec<Node>, bool, Vec<Node>, bool)> {
    match fans.fan_curve_data(profile) {
        Ok(curves) => {
            let cpu_curve = curves.first().map(|c| {
                (curve_to_nodes(&c.pwm, &c.temp), c.enabled)
            });
            let gpu_curve = curves.get(1).map(|c| {
                (curve_to_nodes(&c.pwm, &c.temp), c.enabled)
            });

            match (cpu_curve, gpu_curve) {
                (Some((cpu_nodes, cpu_enabled)), Some((gpu_nodes, gpu_enabled))) => {
                    Some((cpu_nodes, cpu_enabled, gpu_nodes, gpu_enabled))
                }
                _ => None
            }
        }
        Err(e) => {
            warn!("Failed to load curves for {:?}: {:?}", profile, e);
            None
        }
    }
}

/// Initial setup for fans page - fetches current fan curves from D-Bus
pub fn setup_fans_page(ui: &MainWindow) {
    // Connect to system bus (blocking for initial setup)
    let conn = match zbus::blocking::Connection::system() {
        Ok(c) => c,
        Err(e) => {
            error!("D-Bus system connection failed: {:?}", e);
            return;
        }
    };

    // Get fan curves proxy
    let fans = match rog_dbus::zbus_fan_curves::FanCurvesProxyBlocking::new(&conn) {
        Ok(f) => f,
        Err(e) => {
            warn!("FanCurvesProxy failed: {:?}", e);
            ui.global::<FansPageData>().set_fans_available(false);
            return;
        }
    };

    ui.global::<FansPageData>().set_fans_available(true);
    let fans_data = ui.global::<FansPageData>();

    // Load Balanced profile
    if let Some((cpu, cpu_en, gpu, gpu_en)) = load_profile_curves(&fans, PlatformProfile::Balanced) {
        fans_data.set_cpu_fan_curve_balanced(slint::ModelRc::new(slint::VecModel::from(cpu)));
        fans_data.set_gpu_fan_curve_balanced(slint::ModelRc::new(slint::VecModel::from(gpu)));
        fans_data.set_cpu_fan_enabled(cpu_en);
        fans_data.set_gpu_fan_enabled(gpu_en);
        debug!("Balanced profile loaded");
    }

    // Load Performance profile
    if let Some((cpu, _, gpu, _)) = load_profile_curves(&fans, PlatformProfile::Performance) {
        fans_data.set_cpu_fan_curve_performance(slint::ModelRc::new(slint::VecModel::from(cpu)));
        fans_data.set_gpu_fan_curve_performance(slint::ModelRc::new(slint::VecModel::from(gpu)));
        debug!("Performance profile loaded");
    }

    // Load Quiet profile
    if let Some((cpu, _, gpu, _)) = load_profile_curves(&fans, PlatformProfile::Quiet) {
        fans_data.set_cpu_fan_curve_quiet(slint::ModelRc::new(slint::VecModel::from(cpu)));
        fans_data.set_gpu_fan_curve_quiet(slint::ModelRc::new(slint::VecModel::from(gpu)));
        debug!("Quiet profile loaded");
    }

    info!("Fans page initialized with all profiles");
}

/// Setup async callbacks for fans page
pub fn setup_fans_page_callbacks(ui: &MainWindow) {
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

        // Get fan curves proxy
        let fans = match rog_dbus::zbus_fan_curves::FanCurvesProxy::new(&conn).await {
            Ok(f) => f,
            Err(e) => {
                warn!("FanCurvesProxy failed: {:?}", e);
                return;
            }
        };

        let fans_copy = fans.clone();
        let handle_copy = handle.clone();

        let _ = handle.upgrade_in_event_loop(move |ui| {
            // Apply fan curves callback
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>().on_cb_apply_curves(move || {
                let proxy = fans_inner.clone();
                let h = handle_inner.clone();

                // Extract all UI data synchronously before spawning async task
                let ui_data = h.upgrade().map(|ui| {
                    let fans_data = ui.global::<FansPageData>();
                    let profile_idx = fans_data.get_selected_profile();
                    let selected_fan = fans_data.get_selected_fan();

                    // Get the appropriate curve based on profile and fan
                    let curve_model = if selected_fan == 0 {
                        match profile_idx {
                            0 => fans_data.get_cpu_fan_curve_balanced(),
                            1 => fans_data.get_cpu_fan_curve_performance(),
                            _ => fans_data.get_cpu_fan_curve_quiet(),
                        }
                    } else {
                        match profile_idx {
                            0 => fans_data.get_gpu_fan_curve_balanced(),
                            1 => fans_data.get_gpu_fan_curve_performance(),
                            _ => fans_data.get_gpu_fan_curve_quiet(),
                        }
                    };

                    // Convert to Vec<Node>
                    let nodes: Vec<Node> = (0..curve_model.row_count())
                        .filter_map(|i| curve_model.row_data(i))
                        .collect();

                    let (pwm, temp) = nodes_to_curve(&nodes);

                    let fan_type = if selected_fan == 0 { FanCurvePU::CPU } else { FanCurvePU::GPU };
                    let enabled = if selected_fan == 0 {
                        fans_data.get_cpu_fan_enabled()
                    } else {
                        fans_data.get_gpu_fan_enabled()
                    };

                    let profile = match profile_idx {
                        0 => PlatformProfile::Balanced,
                        1 => PlatformProfile::Performance,
                        2 => PlatformProfile::Quiet,
                        _ => PlatformProfile::Balanced,
                    };

                    (profile, rog_profiles::fan_curve_set::CurveData {
                        fan: fan_type,
                        pwm,
                        temp,
                        enabled,
                    })
                });

                if let Some((profile, curve_data)) = ui_data {
                    tokio::spawn(async move {
                        match proxy.set_fan_curve(profile, curve_data).await {
                            Ok(_) => {
                                let msg: slint::SharedString = "Fan curve applied successfully".into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Failed to apply fan curve: {:?}", e);
                                let msg: slint::SharedString = "Failed to apply fan curve".into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                        }
                    });
                }
            });

            // Reset to defaults callback
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>().on_cb_reset_defaults(move || {
                let proxy = fans_inner.clone();
                let h = handle_inner.clone();

                // Extract profile synchronously
                let profile_opt = h.upgrade().map(|ui| {
                    let profile_idx = ui.global::<FansPageData>().get_selected_profile();
                    match profile_idx {
                        0 => PlatformProfile::Balanced,
                        1 => PlatformProfile::Performance,
                        2 => PlatformProfile::Quiet,
                        _ => PlatformProfile::Balanced,
                    }
                });

                if let Some(profile) = profile_opt {
                    tokio::spawn(async move {
                        match proxy.set_curves_to_defaults(profile).await {
                            Ok(_) => {
                                let msg: slint::SharedString = "Fan curves reset to defaults".into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Failed to reset fan curves: {:?}", e);
                                let msg: slint::SharedString = "Failed to reset fan curves".into();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = h.upgrade() {
                                        ui.invoke_show_toast(msg);
                                    }
                                });
                            }
                        }
                    });
                }
            });

            // Profile changed callback - reload curves for new profile
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>().on_cb_profile_changed(move |profile_idx| {
                let proxy = fans_inner.clone();
                let h = handle_inner.clone();

                let profile = match profile_idx {
                    0 => PlatformProfile::Balanced,
                    1 => PlatformProfile::Performance,
                    2 => PlatformProfile::Quiet,
                    _ => PlatformProfile::Balanced,
                };

                tokio::spawn(async move {
                    // Reload curves from D-Bus for this profile
                    if let Ok(curves) = proxy.fan_curve_data(profile).await {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = h.upgrade() {
                                let fans_data = ui.global::<FansPageData>();

                                // Update CPU curve
                                if let Some(cpu_curve) = curves.first() {
                                    let nodes = curve_to_nodes(&cpu_curve.pwm, &cpu_curve.temp);
                                    match profile_idx {
                                        0 => fans_data.set_cpu_fan_curve_balanced(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                        1 => fans_data.set_cpu_fan_curve_performance(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                        _ => fans_data.set_cpu_fan_curve_quiet(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                    }
                                    fans_data.set_cpu_fan_enabled(cpu_curve.enabled);
                                }

                                // Update GPU curve
                                if let Some(gpu_curve) = curves.get(1) {
                                    let nodes = curve_to_nodes(&gpu_curve.pwm, &gpu_curve.temp);
                                    match profile_idx {
                                        0 => fans_data.set_gpu_fan_curve_balanced(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                        1 => fans_data.set_gpu_fan_curve_performance(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                        _ => fans_data.set_gpu_fan_curve_quiet(slint::ModelRc::new(slint::VecModel::from(nodes))),
                                    }
                                    fans_data.set_gpu_fan_enabled(gpu_curve.enabled);
                                }
                            }
                        });
                    }
                });
            });

            // Enable/disable CPU fan callback
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>()
                .on_cb_set_cpu_fan_enabled(move |enabled| {
                    let proxy = fans_inner.clone();
                    let h = handle_inner.clone();

                    // Extract profile synchronously
                    let profile_opt = h.upgrade().map(|ui| {
                        let profile_idx = ui.global::<FansPageData>().get_selected_profile();
                        match profile_idx {
                            0 => PlatformProfile::Balanced,
                            1 => PlatformProfile::Performance,
                            2 => PlatformProfile::Quiet,
                            _ => PlatformProfile::Balanced,
                        }
                    });

                    if let Some(profile) = profile_opt {
                        tokio::spawn(async move {
                            let _ = proxy
                                .set_profile_fan_curve_enabled(profile, FanCurvePU::CPU, enabled)
                                .await;
                        });
                    }
                });

            // Enable/disable GPU fan callback
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>()
                .on_cb_set_gpu_fan_enabled(move |enabled| {
                    let proxy = fans_inner.clone();
                    let h = handle_inner.clone();

                    // Extract profile synchronously
                    let profile_opt = h.upgrade().map(|ui| {
                        let profile_idx = ui.global::<FansPageData>().get_selected_profile();
                        match profile_idx {
                            0 => PlatformProfile::Balanced,
                            1 => PlatformProfile::Performance,
                            2 => PlatformProfile::Quiet,
                            _ => PlatformProfile::Balanced,
                        }
                    });

                    if let Some(profile) = profile_opt {
                        tokio::spawn(async move {
                            let _ = proxy
                                .set_profile_fan_curve_enabled(profile, FanCurvePU::GPU, enabled)
                                .await;
                        });
                    }
                });
        });
    });
}
