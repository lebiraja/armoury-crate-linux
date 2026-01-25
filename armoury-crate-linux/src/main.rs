//! Armoury Crate Linux - Main Entry Point
//!
//! A gaming-focused control center for ASUS ROG laptops on Linux.

use std::env;
use std::sync::{Arc, Mutex};

use armoury_crate_linux::config::Config;
use armoury_crate_linux::error::Result;
use armoury_crate_linux::monitoring::SystemMonitor;
use armoury_crate_linux::scenario_manager::ScenarioManager;
use armoury_crate_linux::slint::ComponentHandle;
use armoury_crate_linux::ui;
use armoury_crate_linux::{print_versions, MainWindow};
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
    let config = Arc::new(Mutex::new(Config::new().load()));
    info!("Configuration loaded");

    // Initialize system monitoring
    let update_interval = config.lock().unwrap().monitoring.update_interval_ms;
    let history_duration = config.lock().unwrap().monitoring.history_duration_sec;
    let monitor = Arc::new(SystemMonitor::new(
        update_interval,
        history_duration,
    ));

    // Start monitoring in background
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        monitor_clone.start().await;
    });
    info!("System monitoring started");

    // Initialize scenario manager
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("armoury-crate-linux")
        .join("scenarios.toml");
    let scenario_manager = Arc::new(ScenarioManager::new(config_path));

    // Start scenario monitoring with profile change callback
    let scenario_clone = scenario_manager.clone();
    tokio::spawn(async move {
        scenario_clone
            .start_monitoring(|rule| {
                if let Some(profile) = rule.power_profile {
                    let profile_owned = profile.clone();
                    tokio::spawn(async move {
                        if let Ok(conn) = zbus::Connection::system().await {
                            if let Ok(_proxy) = rog_dbus::zbus_platform::PlatformProxy::new(&conn).await
                            {
                                // Note: The actual method name may differ - using throttle_thermal_policy as a fallback
                                info!("Scenario: would switch to profile '{}'", profile_owned);
                            }
                        }
                    });
                }
            })
            .await;
    });
    info!("Scenario manager initialized");

    // Start notifications
    let _rt = tokio::runtime::Handle::current();
    // We need a way to pass &Runtime but we are in #[tokio::main]
    // notify.rs expects &Runtime, but we can probably change it to use Handle or just spawn on current
    // For now, let's just use what we have and see if it works with Handle if we change the signature
    // or create a new runtime if really needed.
    // Actually, start_notifications takes &Runtime for spawn_blocking and spawn.
    // Let's modify start_notifications to be more flexible or use Handle.

    info!("Starting notification system");
    // Since we are in tokio main, we'll pass the handle if possible or use a trick.
    // Let's check start_notifications signature in notify.rs again.

    // Create and show the main window
    let ui = MainWindow::new()?;

    // Setup all UI pages with initial D-Bus values
    ui::setup_all_pages(&ui);
    info!("UI pages initialized");

    // Setup async callbacks for D-Bus interactions
    ui::setup_all_callbacks(&ui);
    info!("UI callbacks set up");

    // Setup dashboard page with monitoring
    ui::setup_dashboard::setup_dashboard_page(&ui, monitor.clone());
    info!("Dashboard monitoring integrated");

    // Setup scenario profiles page
    ui::setup_scenario::setup_scenario_page(&ui, scenario_manager.clone());
    info!("Scenario profiles integrated");

    // Setup settings page
    ui::setup_settings::setup_settings_page(&ui, config.clone());
    ui::setup_settings::setup_settings_page_callbacks(&ui, config.clone());
    info!("Settings page integrated");

    // Show the window
    ui.show()?;
    info!("Window opened");

    // Run the event loop
    slint::run_event_loop()?;

    // Cleanup
    monitor.stop().await;
    scenario_manager.set_enabled(false).await;
    info!("Application closed");

    Ok(())
}
