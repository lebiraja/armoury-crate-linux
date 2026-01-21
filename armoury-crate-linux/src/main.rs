//! Armoury Crate Linux - Main Entry Point
//!
//! A gaming-focused control center for ASUS ROG laptops on Linux.

use std::env;
use std::sync::Arc;

use armoury_crate_linux::config::Config;
use armoury_crate_linux::error::Result;
use armoury_crate_linux::monitoring::SystemMonitor;
use armoury_crate_linux::slint::ComponentHandle;
use armoury_crate_linux::ui;
use armoury_crate_linux::{print_versions, DashboardData, MainWindow};
use config_traits::{StdConfig, StdConfigLoad1};
use dmi_id::DMIID;
use gumdrop::Options;
use log::{debug, info, warn, LevelFilter};

/// CLI options for Armoury Crate Linux
#[derive(Debug, Default, Options)]
pub struct CliStart {
    #[options(help = "print help message")]
    pub help: bool,
    #[options(help = "print version")]
    pub version: bool,
    #[options(help = "start in fullscreen mode")]
    pub fullscreen: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::Builder::new()
        .parse_default_env()
        .filter_level(LevelFilter::Info)
        .target(env_logger::Target::Stderr)
        .format_timestamp(None)
        .init();

    // Handle gamescope environment for ROG Ally
    if let Ok(gamescope) = env::var("GAMESCOPE_WAYLAND_DISPLAY") {
        debug!("Gamescope detected");
        if !gamescope.is_empty() {
            debug!("Setting WAYLAND_DISPLAY to {}", gamescope);
            env::set_var("WAYLAND_DISPLAY", gamescope);
        }
    }

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli_parsed = CliStart::parse_args_default(&args).unwrap_or_default();

    if cli_parsed.help {
        println!("{}", CliStart::usage());
        return Ok(());
    }

    if cli_parsed.version {
        print_versions();
        return Ok(());
    }

    // Get device info
    let dmi = DMIID::new().unwrap_or_default();
    info!("Running on {}, product: {}", dmi.board_name, dmi.product_family);

    // Version check with asusd
    let self_version = env!("CARGO_PKG_VERSION");
    if let Ok(zbus_con) = zbus::blocking::Connection::system() {
        if let Ok(platform_proxy) = rog_dbus::zbus_platform::PlatformProxyBlocking::new(&zbus_con) {
            match platform_proxy.version() {
                Ok(asusd_version) => {
                    if asusd_version != self_version {
                        warn!(
                            "Version mismatch: armoury-crate-linux = {}, asusd = {}",
                            self_version, asusd_version
                        );
                    } else {
                        info!("Connected to asusd v{}", asusd_version);
                    }
                }
                Err(e) => {
                    warn!("Could not get asusd version: {:?}", e);
                    warn!("Is asusd.service running? Some features may not work.");
                }
            }
        }
    }

    // Load configuration
    let config = Config::new().load();
    info!("Configuration loaded");

    // Initialize system monitoring
    let monitor = Arc::new(SystemMonitor::new(
        config.monitoring.update_interval_ms,
        config.monitoring.history_duration_sec,
    ));

    // Start monitoring in background
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        monitor_clone.start().await;
    });
    info!("System monitoring started");

    // Create and show the main window
    let ui = MainWindow::new()?;

    // Setup all UI pages with initial D-Bus values
    ui::setup_all_pages(&ui);
    info!("UI pages initialized");

    // Setup async callbacks for D-Bus interactions
    ui::setup_all_callbacks(&ui);
    info!("UI callbacks set up");

    // Set up monitoring data updates
    let ui_weak = ui.as_weak();
    let monitor_for_updates = monitor.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;

            let data = monitor_for_updates.get_data().await;
            let ui_weak_clone = ui_weak.clone();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_clone.upgrade() {
                    let dashboard = ui.global::<DashboardData>();
                    dashboard.set_cpu_usage(data.cpu_usage);
                    dashboard.set_cpu_temp(data.cpu_temp);
                    dashboard.set_cpu_freq_ghz(data.cpu_freq_mhz as f32 / 1000.0);
                    dashboard.set_gpu_usage(data.gpu_usage);
                    dashboard.set_gpu_temp(data.gpu_temp);
                    dashboard.set_gpu_power_watts(data.gpu_power_watts);
                    dashboard.set_ram_usage_percent(data.ram_usage);
                    dashboard.set_ram_total_gb(data.ram_total_gb);
                    dashboard.set_ram_used_gb(data.ram_used_gb);
                    dashboard.set_fan1_rpm(data.fan_rpm.first().copied().unwrap_or(0) as i32);
                    dashboard.set_fan2_rpm(data.fan_rpm.get(1).copied().unwrap_or(0) as i32);
                    dashboard.set_battery_percent(data.battery_percent.unwrap_or(100.0));
                    dashboard.set_on_ac_power(data.on_ac_power);
                }
            });
        }
    });

    // Show the window
    ui.show()?;
    info!("Window opened");

    // Run the event loop
    slint::run_event_loop()?;

    // Cleanup
    monitor.stop().await;
    info!("Application closed");

    Ok(())
}
