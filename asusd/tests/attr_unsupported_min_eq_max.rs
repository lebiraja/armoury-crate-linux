use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use tempfile::tempdir;

use asusd::asus_armoury::set_config_or_default;
use asusd::config::Config;
use rog_platform::asus_armoury::FirmwareAttributes;
use rog_platform::platform::PlatformProfile;

fn write_attr_dir_with_min_max(base: &PathBuf, name: &str, default: &str, min: &str, max: &str) {
    let attr_dir = base.join(name);
    create_dir_all(&attr_dir).unwrap();

    let mut f = File::create(attr_dir.join("default_value")).unwrap();
    write!(f, "{}", default).unwrap();
    let mut f = File::create(attr_dir.join("display_name")).unwrap();
    write!(f, "{}", name).unwrap();
    // create current_value file so set_current_value can open for write
    let mut f = File::create(attr_dir.join("current_value")).unwrap();
    write!(f, "{}", default).unwrap();
    // write explicit min and max
    let mut f = File::create(attr_dir.join("min_value")).unwrap();
    write!(f, "{}", min).unwrap();
    let mut f = File::create(attr_dir.join("max_value")).unwrap();
    write!(f, "{}", max).unwrap();
}

#[test]
fn attribute_with_min_eq_max_is_unsupported_and_skipped() {
    let td = tempdir().unwrap();
    let base = td.path().join("attributes");
    create_dir_all(&base).unwrap();

    // create an attribute where min == max (no range)
    write_attr_dir_with_min_max(&base, "nv_dynamic_boost", "5", "10", "10");

    let attrs = FirmwareAttributes::from_dir(&base);

    let mut cfg = Config::default();
    let profile = PlatformProfile::Performance;

    // set stored tuning that would normally be applied
    {
        let ac = cfg.select_tunings(true, profile);
        ac.enabled = true;
        ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvDynamicBoost,
            9,
        );
    }

    let rt = tokio::runtime::Runtime::new().unwrap();

    // apply AC
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, true, profile).await;
    });

    // Since min==max the attribute is considered unsupported and the current_value should remain the default (5)
    assert_eq!(
        std::fs::read_to_string(base.join("nv_dynamic_boost").join("current_value"))
            .unwrap()
            .trim(),
        "5"
    );
}
