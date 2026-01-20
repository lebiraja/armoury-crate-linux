//! Armoury Crate Linux - Main Entry Point
//!
//! A gaming-focused control center for ASUS ROG laptops on Linux.

use std::env::{self, args};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use armoury_crate_linux::config::Config;
use armoury_crate_linux::error::Result;
use armoury_crate_linux::monitoring::SystemMonitor;
use armoury_crate_linux::slint::ComponentHandle;
use armoury_crate_linux::{print_versions, MainWindow};
use config_traits::{StdConfig, StdConfigLoad1};
use dmi_id::DMIID;
use gumdrop::Options;
use log::{debug, info, warn, LevelFilter};
use tokio::runtime::Runtime;

/// CLI options for Armoury Crate Linux
#[derive(Debug, Options)]
pub struct CliStart {
    #[options(help = "print help message")]
    pub help: bool,
    #[options(help = "print version")]
    pub version: bool,
    #[options(help = "start in fullscreen mode")]
    pub fullscreen: bool,
    #[options(help = "start in windowed mode (overrides config)")]
    pub windowed: bool,
    #[options(help = "fullscreen width", default = "0")]
    pub width_fullscreen: u32,
    #[options(help = "fullscreen height", default = "0")]
    pub height_fullscreen: u32,
}

/// Application state for managing window visibility
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    StartingUp,
    MainWindowShouldOpen,
    MainWindowOpen,
    MainWindowClosed,
    QuitApp,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "warn,tracing=error,zbus=error");
    }
    let mut logger = env_logger::Builder::new();
    logger
        .parse_default_env()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .target(env_logger::Target::Stderr)
        .format_timestamp(None)
        .init();

    // Handle gamescope environment for ROG Ally
    if let Ok(gamescope) = env::var("GAMESCOPE_WAYLAND_DISPLAY") {
        debug!("Gamescope detected");
        if !gamescope.is_empty() {
            debug!("Setting WAYLAND_DISPLAY to {}", gamescope);
            env::set_var("WAYLAND_DISPLAY", gamescope);
        } else if let Ok(wayland) = env::var("WAYLAND_DISPLAY") {
            if wayland.is_empty() {
                debug!("Setting WAYLAND_DISPLAY to gamescope-0");
                env::set_var("WAYLAND_DISPLAY", "gamescope-0");
            }
        }
    }

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
                    }
                }
                Err(e) => {
                    warn!("Could not get asusd version: {:?}", e);
                    warn!("Is asusd.service running?");
                }
            }
        }
    }

    // Start tokio runtime
    let rt = Runtime::new().expect("Unable to create Runtime");
    let _enter = rt.enter();

    // Get device info
    let dmi = DMIID::new().unwrap_or_default();
    let board_name = dmi.board_name;
    let prod_family = dmi.product_family;
    info!("Running on {board_name}, product: {prod_family}");

    // Parse CLI arguments
    let args: Vec<String> = args().skip(1).collect();
    let cli_parsed = match CliStart::parse_args_default(&args) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("Error parsing arguments: {}", err);
            exit(1);
        }
    };

    if do_cli_help(&cli_parsed) {
        return Ok(());
    }

    // Load configuration
    let mut config = Config::new().load();
    if cli_parsed.fullscreen {
        config.start_fullscreen = true;
        if cli_parsed.width_fullscreen != 0 {
            config.fullscreen_width = cli_parsed.width_fullscreen;
        }
        if cli_parsed.height_fullscreen != 0 {
            config.fullscreen_height = cli_parsed.height_fullscreen;
        }
    } else if cli_parsed.windowed {
        config.start_fullscreen = false;
    }

    // Detect ROG Ally
    let _is_rog_ally = {
        #[cfg(feature = "rog_ally")]
        {
            board_name == "RC71L" || board_name == "RC72L" || prod_family == "ROG Ally"
        }
        #[cfg(not(feature = "rog_ally"))]
        {
            false
        }
    };

    #[cfg(feature = "rog_ally")]
    if is_rog_ally {
        config.notifications.enabled = false;
        config.enable_tray_icon = false;
        config.run_in_background = false;
        config.startup_in_background = false;
        config.start_fullscreen = true;
    }

    config.write();

    let startup_in_background = config.startup_in_background;
    let config = Arc::new(Mutex::new(config));
    let app_state = Arc::new(Mutex::new(AppState::StartingUp));

    // Initialize system monitoring
    let monitor = Arc::new(SystemMonitor::new(500, 60));
    {
        let monitor = monitor.clone();
        tokio::spawn(async move {
            monitor.start().await;
        });
    }

    // Set initial app state
    if !startup_in_background {
        if let Ok(mut state) = app_state.lock() {
            *state = AppState::MainWindowShouldOpen;
        }
    }

    // Initialize translations
    if std::env::var("RUST_TRANSLATIONS").is_ok() {
        log::debug!("Using local-dir translations");
        slint::init_translations!("/usr/share/locale/");
    } else {
        log::debug!("Using system installed translations");
        slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/translations/"));
    }

    thread_local! { pub static UI: std::cell::RefCell<Option<MainWindow>> = Default::default()}

    // UI management thread
    let app_state_clone = app_state.clone();
    let config_clone = config.clone();
    let monitor_clone = monitor.clone();

    thread::spawn(move || {
        let mut state = AppState::StartingUp;
        loop {
            // Get current state
            if let Ok(current_state) = app_state_clone.lock() {
                state = *current_state;
            }

            sleep(Duration::from_millis(300));

            if state == AppState::MainWindowShouldOpen {
                if let Ok(mut app_state) = app_state_clone.lock() {
                    *app_state = AppState::MainWindowOpen;
                }

                let app_state_copy = app_state_clone.clone();
                let config_copy = config_clone.clone();
                let _monitor_copy = monitor_clone.clone();

                slint::invoke_from_event_loop(move || {
                    UI.with(|ui| {
                        let app_state_copy = app_state_copy.clone();
                        let mut ui = ui.borrow_mut();
                        if let Some(ui) = ui.as_mut() {
                            ui.window().show().unwrap();
                            ui.window().on_close_requested(move || {
                                if let Ok(mut app_state) = app_state_copy.lock() {
                                    *app_state = AppState::MainWindowClosed;
                                }
                                slint::CloseRequestResponse::HideWindow
                            });
                        } else {
                            let newui = setup_window(config_copy.clone());
                            newui.window().on_close_requested(move || {
                                if let Ok(mut app_state) = app_state_copy.lock() {
                                    *app_state = AppState::MainWindowClosed;
                                }
                                slint::CloseRequestResponse::HideWindow
                            });
                            ui.replace(newui);
                        }
                    });
                })
                .unwrap();
            } else if state == AppState::QuitApp {
                slint::quit_event_loop().unwrap();
                exit(0);
            } else if state != AppState::MainWindowOpen {
                if let Ok(config) = config_clone.lock() {
                    if !config.run_in_background {
                        slint::quit_event_loop().unwrap();
                        exit(0);
                    }
                }
            }
        }
    });

    slint::run_event_loop_until_quit().unwrap();
    rt.shutdown_background();
    Ok(())
}

/// Set up the main window
fn setup_window(_config: Arc<Mutex<Config>>) -> MainWindow {
    let ui = MainWindow::new().expect("Failed to create main window");

    // Set up initial UI state
    // This will be expanded to wire up all D-Bus connections and monitoring

    ui
}

/// Handle CLI help and version flags
fn do_cli_help(parsed: &CliStart) -> bool {
    if parsed.help {
        println!("{}", CliStart::usage());
        println!();
        if let Some(cmdlist) = CliStart::command_list() {
            let commands: Vec<String> = cmdlist.lines().map(|s| s.to_owned()).collect();
            for command in &commands {
                println!("{}", command);
            }
        }
    }

    if parsed.version {
        print_versions();
        println!();
    }

    parsed.help || parsed.version
}
