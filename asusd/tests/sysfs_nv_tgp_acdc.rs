use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use tempfile::tempdir;

use asusd::asus_armoury::set_config_or_default;
use asusd::config::Config;
use rog_platform::asus_armoury::FirmwareAttributes;
use rog_platform::platform::PlatformProfile;

fn write_attr_dir(base: &PathBuf, name: &str, default: &str, display: &str) {
    let attr_dir = base.join(name);
    create_dir_all(&attr_dir).unwrap();

    let mut f = File::create(attr_dir.join("default_value")).unwrap();
    write!(f, "{}", default).unwrap();
    let mut f = File::create(attr_dir.join("display_name")).unwrap();
    write!(f, "{}", display).unwrap();
    // create current_value file so set_current_value can open for write
    let mut f = File::create(attr_dir.join("current_value")).unwrap();
    write!(f, "{}", default).unwrap();
}

#[test]
fn nv_tgp_ac_dc_applies_different_values() {
    let td = tempdir().unwrap();
    let base = td.path().join("attributes");
    create_dir_all(&base).unwrap();

    // create mock attribute nv_tgp
    write_attr_dir(&base, "nv_tgp", "0", "nv_tgp");

    // Build FirmwareAttributes from this dir
    let attrs = FirmwareAttributes::from_dir(&base);

    // Create a config with different AC/DC tunings for Performance profile
    let mut cfg = Config::default();
    let profile = PlatformProfile::Performance;
    {
        let tuning_ac = cfg.select_tunings(true, profile);
        tuning_ac.enabled = true;
        tuning_ac
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::DgpuTgp, 123);

        let tuning_dc = cfg.select_tunings(false, profile);
        tuning_dc.enabled = true;
        tuning_dc
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::DgpuTgp, 45);
    }

    // Apply for AC
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, true, profile).await;
    });

    let nv_tgp_val_path = base.join("nv_tgp").join("current_value");
    let val_ac = std::fs::read_to_string(&nv_tgp_val_path).unwrap();
    assert_eq!(val_ac.trim(), "123");

    // Now apply for DC
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, false, profile).await;
    });

    let val_dc = std::fs::read_to_string(&nv_tgp_val_path).unwrap();
    assert_eq!(val_dc.trim(), "45");
}
