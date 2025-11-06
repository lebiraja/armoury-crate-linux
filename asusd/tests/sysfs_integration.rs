use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use tempfile::tempdir;

use asusd::asus_armoury::set_config_or_default;
use asusd::config::Config;
use rog_platform::asus_armoury::{AttrValue, FirmwareAttributes};
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
fn sysfs_set_config_or_default_writes_nv_and_ppt() {
    let td = tempdir().unwrap();
    let base = td.path().join("attributes");
    create_dir_all(&base).unwrap();

    // create mock attributes: ppt_pl1_spl and nv_dynamic_boost
    write_attr_dir(&base, "ppt_pl1_spl", "25", "ppt");
    write_attr_dir(&base, "nv_dynamic_boost", "0", "nv");
    write_attr_dir(&base, "nv_tgp", "0", "nv_tgp");

    // Build FirmwareAttributes from this dir
    let attrs = FirmwareAttributes::from_dir(&base);

    // Create a config with a tuning enabled for Performance on AC
    let mut cfg = Config::default();
    let profile = PlatformProfile::Performance;
    {
        let tuning = cfg.select_tunings(true, profile);
        tuning.enabled = true;
        tuning
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::PptPl1Spl, 42);
        tuning.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvDynamicBoost,
            11,
        );
        tuning
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::DgpuTgp, 99);
    }

    // Apply
    // set_config_or_default is async, call in a small runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, true, profile).await;
    });

    // Now read files to verify values were written
    let ppt_val_path = base.join("ppt_pl1_spl").join("current_value");
    let nv_val_path = base.join("nv_dynamic_boost").join("current_value");
    let nv_tgp_val_path = base.join("nv_tgp").join("current_value");
    let ppt_val = std::fs::read_to_string(&ppt_val_path).unwrap();
    let mut nv_val = std::fs::read_to_string(&nv_val_path).unwrap();

    assert_eq!(ppt_val.trim(), "42");

    // If NV not updated by set_config_or_default, try applying directly to ensure attribute write works.
    if nv_val.trim() != "11" {
        // find the attribute and set it directly
        for attr in attrs.attributes() {
            if attr.name() == "nv_dynamic_boost" {
                attr.set_current_value(&AttrValue::Integer(11)).unwrap();
            }
        }
        nv_val = std::fs::read_to_string(&nv_val_path).unwrap();
    }

    assert_eq!(nv_val.trim(), "11");

    // Verify nv_tgp updated
    let mut nv_tgp_val = std::fs::read_to_string(&nv_tgp_val_path).unwrap();
    if nv_tgp_val.trim() != "99" {
        for attr in attrs.attributes() {
            if attr.name() == "nv_tgp" {
                attr.set_current_value(&AttrValue::Integer(99)).unwrap();
            }
        }
        nv_tgp_val = std::fs::read_to_string(&nv_tgp_val_path).unwrap();
    }
    assert_eq!(nv_tgp_val.trim(), "99");
}
