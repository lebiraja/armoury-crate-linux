//! `update_and_notify` is responsible for both notifications *and* updating
//! stored statuses about the system state. This is done through either direct,
//! intoify, zbus notifications or similar methods.
//!
//! This module very much functions like a stand-alone app on its own thread.

use std::fmt::Display;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use log::{debug, error, info, warn};
use notify_rust::{Hint, Notification, Timeout};
use rog_dbus::asus_armoury::AsusArmouryProxy;
use rog_platform::asus_armoury::FirmwareAttribute;
use rog_platform::platform::GpuMode;
use rog_platform::power::AsusPower;
use serde::{Deserialize, Serialize};
use supergfxctl::pci_device::GfxPower;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::error::Result;
use crate::zbus_proxies::find_iface_async;

const NOTIF_HEADER: &str = "ROG Control";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EnabledNotifications {
    pub enabled: bool,
    pub receive_notify_gfx: bool,
    pub receive_notify_gfx_status: bool,
}

impl Default for EnabledNotifications {
    fn default() -> Self {
        Self {
            enabled: true,
            receive_notify_gfx: true,
            receive_notify_gfx_status: true,
        }
    }
}

fn start_dpu_status_mon(config: Arc<Mutex<Config>>) {
    use supergfxctl::pci_device::Device;
    let dev = Device::find().unwrap_or_default();
    let mut found_dgpu = false; // just for logging
    for dev in dev {
        if dev.is_dgpu() {
            info!(
                "Found dGPU: {}, starting status notifications",
                dev.pci_id()
            );
            let enabled_notifications_copy = config.clone();
            // Plain old thread is perfectly fine since most of this is potentially blocking
            std::thread::spawn(move || {
                let mut last_status = GfxPower::Unknown;
                loop {
                    std::thread::sleep(Duration::from_millis(1500));
                    if let Ok(status) = dev.get_runtime_status() {
                        if status != GfxPower::Unknown && status != last_status {
                            if let Ok(config) = enabled_notifications_copy.lock() {
                                if !config.notifications.receive_notify_gfx_status
                                    || !config.notifications.enabled
                                {
                                    continue;
                                }
                            }
                            // Required check because status cycles through
                            // active/unknown/suspended
                            do_gpu_status_notif("dGPU status changed:", &status)
                                .show()
                                .unwrap()
                                .on_close(|_| ());
                            debug!("dGPU status changed: {:?}", &status);
                        }
                        last_status = status;
                    }
                }
            });
            found_dgpu = true;
            break;
        }
    }
    if !found_dgpu {
        warn!("Did not find a dGPU on this system, dGPU status won't be avilable");
    }
}

pub fn start_notifications(
    config: Arc<Mutex<Config>>,
    rt: &Runtime,
) -> Result<Vec<JoinHandle<()>>> {
    // Setup the AC/BAT commands that will run on power status change
    let config_copy = config.clone();
    let blocking = rt.spawn_blocking(move || {
        let power = AsusPower::new()
            .map_err(|e| {
                error!("AsusPower: {e}");
                e
            })
            .unwrap();

        let mut last_state = power.get_online().unwrap_or_default();
        loop {
            if let Ok(p) = power.get_online() {
                let mut ac = String::new();
                let mut bat = String::new();
                if let Ok(config) = config_copy.lock() {
                    ac.clone_from(&config.ac_command);
                    bat.clone_from(&config.bat_command);
                }

                if p == 0 && p != last_state {
                    let prog: Vec<&str> = bat.split_whitespace().collect();
                    if (!prog.is_empty()) && (!prog[0].is_empty()) {
                        let mut cmd = Command::new(prog[0]);

                        for arg in prog.iter().skip(1) {
                            cmd.arg(*arg);
                        }
                        cmd.spawn()
                            .map_err(|e| error!("Battery power command error: {e:?}"))
                            .ok();
                    }
                } else if p != last_state {
                    let prog: Vec<&str> = ac.split_whitespace().collect();
                    if (!prog.is_empty()) && (!prog[0].is_empty()) {
                        let mut cmd = Command::new(prog[0]);

                        for arg in prog.iter().skip(1) {
                            cmd.arg(*arg);
                        }
                        cmd.spawn()
                            .map_err(|e| error!("AC power command error: {e:?}"))
                            .ok();
                    }
                }
                last_state = p;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });

    info!("Attempting to start plain dgpu status monitor");
    start_dpu_status_mon(config.clone());

    // GPU MUX Mode notif
    let enabled_notifications_copy = config.clone();
    rt.spawn(async move {
        let attrs = match find_iface_async::<AsusArmouryProxy>("xyz.ljones.AsusArmoury").await {
            Ok(attrs) => attrs,
            Err(_) => return,
        };
        
        for attr in attrs {
            let name = match attr.name().await {
                Ok(name) => name,
                Err(_) => continue,
            };
            
            if name == FirmwareAttribute::GpuMuxMode {
                let mut last_mode = attr.current_value().await.unwrap_or(-1);
                let mut stream = attr.receive_current_value_changed().await;
                info!("Started monitoring GPU MUX mode changes");
                while let Some(signal) = stream.next().await {
                    if let Ok(new_val) = signal.get().await {
                        if new_val != last_mode {
                            let config_copy = enabled_notifications_copy.clone();
                            let should_notify = tokio::task::spawn_blocking(move || {
                                config_copy.lock()
                                    .map(|config| config.notifications.enabled && config.notifications.receive_notify_gfx)
                                    .unwrap_or(false)
                            }).await.unwrap_or(false);
                            
                            if !should_notify {
                                last_mode = new_val;
                                continue;
                            }
                            let mode = GpuMode::from(new_val as u8);
                            do_mux_notification("Reboot required. BIOS GPU MUX mode set to", &mode)
                                .show()
                                .unwrap()
                                .on_close(|_| ());
                            last_mode = new_val;
                        }
                    }
                }
                break;
            }
        }
    });

    Ok(vec![blocking])
}

fn base_notification<T>(message: &str, data: &T) -> Notification
where
    T: Display,
{
    let mut notif = Notification::new();
    notif
        .appname(NOTIF_HEADER)
        .summary(&format!("{message} {data}"))
        .timeout(Timeout::Milliseconds(3000))
        .hint(Hint::Category("device".into()));
    notif
}

fn do_gpu_status_notif(message: &str, data: &GfxPower) -> Notification {
    let mut notif = base_notification(message, &<&str>::from(data).to_owned());
    let icon = match data {
        GfxPower::Suspended => "asus_notif_blue",
        GfxPower::Off => "asus_notif_green",
        GfxPower::AsusDisabled => "asus_notif_white",
        GfxPower::AsusMuxDiscreet | GfxPower::Active => "asus_notif_red",
        GfxPower::Unknown => "gpu-integrated",
    };
    notif.icon(icon);
    notif
}

fn do_mux_notification(message: &str, mode: &GpuMode) -> Notification {
    let mut notif = base_notification(message, mode);
    let icon = match mode {
        GpuMode::Ultimate => "asus_notif_red",
        _ => "asus_notif_green",
    };
    notif.icon(icon);
    notif
}
