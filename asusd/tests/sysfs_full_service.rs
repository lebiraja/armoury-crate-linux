use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::tempdir;

use asusd::asus_armoury::start_attributes_zbus;
use asusd::config::Config;
use rog_platform::asus_armoury::FirmwareAttributes;
use rog_platform::platform::PlatformProfile;
use rog_platform::platform::RogPlatform;
use rog_platform::power::AsusPower;
use tokio::runtime::Runtime;
use tokio::sync::Mutex as TokioMutex;

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
fn full_service_handles_boot_sound_and_nv_tgp() {
    let td = tempdir().unwrap();
    let base = td.path().join("attributes");
    create_dir_all(&base).unwrap();

    // create fake attributes (ppt and nv related)
    write_attr_dir(&base, "boot_sound", "0", "boot_sound");
    write_attr_dir(&base, "ppt_pl1_spl", "25", "ppt_pl1_spl");
    write_attr_dir(&base, "ppt_pl2_sppt", "50", "ppt_pl2_sppt");
    write_attr_dir(&base, "ppt_pl3_fppt", "75", "ppt_pl3_fppt");
    write_attr_dir(&base, "ppt_apu_sppt", "20", "ppt_apu_sppt");
    write_attr_dir(&base, "ppt_platform_sppt", "30", "ppt_platform_sppt");
    write_attr_dir(&base, "nv_dynamic_boost", "0", "nv_dynamic_boost");
    write_attr_dir(&base, "nv_temp_target", "0", "nv_temp_target");
    write_attr_dir(&base, "nv_base_tgp", "10", "nv_base_tgp");
    write_attr_dir(&base, "nv_tgp", "0", "nv_tgp");

    // Ensure FirmwareAttributes reads from our fake sysfs
    let attrs = FirmwareAttributes::from_dir(&base);

    // Build config and set nv_tgp tuning for the platform default (Balanced) on AC
    let mut cfg = Config::default();
    let profile = PlatformProfile::Balanced;
    {
        let tuning_ac = cfg.select_tunings(true, profile);
        tuning_ac.enabled = true;
        tuning_ac
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::PptPl1Spl, 42);
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl2Sppt,
            43,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPl3Fppt,
            44,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptApuSppt,
            45,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::PptPlatformSppt,
            46,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvDynamicBoost,
            11,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::NvTempTarget,
            66,
        );
        tuning_ac.group.insert(
            rog_platform::asus_armoury::FirmwareAttribute::DgpuBaseTgp,
            12,
        );
        tuning_ac
            .group
            .insert(rog_platform::asus_armoury::FirmwareAttribute::DgpuTgp, 77);
    }

    // Use default platform/power stubs (they expect to find udev sysfs, so use Defaults)
    let platform = RogPlatform::default();
    let power = AsusPower::default();

    // Start attributes without DBus
    let rt = Runtime::new().unwrap();
    let cfg_arc = Arc::new(TokioMutex::new(cfg));
    let attrs_clone = attrs.clone();
    rt.block_on(async {
        let registry = start_attributes_zbus(
            None,
            platform,
            power,
            attrs_clone,
            cfg_arc.clone(),
            false,
            Some(PlatformProfile::Balanced),
            Some(true),
        )
        .await
        .unwrap();
        // registry now contains AsusArmouryAttribute objects that have been reloaded and applied
        assert!(!registry.is_empty());

        // verify registry contains expected attribute names
        let names: std::collections::HashSet<String> =
            registry.iter().map(|a| a.attribute_name()).collect();

        let expected = [
            "boot_sound", "ppt_pl1_spl", "ppt_pl2_sppt", "ppt_pl3_fppt", "ppt_apu_sppt",
            "ppt_platform_sppt", "nv_dynamic_boost", "nv_temp_target", "nv_base_tgp", "nv_tgp",
        ];

        for &e in &expected {
            assert!(names.contains(e), "Registry missing expected attr: {}", e);
        }

        // Check that PPT and NV attributes current_value exist and were applied
        let nv_tgp_val_path = base.join("nv_tgp").join("current_value");
        let boot_val_path = base.join("boot_sound").join("current_value");
        let ppt1_val_path = base.join("ppt_pl1_spl").join("current_value");
        let ppt2_val_path = base.join("ppt_pl2_sppt").join("current_value");
        let ppt3_val_path = base.join("ppt_pl3_fppt").join("current_value");
        let apu_val_path = base.join("ppt_apu_sppt").join("current_value");
        let plat_val_path = base.join("ppt_platform_sppt").join("current_value");
        let nv_dyn_path = base.join("nv_dynamic_boost").join("current_value");
        let nv_temp_path = base.join("nv_temp_target").join("current_value");
        let nv_base_path = base.join("nv_base_tgp").join("current_value");

        let nv = std::fs::read_to_string(&nv_tgp_val_path).unwrap();
        assert_eq!(nv.trim(), "77");

        // PPTs
        assert_eq!(
            std::fs::read_to_string(&ppt1_val_path).unwrap().trim(),
            "42"
        );
        assert_eq!(
            std::fs::read_to_string(&ppt2_val_path).unwrap().trim(),
            "43"
        );
        assert_eq!(
            std::fs::read_to_string(&ppt3_val_path).unwrap().trim(),
            "44"
        );
        assert_eq!(std::fs::read_to_string(&apu_val_path).unwrap().trim(), "45");
        assert_eq!(
            std::fs::read_to_string(&plat_val_path).unwrap().trim(),
            "46"
        );

        // NVs
        assert_eq!(std::fs::read_to_string(&nv_dyn_path).unwrap().trim(), "11");
        assert_eq!(std::fs::read_to_string(&nv_temp_path).unwrap().trim(), "66");
        assert_eq!(std::fs::read_to_string(&nv_base_path).unwrap().trim(), "12");

        // boot_sound default was 0, it should remain 0 unless config.armoury_settings stored something
        let boot = std::fs::read_to_string(&boot_val_path).unwrap();
        assert_eq!(boot.trim(), "0");
    });
}
