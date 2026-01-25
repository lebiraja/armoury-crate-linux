//! D-Bus proxy for inter-process communication
//!
//! Provides application state management over D-Bus for tray icon and window control.

use std::sync::{Arc, Mutex};

use zbus::blocking::proxy::ProxyImpl;
use zbus::blocking::{fdo, Connection};
use zbus::zvariant::{OwnedValue, Type, Value};
use zbus::{interface, proxy};

/// Application state for IPC between tray and main window
#[derive(Debug, Copy, Clone, PartialEq, Eq, Type, Value, OwnedValue)]
#[zvariant(signature = "u")]
pub enum AppState {
    MainWindowOpen = 0,
    /// If the app is running, open the main window
    MainWindowShouldOpen = 1,
    MainWindowClosed = 2,
    StartingUp = 3,
    QuitApp = 4,
    LockFailed = 5,
}

/// D-Bus interface implementation for Armoury Crate Linux
pub struct ArmouryCrateZbus {
    state: Arc<Mutex<AppState>>,
}

impl ArmouryCrateZbus {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState::StartingUp)),
        }
    }

    pub fn clone_state(&self) -> Arc<Mutex<AppState>> {
        self.state.clone()
    }
}

pub const ZBUS_PATH: &str = "/xyz/ljones/ArmouryCrate";
pub const ZBUS_IFACE: &str = "xyz.ljones.ArmouryCrate";

#[interface(name = "xyz.ljones.ArmouryCrate")]
impl ArmouryCrateZbus {
    /// Return the current application state
    #[zbus(property)]
    async fn state(&self) -> AppState {
        if let Ok(lock) = self.state.try_lock() {
            return *lock;
        }
        AppState::LockFailed
    }

    #[zbus(property)]
    async fn set_state(&self, state: AppState) {
        if let Ok(mut lock) = self.state.try_lock() {
            *lock = state;
        }
    }
}

/// D-Bus proxy for communicating with Armoury Crate Linux
/// Used by the tray icon to control the main window
#[proxy(
    interface = "xyz.ljones.ArmouryCrate",
    default_service = "xyz.ljones.ArmouryCrate",
    default_path = "/xyz/ljones/ArmouryCrate"
)]
pub trait ROGCCZbus {
    /// State property - current application state
    #[zbus(property)]
    fn state(&self) -> zbus::Result<AppState>;

    #[zbus(property)]
    fn set_state(&self, state: AppState) -> zbus::Result<()>;
}

/// Find D-Bus interface implementations by name
pub fn find_iface<T>(iface_name: &str) -> Result<Vec<T>, Box<dyn std::error::Error>>
where
    T: ProxyImpl<'static> + From<zbus::Proxy<'static>>,
{
    let conn = Connection::system()?;
    let f = fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/")?;
    let interfaces = f.get_managed_objects()?;
    let mut paths = Vec::new();
    for v in interfaces.iter() {
        for k in v.1.keys() {
            if k.as_str() == iface_name {
                paths.push(v.0.clone());
            }
        }
    }
    if paths.len() > 1 {
        log::warn!("Multiple asusd interfaces devices found");
    }
    if !paths.is_empty() {
        let mut ctrl = Vec::new();
        paths.sort_by(|a, b| a.cmp(b));
        for path in paths {
            ctrl.push(
                T::builder(&conn)
                    .path(path.clone())?
                    .destination("xyz.ljones.Asusd")?
                    .build()?,
            );
        }
        return Ok(ctrl);
    }

    Err("No Aura interface".into())
}

/// Find D-Bus interface implementations by name (async version)
pub async fn find_iface_async<T>(iface_name: &str) -> Result<Vec<T>, Box<dyn std::error::Error>>
where
    T: zbus::proxy::ProxyImpl<'static> + From<zbus::Proxy<'static>>,
{
    let conn = zbus::Connection::system().await?;
    let f = zbus::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/")
        .await?;
    let interfaces = f.get_managed_objects().await?;
    let mut paths = Vec::new();
    for v in interfaces.iter() {
        for k in v.1.keys() {
            if k.as_str() == iface_name {
                paths.push(v.0.clone());
            }
        }
    }
    if paths.len() > 1 {
        log::warn!("Multiple asusd interfaces devices found");
    }
    if !paths.is_empty() {
        let mut ctrl = Vec::new();
        paths.sort_by(|a, b| a.cmp(b));
        for path in paths {
            ctrl.push(
                T::builder(&conn)
                    .path(path.clone())?
                    .destination("xyz.ljones.Asusd")?
                    .build()
                    .await?,
            );
        }
        return Ok(ctrl);
    }

    Err("No interface".into())
}
