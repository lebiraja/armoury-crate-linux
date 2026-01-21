//! Fan curves page setup - Custom fan curve controls
//!
//! Connects the Fans page UI to D-Bus FanCurves interface.

use log::{debug, error, info, warn};
use slint::ComponentHandle;

use crate::{FansPageData, MainWindow, FanCurvePoint};

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

    // Load current fan curve data for Balanced profile
    if let Ok(curves) = fans.fan_curve_data(rog_platform::platform::PlatformProfile::Balanced) {
        debug!("Fan curves loaded: {:?}", curves);

        // Convert to Slint model - CPU fan (first curve)
        if let Some(cpu_curve) = curves.first() {
            let points: Vec<FanCurvePoint> = cpu_curve
                .pwm
                .iter()
                .zip(cpu_curve.temp.iter())
                .map(|(pwm, temp)| FanCurvePoint {
                    temp: *temp as i32,
                    pwm: *pwm as i32,
                })
                .collect();

            fans_data.set_cpu_fan_curve(slint::ModelRc::new(slint::VecModel::from(points)));
            fans_data.set_cpu_fan_enabled(cpu_curve.enabled);
        }

        // GPU fan (second curve)
        if let Some(gpu_curve) = curves.get(1) {
            let points: Vec<FanCurvePoint> = gpu_curve
                .pwm
                .iter()
                .zip(gpu_curve.temp.iter())
                .map(|(pwm, temp)| FanCurvePoint {
                    temp: *temp as i32,
                    pwm: *pwm as i32,
                })
                .collect();

            fans_data.set_gpu_fan_curve(slint::ModelRc::new(slint::VecModel::from(points)));
            fans_data.set_gpu_fan_enabled(gpu_curve.enabled);
        }
    }

    info!("Fans page initialized");
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

                tokio::spawn(async move {
                    // For now, just show a toast that curves would be applied
                    let msg: slint::SharedString = "Fan curves apply - not yet implemented".into();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = h.upgrade() {
                            ui.invoke_show_toast(msg);
                        }
                    });
                    // TODO: Get curves from UI and call proxy.set_fan_curve()
                    let _ = proxy; // suppress unused warning
                });
            });

            // Reset to defaults callback
            let fans_inner = fans_copy.clone();
            let handle_inner = handle_copy.clone();
            ui.global::<FansPageData>().on_cb_reset_defaults(move || {
                let proxy = fans_inner.clone();
                let h = handle_inner.clone();

                tokio::spawn(async move {
                    // Get current profile selection
                    let profile = rog_platform::platform::PlatformProfile::Balanced;

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
            });

            // Enable/disable CPU fan callback
            let fans_inner = fans_copy.clone();
            ui.global::<FansPageData>()
                .on_cb_set_cpu_fan_enabled(move |enabled| {
                    let proxy = fans_inner.clone();
                    tokio::spawn(async move {
                        let profile = rog_platform::platform::PlatformProfile::Balanced;
                        let _ = proxy
                            .set_profile_fan_curve_enabled(
                                profile,
                                rog_profiles::FanCurvePU::CPU,
                                enabled,
                            )
                            .await;
                    });
                });

            // Enable/disable GPU fan callback
            let fans_inner = fans_copy.clone();
            ui.global::<FansPageData>()
                .on_cb_set_gpu_fan_enabled(move |enabled| {
                    let proxy = fans_inner.clone();
                    tokio::spawn(async move {
                        let profile = rog_platform::platform::PlatformProfile::Balanced;
                        let _ = proxy
                            .set_profile_fan_curve_enabled(
                                profile,
                                rog_profiles::FanCurvePU::GPU,
                                enabled,
                            )
                            .await;
                    });
                });
        });
    });
}
