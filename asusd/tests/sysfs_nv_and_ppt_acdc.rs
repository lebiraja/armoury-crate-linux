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
fn nv_dynamic_boost_and_ppt_acdc() {
    let td = tempdir().unwrap();
    let base = td.path().join("attributes");
    create_dir_all(&base).unwrap();

    // create mock attributes: several PPTs and nv_dynamic_boost
    write_attr_dir(&base, "ppt_pl1_spl", "25", "ppt_pl1_spl");
    write_attr_dir(&base, "ppt_pl2_sppt", "50", "ppt_pl2_sppt");
    write_attr_dir(&base, "ppt_pl3_fppt", "75", "ppt_pl3_fppt");
    write_attr_dir(&base, "nv_dynamic_boost", "0", "nv_dynamic_boost");

    let attrs = FirmwareAttributes::from_dir(&base);

    let mut cfg = Config::default();
    let profile = PlatformProfile::Performance;

    // set different values for AC and DC
    {
        let ac = cfg.select_tunings(true, profile);
        ac.enabled = true;
        ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl1Spl,
            100,
        );
        ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl2Sppt,
            101,
        );
        ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl3Fppt,
            102,
        );
        ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvDynamicBoost,
            9,
        );
    }

    {
        let dc = cfg.select_tunings(false, profile);
        dc.enabled = true;
        dc.group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::PptPl1Spl, 10);
        dc.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl2Sppt,
            11,
        );
        dc.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl3Fppt,
            12,
        );
        dc.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvDynamicBoost,
            3,
        );
    }

    let rt = tokio::runtime::Runtime::new().unwrap();

    // apply AC
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, true, profile).await;
    });

    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl1_spl").join("current_value"))
            .unwrap()
            .trim(),
        "100"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl2_sppt").join("current_value"))
            .unwrap()
            .trim(),
        "101"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl3_fppt").join("current_value"))
            .unwrap()
            .trim(),
        "102"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("nv_dynamic_boost").join("current_value"))
            .unwrap()
            .trim(),
        "9"
    );

    // apply DC
    rt.block_on(async {
        set_config_or_default(&attrs, &mut cfg, false, profile).await;
    });

    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl1_spl").join("current_value"))
            .unwrap()
            .trim(),
        "10"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl2_sppt").join("current_value"))
            .unwrap()
            .trim(),
        "11"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("ppt_pl3_fppt").join("current_value"))
            .unwrap()
            .trim(),
        "12"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("nv_dynamic_boost").join("current_value"))
            .unwrap()
            .trim(),
        "3"
    );
}
