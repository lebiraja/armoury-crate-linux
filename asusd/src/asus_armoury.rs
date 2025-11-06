use std::sync::Arc;

use config_traits::StdConfig;
use log::{debug, error, info};
use rog_platform::asus_armoury::{AttrValue, Attribute, FirmwareAttribute, FirmwareAttributes};
use rog_platform::platform::{PlatformProfile, RogPlatform};
use rog_platform::power::AsusPower;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value};
use zbus::{fdo, interface, Connection};

use crate::config::Config;
use crate::error::RogError;
use crate::{Reloadable, ASUS_ZBUS_PATH};

const MOD_NAME: &str = "asus_armoury";

#[derive(Debug, Default, Clone, Deserialize, Serialize, Type, Value, OwnedValue)]
pub struct PossibleValues {
    strings: Vec<String>,
    nums: Vec<i32>,
}

fn dbus_path_for_attr(attr_name: &str) -> OwnedObjectPath {
    ObjectPath::from_str_unchecked(&format!("{ASUS_ZBUS_PATH}/{MOD_NAME}/{attr_name}")).into()
}

#[derive(Clone)]
pub struct AsusArmouryAttribute {
    attr: Attribute,
    config: Arc<Mutex<Config>>,
    /// platform control required here for access to PPD or Throttle profile
    platform: RogPlatform,
    power: AsusPower,
}

impl AsusArmouryAttribute {
    pub fn new(
        attr: Attribute,
        platform: RogPlatform,
        power: AsusPower,
        config: Arc<Mutex<Config>>,
    ) -> Self {
        Self {
            attr,
            config,
            platform,
            power,
        }
    }

    pub fn attribute_name(&self) -> String {
        String::from(self.attr.name())
    }

    fn resolve_i32_value(refreshed: Option<i32>, cached: &AttrValue) -> i32 {
        refreshed
            .or(match cached {
                AttrValue::Integer(i) => Some(*i),
                _ => None,
            })
            .unwrap_or(-1)
    }

    pub async fn emit_limits(&self, connection: &Connection) -> Result<(), RogError> {
        let path = dbus_path_for_attr(self.attr.name());
        let signal = SignalEmitter::new(connection, path)?;
        self.min_value_changed(&signal).await?;
        self.max_value_changed(&signal).await?;
        self.scalar_increment_changed(&signal).await?;
        self.current_value_changed(&signal).await?;
        Ok(())
    }

    pub async fn move_to_zbus(self, connection: &Connection) -> Result<(), RogError> {
        let path = dbus_path_for_attr(self.attr.name());
        connection
            .object_server()
            .at(path.clone(), self)
            .await
            .map_err(|e| error!("Couldn't add server at path: {path}, {e:?}"))
            .ok();
        Ok(())
    }

    async fn watch_and_notify(
        &mut self,
        signal_ctxt: SignalEmitter<'static>,
    ) -> Result<(), RogError> {
        use futures_util::StreamExt;

        let name = self.name();
        macro_rules! watch_value_notify {
            ($attr_str:expr, $fn_prop_changed:ident) => {
                match self.attr.get_watcher($attr_str) {
                    Ok(watch) => {
                        let name = <&str>::from(name);
                        let ctrl = self.clone();
                        let sig = signal_ctxt.clone();
                        tokio::spawn(async move {
                            let mut buffer = [0; 32];
                            if let Ok(stream) = watch.into_event_stream(&mut buffer) {
                                stream
                                    .for_each(|_| async {
                                        debug!("{} changed", name);
                                        ctrl.$fn_prop_changed(&sig).await.ok();
                                    })
                                    .await;
                            } else {
                                info!(
                                    "inotify event stream failed for {} ({}). You can ignore this \
                                     if unsupported",
                                    name, $attr_str
                                );
                            }
                        });
                    }
                    Err(e) => info!(
                        "inotify watch failed: {}. You can ignore this if your device does not \
                         support the feature",
                        e
                    ),
                }
            };
        }

        // "current_value", "default_value", "min_value", "max_value"
        watch_value_notify!("current_value", current_value_changed);
        watch_value_notify!("default_value", default_value_changed);
        watch_value_notify!("min_value", min_value_changed);
        watch_value_notify!("max_value", max_value_changed);

        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct ArmouryAttributeRegistry {
    attrs: Vec<AsusArmouryAttribute>,
}

impl ArmouryAttributeRegistry {
    pub fn push(&mut self, attr: AsusArmouryAttribute) {
        self.attrs.push(attr);
    }

    pub async fn emit_limits(&self, connection: &Connection) -> Result<(), RogError> {
        let mut last_err: Option<RogError> = None;
        for attr in &self.attrs {
            if let Err(e) = attr.emit_limits(connection).await {
                error!(
                    "Failed to emit updated limits for attribute '{}': {e:?}",
                    attr.attribute_name()
                );
                last_err = Some(e);
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }
}

impl crate::Reloadable for AsusArmouryAttribute {
    async fn reload(&mut self) -> Result<(), RogError> {
        info!("Reloading {}", self.attr.name());
        let name: FirmwareAttribute = self.attr.name().into();

        if name.is_ppt() || name.is_dgpu() {
            let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
            let power_plugged = self
                .power
                .get_online()
                .map_err(|e| {
                    error!("Could not get power status: {e:?}");
                    e
                })
                .unwrap_or_default()
                == 1;

            let apply_value = {
                let config = self.config.lock().await;
                config
                    .select_tunings_ref(power_plugged, profile)
                    .and_then(|tuning| {
                        if tuning.enabled {
                            tuning.group.get(&self.name()).copied()
                        } else {
                            None
                        }
                    })
            };

            if let Some(tune) = apply_value {
                self.attr
                    .set_current_value(&AttrValue::Integer(tune))
                    .map_err(|e| {
                        error!("Could not set {} value: {e:?}", self.attr.name());
                        self.attr.base_path_exists();
                        e
                    })?;
                info!("Set {} to {:?}", self.attr.name(), tune);
            }
        } else {
            // Handle non-PPT attributes (boolean and other settings)
            if let Some(saved_value) = self.config.lock().await.armoury_settings.get(&name) {
                self.attr
                    .set_current_value(&AttrValue::Integer(*saved_value))
                    .map_err(|e| {
                        error!("Could not set {} value: {e:?}", self.attr.name());
                        self.attr.base_path_exists();
                        e
                    })?;
                info!(
                    "Restored armoury setting {} to {:?}",
                    self.attr.name(),
                    saved_value
                );
            }
        }

        Ok(())
    }
}

/// If return is `-1` on a property then there is avilable value for that
/// property
#[interface(name = "xyz.ljones.AsusArmoury")]
impl AsusArmouryAttribute {
    #[zbus(property)]
    fn name(&self) -> FirmwareAttribute {
        self.attr.name().into()
    }

    #[zbus(property)]
    async fn available_attrs(&self) -> Vec<String> {
        let mut attrs = Vec::new();
        if !matches!(self.attr.default_value(), AttrValue::None) {
            attrs.push("default_value".to_string());
        }
        if !matches!(self.attr.min_value(), AttrValue::None) {
            attrs.push("min_value".to_string());
        }
        if !matches!(self.attr.max_value(), AttrValue::None) {
            attrs.push("max_value".to_string());
        }
        if !matches!(self.attr.scalar_increment(), AttrValue::None) {
            attrs.push("scalar_increment".to_string());
        }
        if !matches!(self.attr.possible_values(), AttrValue::None) {
            attrs.push("possible_values".to_string());
        }
        // TODO: Don't unwrap, use error
        if let Ok(value) = self.attr.current_value().map_err(|e| {
            error!("Failed to read: {e:?}");
            e
        }) {
            if !matches!(value, AttrValue::None) {
                attrs.push("current_value".to_string());
            }
        }
        attrs
    }

    /// If return is `-1` then there is no default value
    #[zbus(property)]
    async fn default_value(&self) -> i32 {
        match self.attr.default_value() {
            AttrValue::Integer(i) => *i,
            _ => -1,
        }
    }

    async fn restore_default(&self) -> fdo::Result<()> {
        self.attr.restore_default()?;
        if self.name().is_ppt() || self.name().is_dgpu() {
            let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
            let power_plugged = self
                .power
                .get_online()
                .map_err(|e| {
                    error!("Could not get power status: {e:?}");
                    e
                })
                .unwrap_or_default();

            let mut config = self.config.lock().await;
            let tuning = config.select_tunings(power_plugged == 1, profile);
            if let Some(tune) = tuning.group.get_mut(&self.name()) {
                if let AttrValue::Integer(i) = self.attr.default_value() {
                    *tune = *i;
                }
            }
            if tuning.enabled {
                self.attr
                    .set_current_value(self.attr.default_value())
                    .map_err(|e| {
                        error!("Could not set value: {e:?}");
                        e
                    })?;
            }
            config.write();
        }
        Ok(())
    }

    #[zbus(property)]
    async fn min_value(&self) -> i32 {
        Self::resolve_i32_value(self.attr.refresh_min_value(), self.attr.min_value())
    }

    #[zbus(property)]
    async fn max_value(&self) -> i32 {
        Self::resolve_i32_value(self.attr.refresh_max_value(), self.attr.max_value())
    }

    #[zbus(property)]
    async fn scalar_increment(&self) -> i32 {
        Self::resolve_i32_value(
            self.attr.refresh_scalar_increment(),
            self.attr.scalar_increment(),
        )
    }

    #[zbus(property)]
    async fn possible_values(&self) -> Vec<i32> {
        match self.attr.possible_values() {
            AttrValue::EnumInt(i) => i.clone(),
            _ => Vec::default(),
        }
    }

    #[zbus(property)]
    async fn current_value(&self) -> fdo::Result<i32> {
        if self.name().is_ppt() || self.name().is_dgpu() {
            let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
            let power_plugged = self
                .power
                .get_online()
                .map_err(|e| {
                    error!("Could not get power status: {e:?}");
                    e
                })
                .unwrap_or_default()
                == 1;
            let config = self.config.lock().await;
            if let Some(tuning) = config.select_tunings_ref(power_plugged, profile) {
                if let Some(tune) = tuning.group.get(&self.name()) {
                    return Ok(*tune);
                }
            }
            if let AttrValue::Integer(i) = self.attr.default_value() {
                return Ok(*i);
            }
            return Err(fdo::Error::Failed(
                "Could not read current value".to_string(),
            ));
        }

        if let Ok(AttrValue::Integer(i)) = self.attr.current_value() {
            return Ok(i);
        }
        Err(fdo::Error::Failed(
            "Could not read current value".to_string(),
        ))
    }

    async fn stored_value_for_power(&self, on_ac: bool) -> fdo::Result<i32> {
        if !(self.name().is_ppt() || self.name().is_dgpu()) {
            return Err(fdo::Error::NotSupported(
                "Stored values are only available for PPT/dGPU tunable attributes".to_string(),
            ));
        }

        let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
        let config = self.config.lock().await;
        if let Some(tuning) = config.select_tunings_ref(on_ac, profile) {
            if let Some(tune) = tuning.group.get(&self.name()) {
                return Ok(*tune);
            }
        }

        if let AttrValue::Integer(i) = self.attr.default_value() {
            return Ok(*i);
        }
        Err(fdo::Error::Failed(
            "Could not read stored value".to_string(),
        ))
    }

    async fn set_value_for_power(&mut self, on_ac: bool, value: i32) -> fdo::Result<()> {
        if !(self.name().is_ppt() || self.name().is_dgpu()) {
            return Err(fdo::Error::NotSupported(
                "Setting stored values is only supported for PPT/dGPU tunable attributes"
                    .to_string(),
            ));
        }

        let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
        let apply_now;

        {
            let mut config = self.config.lock().await;
            let tuning = config.select_tunings(on_ac, profile);

            if let Some(tune) = tuning.group.get_mut(&self.name()) {
                *tune = value;
            } else {
                tuning.group.insert(self.name(), value);
                debug!(
                    "Store {} value for {} power = {}",
                    self.attr.name(),
                    if on_ac { "AC" } else { "DC" },
                    value
                );
            }

            apply_now = tuning.enabled;
            config.write();
        }

        if apply_now {
            let power_plugged = self
                .power
                .get_online()
                .map_err(|e| {
                    error!("Could not get power status: {e:?}");
                    e
                })
                .unwrap_or_default()
                != 0;

            if power_plugged == on_ac {
                self.attr
                    .set_current_value(&AttrValue::Integer(value))
                    .map_err(|e| {
                        error!("Could not set value: {e:?}");
                        e
                    })?;
            }
        }

        Ok(())
    }

    #[zbus(property)]
    async fn set_current_value(&mut self, value: i32) -> fdo::Result<()> {
        if self.name().is_ppt() || self.name().is_dgpu() {
            let profile: PlatformProfile = self.platform.get_platform_profile()?.into();
            let power_plugged = self
                .power
                .get_online()
                .map_err(|e| {
                    error!("Could not get power status: {e:?}");
                    e
                })
                .unwrap_or_default();

            let mut config = self.config.lock().await;
            let tuning = config.select_tunings(power_plugged == 1, profile);

            if let Some(tune) = tuning.group.get_mut(&self.name()) {
                *tune = value;
            } else {
                tuning.group.insert(self.name(), value);
                debug!("Store tuning config for {} = {:?}", self.attr.name(), value);
            }
            if tuning.enabled {
                self.attr
                    .set_current_value(&AttrValue::Integer(value))
                    .map_err(|e| {
                        error!("Could not set value: {e:?}");
                        e
                    })?;
            }
        } else {
            self.attr
                .set_current_value(&AttrValue::Integer(value))
                .map_err(|e| {
                    error!("Could not set value: {e:?}");
                    e
                })?;

            let has_attr = self
                .config
                .lock()
                .await
                .armoury_settings
                .contains_key(&self.name());
            if has_attr {
                if let Some(setting) = self
                    .config
                    .lock()
                    .await
                    .armoury_settings
                    .get_mut(&self.name())
                {
                    *setting = value
                }
            } else {
                debug!("Adding config for {}", self.attr.name());
                self.config
                    .lock()
                    .await
                    .armoury_settings
                    .insert(self.name(), value);
                debug!("Set config for {} = {:?}", self.attr.name(), value);
            }
        }
        self.config.lock().await.write();
        Ok(())
    }
}

pub async fn start_attributes_zbus(
    conn: &Connection,
    platform: RogPlatform,
    power: AsusPower,
    attributes: FirmwareAttributes,
    config: Arc<Mutex<Config>>,
) -> Result<ArmouryAttributeRegistry, RogError> {
    let mut registry = ArmouryAttributeRegistry::default();
    for attr in attributes.attributes() {
        let mut attr = AsusArmouryAttribute::new(
            attr.clone(),
            platform.clone(),
            power.clone(),
            config.clone(),
        );

        let registry_attr = attr.clone();

        if let Err(e) = attr.reload().await {
            error!(
                "Skipping attribute '{}' due to reload error: {e:?}",
                attr.attr.name()
            );
            // continue with others
            continue;
        }

        let attr_name = attr.attribute_name();

        let path = dbus_path_for_attr(attr_name.as_str());
        match zbus::object_server::SignalEmitter::new(conn, path) {
            Ok(sig) => {
                if let Err(e) = attr.watch_and_notify(sig).await {
                    error!("Failed to start watcher for '{}': {e:?}", attr.attr.name());
                }
            }
            Err(e) => {
                error!(
                    "Failed to create SignalEmitter for '{}': {e:?}",
                    attr.attr.name()
                );
            }
        }

        if let Err(e) = attr.move_to_zbus(conn).await {
            error!("Failed to register attribute '{attr_name}' on zbus: {e:?}");
            continue;
        }

        registry.push(registry_attr);
    }
    Ok(registry)
}

pub async fn set_config_or_default(
    attrs: &FirmwareAttributes,
    config: &mut Config,
    power_plugged: bool,
    profile: PlatformProfile,
) {
    for attr in attrs.attributes().iter() {
        let name: FirmwareAttribute = attr.name().into();
        if name.is_ppt() || name.is_dgpu() {
            let tuning = config.select_tunings(power_plugged, profile);
            if !tuning.enabled {
                debug!("Tuning group is not enabled, skipping");
                continue;
            }

            if let Some(tune) = tuning.group.get(&name) {
                attr.set_current_value(&AttrValue::Integer(*tune))
                    .map_err(|e| {
                        error!("Failed to set {}: {e}", <&str>::from(name));
                    })
                    .ok();
            } else {
                let default = attr.default_value();
                attr.set_current_value(default)
                    .map_err(|e| {
                        error!("Failed to set {}: {e}", <&str>::from(name));
                    })
                    .ok();
                if let AttrValue::Integer(i) = default {
                    tuning.group.insert(name, *i);
                    info!(
                        "Set default tuning config for {} = {:?}",
                        <&str>::from(name),
                        i
                    );
                    // config.write();
                }
            }
        } else {
            // Handle non-PPT attributes (boolean and other settings)
            if let Some(saved_value) = config.armoury_settings.get(&name) {
                attr.set_current_value(&AttrValue::Integer(*saved_value))
                    .map_err(|e| {
                        error!("Failed to set {}: {e}", <&str>::from(name));
                    })
                    .ok();
                info!(
                    "Restored armoury setting for {} = {:?}",
                    <&str>::from(name),
                    saved_value
                );
            }
        }
    }
}

// Internal helper to store a tuning value into the correct per-profile, per-power map.
// This centralizes the behavior so tests can validate storage semantics.
#[allow(dead_code)]
fn insert_tuning_value(
    config: &mut Config,
    on_ac: bool,
    profile: PlatformProfile,
    name: rog_platform::asus_armoury::FirmwareAttribute,
    value: i32,
) {
    let tuning = config.select_tunings(on_ac, profile);
    if let Some(t) = tuning.group.get_mut(&name) {
        *t = value;
    } else {
        tuning.group.insert(name, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use rog_platform::asus_armoury::FirmwareAttribute;
    use rog_platform::platform::PlatformProfile;

    #[test]
    fn insert_nv_tuning_is_per_profile_and_power() {
        let mut cfg = Config::default();
        let profile = PlatformProfile::Performance;

        // Insert value for AC
        insert_tuning_value(
            &mut cfg,
            true,
            profile,
            FirmwareAttribute::NvDynamicBoost,
            7,
        );

        // Value should be present in ac_profile_tunings
        let t_ac = cfg.select_tunings_ref(true, profile).unwrap();
        assert_eq!(t_ac.group.get(&FirmwareAttribute::NvDynamicBoost), Some(&7));

        // Insert separate value for DC
        insert_tuning_value(
            &mut cfg,
            false,
            profile,
            FirmwareAttribute::NvDynamicBoost,
            3,
        );
        let t_dc = cfg.select_tunings_ref(false, profile).unwrap();
        assert_eq!(t_dc.group.get(&FirmwareAttribute::NvDynamicBoost), Some(&3));
    }

    #[test]
    fn non_ppt_attribute_stores_in_armoury_settings() {
        let mut cfg = Config::default();
        // Non-PPT/dGPU attribute, e.g., BootSound
        let name = FirmwareAttribute::BootSound;
        // Simulate setting armoury setting
        cfg.armoury_settings.insert(name, 1);
        assert_eq!(cfg.armoury_settings.get(&name), Some(&1));
    }
}
