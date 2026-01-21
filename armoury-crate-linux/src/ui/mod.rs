//! UI setup module for Armoury Crate Linux
//!
//! Contains setup functions and macros for connecting Slint UI to D-Bus.

use log::warn;
use slint::{SharedString, Weak};

use crate::MainWindow;

pub mod setup_aura;
pub mod setup_fans;
pub mod setup_system;

/// Macro for setting up UI property from D-Bus with async fetch
#[macro_export]
macro_rules! set_ui_props_async {
    ($handle:ident, $proxy:ident, $global:ident, $property:ident) => {
        if let Ok(val) = $proxy.$property().await {
            let handle_copy = $handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = handle_copy.upgrade() {
                    ui.global::<$global>()
                        .set(concat_idents::concat_idents!(set_ = set_, $property { set_($property)(val.into()) }));
                }
            });
        }
    };
}

/// Macro for setting up bidirectional binding between UI callback and D-Bus property
#[macro_export]
macro_rules! set_ui_callbacks {
    ($handle:expr,
     $global:ident,
     $proxy:expr,
     $property:ident,
     $to_rust:ty,
     $success_msg:literal,
     $fail_msg:literal
    ) => {{
        // Setup UI → D-Bus callback
        let proxy_copy = $proxy.clone();
        let handle_weak = $handle.as_weak();

        // Construct callback name: cb_<property>
        paste::paste! {
            $handle.global::<$global>().[<on_cb_ $property>](move |value| {
                let proxy_inner = proxy_copy.clone();
                let handle_copy = handle_weak.clone();
                let rust_value: $to_rust = value.into();

                tokio::spawn(async move {
                    let result = proxy_inner.[<set_ $property>](rust_value).await;
                    $crate::ui::show_toast(
                        format!($success_msg, rust_value).into(),
                        $fail_msg.into(),
                        handle_copy,
                        result,
                    );
                });
            });
        }

        // Setup D-Bus → UI signal handler
        let proxy_copy = $proxy.clone();
        let handle_weak = $handle.as_weak();

        tokio::spawn(async move {
            paste::paste! {
                let mut stream = proxy_copy.[<receive_ $property _changed>]().await;
                use futures_util::StreamExt;
                while let Some(event) = stream.next().await {
                    if let Ok(value) = event.get().await {
                        let handle_inner = handle_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = handle_inner.upgrade() {
                                ui.global::<$global>().[<set_ $property>](value.into());
                            }
                        });
                    }
                }
            }
        });
    }};
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
