pub use asusd::{DBUS_IFACE, DBUS_NAME, DBUS_PATH};
use zbus::proxy::ProxyImpl;

pub mod asus_armoury;
pub mod scsi_aura;
pub mod zbus_anime;
pub mod zbus_aura;
pub mod zbus_backlight;
pub mod zbus_fan_curves;
pub mod zbus_platform;
pub mod zbus_slash;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn list_iface_blocking() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let conn = zbus::blocking::Connection::system()?;
    // Try ObjectManager first
    if let Ok(f) = zbus::blocking::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/") {
        if let Ok(interfaces) = f.get_managed_objects() {
            let mut ifaces = Vec::new();
            for v in interfaces.iter() {
                for k in v.1.keys() {
                    ifaces.push(k.to_string());
                }
            }
            return Ok(ifaces);
        }
    }
    // Fallback: Return empty list if ObjectManager fails (or implement introspection fallback if needed)
    // For now, returning empty allows caller to handle "no interfaces" without crashing
    Ok(Vec::new())
}

pub fn has_iface_blocking(iface: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let conn = zbus::blocking::Connection::system()?;
    if let Ok(f) = zbus::blocking::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/") {
        if let Ok(interfaces) = f.get_managed_objects() {
            for v in interfaces.iter() {
                for k in v.1.keys() {
                    if k.as_str() == iface {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub async fn has_iface(iface: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let conn = zbus::Connection::system().await?;
    if let Ok(f) = zbus::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/").await {
        if let Ok(interfaces) = f.get_managed_objects().await {
            for v in interfaces.iter() {
                for k in v.1.keys() {
                    if k.as_str() == iface {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub async fn find_iface_async<T>(iface_name: &str) -> Result<Vec<T>, Box<dyn std::error::Error>>
where
    T: ProxyImpl<'static> + From<zbus::Proxy<'static>>,
{
    let conn = zbus::Connection::system().await?;
    let mut paths = Vec::new();

    // 1. Try ObjectManager
    if let Ok(f) = zbus::fdo::ObjectManagerProxy::new(&conn, "xyz.ljones.Asusd", "/").await {
        if let Ok(interfaces) = f.get_managed_objects().await {
            for v in interfaces.iter() {
                for k in v.1.keys() {
                    if k.as_str() == iface_name {
                        paths.push(v.0.clone());
                    }
                }
            }
        }
    }

    // 2. Fallback: Introspection for specific known paths if ObjectManager failed/returned empty
    if paths.is_empty() {
        // Known locations for interfaces
        let search_paths = match iface_name {
            "xyz.ljones.Anime" => vec!["/xyz/ljones/anime"], // Guessing anime path
            "xyz.ljones.Aura" => vec!["/xyz/ljones/aura"],
            _ => vec![],
        };

        for path in search_paths {
            // Introspect directory to find children
            if let Ok(intro) = zbus::fdo::IntrospectableProxy::builder(&conn)
                .destination("xyz.ljones.Asusd")?
                .path(path)?
                .build()
                .await
            {
                if let Ok(xml) = intro.introspect().await {
                    for line in xml.lines() {
                        let line = line.trim();
                        if line.starts_with("<node") && line.contains("name=\"") {
                            if let Some(start) = line.find("name=\"") {
                                let rest = &line[start + 6..];
                                if let Some(end) = rest.find('"') {
                                    let name = &rest[..end];
                                    if !name.is_empty() && !name.starts_with('.') {
                                        let full_path = format!("{}/{}", path, name);
                                        if let Ok(obj_path) = zbus::zvariant::ObjectPath::try_from(full_path) {
                                            paths.push(obj_path.into());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if paths.len() > 1 {
        println!("Multiple asusd interfaces devices found");
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

    Err(format!("Did not find {iface_name}").into())
}
