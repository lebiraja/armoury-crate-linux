//! UI setup module for Armoury Crate Linux
//!
//! Contains setup functions and macros for connecting Slint UI to D-Bus.

use log::warn;
use slint::{SharedString, Weak};

use crate::MainWindow;

pub mod setup_anime;
pub mod setup_aura;
pub mod setup_dashboard;
pub mod setup_fans;
pub mod setup_scenario;
pub mod setup_settings;
pub mod setup_system;

/// Macro for setting up UI property from D-Bus with async fetch
#[macro_export]
macro_rules! set_ui_props_async {
    ($ui:ident, $proxy:ident, $global:ident, $proxy_fn:ident) => {
        if let Ok(value) = $proxy.$proxy_fn().await {
            $ui.upgrade_in_event_loop(move |handle| {
                concat_idents::concat_idents!(set = set_, $proxy_fn {
                    handle.global::<$global>().set(value.into());
                });
            }).ok();
        }
    };
}

// This macro sets up:
// - a link from UI callback -> dbus proxy property
// - a link from dbus property signal -> UI state
// conv1 and conv2 are type conversion args
#[macro_export]
macro_rules! set_ui_callbacks {
    ($handle:ident, $data:ident($($conv1: tt)*),$proxy:ident.$proxy_fn:tt($($conv2: tt)*),$success:literal,$failed:literal) => {
        let handle_copy = $handle.as_weak();
        let proxy_copy = $proxy.clone();
        let data = $handle.global::<$data>();
        concat_idents::concat_idents!(on_set = on_cb_, $proxy_fn {
        data.on_set(move |value| {
            let proxy_copy = proxy_copy.clone();
            let handle_copy = handle_copy.clone();
            tokio::spawn(async move {
                concat_idents::concat_idents!(set = set_, $proxy_fn {
                show_toast(
                    format!($success, value).into(),
                    $failed.into(),
                    handle_copy,
                    proxy_copy.set(value $($conv2)*).await,
                );
                });
            });
            });
        });
        let handle_copy = $handle.as_weak();
        let proxy_copy = $proxy.clone();
        concat_idents::concat_idents!(receive = receive_, $proxy_fn, _changed {
        // spawn required since the while let never exits
        tokio::spawn(async move {
            let mut x = proxy_copy.receive().await;
            concat_idents::concat_idents!(set = set_, $proxy_fn {
            use futures_util::StreamExt;
            while let Some(e) = x.next().await {
                if let Ok(out) = e.get().await {
                    handle_copy.upgrade_in_event_loop(move |handle| {
                        handle.global::<$data>().set(out $($conv1)*);
                    }).ok();
                }
            }
            });
        });
        });
    };
}

/// Show a toast notification based on D-Bus result
pub fn show_toast(
    success: SharedString,
    fail: SharedString,
    handle: Weak<MainWindow>,
    result: zbus::Result<()>,
) {
    match result {
        Ok(_) => {
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle.upgrade() {
                    ui.invoke_show_toast(success);
                }
            })
            .ok();
        }
        Err(e) => {
            slint::invoke_from_event_loop(move || {
                warn!("{fail}: {e}");
                if let Some(ui) = handle.upgrade() {
                    ui.invoke_show_toast(fail);
                }
            })
            .ok();
        }
    };
}

/// Setup all UI pages with initial values and callbacks
pub fn setup_all_pages(ui: &MainWindow) {
    // Setup system page (blocking initial load)
    setup_system::setup_system_page(ui);

    // Setup aura page (blocking initial load)
    setup_aura::setup_aura_page(ui);

    // Setup fans page (blocking initial load)
    setup_fans::setup_fans_page(ui);
}

/// Setup async callbacks for all pages (must be called after setup_all_pages)
pub fn setup_all_callbacks(ui: &MainWindow) {
    // Setup async callbacks for system page
    setup_system::setup_system_page_callbacks(ui);

    // Setup async callbacks for aura page
    setup_aura::setup_aura_page_callbacks(ui);

    // Setup async callbacks for fans page
    setup_fans::setup_fans_page_callbacks(ui);
}
