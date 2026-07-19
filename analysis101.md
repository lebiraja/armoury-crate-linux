# analysis101.md — Deep Codebase Analysis of `asusctl` / `armoury-crate-linux`

> Version analysed: **v6.3.1** (Rust edition 2021, `rust-version = "1.82"`)  
> Purpose: Complete architectural and implementation reference for rebuilding
> the user-facing front-end as a **Python TUI** without touching the daemon.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Workspace Layout & Crate Dependency Graph](#2-workspace-layout--crate-dependency-graph)
3. [Architecture: Client–Server over D-Bus](#3-architecture-clientserver-over-d-bus)
4. [The System Daemon — `asusd`](#4-the-system-daemon--asusd)
5. [The User Daemon — `asusd-user`](#5-the-user-daemon--asusd-user)
6. [The CLI Client — `asusctl`](#6-the-cli-client--asusctl)
7. [The GUI Application — `armoury-crate-linux`](#7-the-gui-application--armoury-crate-linux)
8. [Legacy GUI — `rog-control-center`](#8-legacy-gui--rog-control-center)
9. [Library Crate Deep Dives](#9-library-crate-deep-dives)
10. [Cross-Cutting Concerns](#10-cross-cutting-concerns)
11. [Hardware Abstraction Layer](#11-hardware-abstraction-layer)
12. [Configuration System](#12-configuration-system)
13. [Concurrency Model](#13-concurrency-model)
14. [D-Bus Object Model (Complete Map)](#14-d-bus-object-model-complete-map)
15. [Kernel & System Dependencies](#15-kernel--system-dependencies)
16. [Known Limitations & Design Decisions](#16-known-limitations--design-decisions)
17. [Python TUI Rewrite Strategy](#17-python-tui-rewrite-strategy)
18. [Glossary](#18-glossary)

---

## 1. Project Overview

**`asusctl`** (also packaged as `armoury-crate-linux`) is a Rust monorepo that
provides Linux-native control over ASUS ROG / TUF / Zephyrus laptop hardware.
Its stated design goals are:

> 1. Provide a safe D-Bus interface over laptop hardware  
> 2. Respect user resources — be small, light, and fast

The suite ships:

| Binary | Role | Process user |
|--------|------|-------------|
| `asusd` | System daemon — **hardware owner** | `root` (via systemd) |
| `asusd-user` | User daemon — custom sequences | login user |
| `asusctl` | CLI client | any user |
| `rog-control-center` | GTK-like GUI (Slint legacy) | any user |
| `armoury-crate-linux` | Modern Slint GUI | any user |

The Python TUI will occupy the **same client role** as `asusctl` and
`armoury-crate-linux` — communicating entirely through D-Bus, never touching
hardware directly.

---

## 2. Workspace Layout & Crate Dependency Graph

```
asusctl/           ← CLI binary (depends on rog-dbus, rog-aura, rog-anime, rog-platform)
asusd/             ← System daemon (depends on ALL rog-* libraries)
asusd-user/        ← User daemon (depends on rog-anime, rog-aura, rog-dbus)
armoury-crate-linux/ ← Modern GUI (depends on rog-dbus, rog-aura, rog-platform)
rog-control-center/  ← Legacy GUI (depends on same as above + GTK/Slint)
rog-aura/          ← RGB keyboard protocol + layout data
rog-anime/         ← AniMe matrix USB protocol + data types
rog-profiles/      ← Fan curve data types and sysfs access
rog-platform/      ← sysfs/udev wrappers: platform profile, power, CPU, backlight
rog-dbus/          ← Generated D-Bus proxy types (client stubs)
rog-slash/         ← Slash LED USB types
rog-scsi/          ← SCSI Aura types
config-traits/     ← Config serialization interfaces
dmi-id/            ← DMI table reader (board name, product family)
simulators/        ← Developer tools: SDL2 AniMe/Aura simulators
```

### Key External Dependencies (workspace-level)

| Crate | Version | Purpose |
|-------|---------|---------|
| `zbus` | 5.13.1 | D-Bus client + server (async, safe Rust) |
| `tokio` | ^1.39 | Async runtime |
| `serde` + `ron` | latest | Serialization (config files) |
| `udev` | ^0.8 | Hardware discovery / hotplug |
| `inotify` | ^0.10 | filesystem watch for config/sysfs |
| `logind-zbus` | 5.2.0 | Login session events (suspend/resume) |
| `nvml_wrapper` | (opt.) | NVIDIA GPU metrics |
| `sysinfo` | — | CPU/RAM/process info |
| `ksni` | — | System tray (KDE-compatible) |
| `slint` | — | GUI framework (declarative `.slint` files) |
| `notify-rust` | 4.11.5 | Desktop notifications |
| `supergfxctl` | (opt.) | GPU mode management (external daemon) |

---

## 3. Architecture: Client–Server over D-Bus

```
┌───────────────────────────────────────────────────────┐
│                    SYSTEM BUS                         │
│                 xyz.ljones.Asusd                      │
│                                                       │
│  ┌─────────────────────────────────────────────────┐  │
│  │  asusd  (root, systemd service)                 │  │
│  │  ┌─────────────┐  ┌──────────┐  ┌───────────┐  │  │
│  │  │CtrlPlatform │  │AuraZbus  │  │FanCurves  │  │  │
│  │  │ (sysfs/ACPI)│  │(USB HID) │  │(hwmon)    │  │  │
│  │  └─────────────┘  └──────────┘  └───────────┘  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │  │
│  │  │AnimeZbus │  │SlashZbus │  │AsusArmoury    │  │  │
│  │  │(USB HID) │  │(USB HID) │  │(ACPI firmware)│  │  │
│  │  └──────────┘  └──────────┘  └───────────────┘  │  │
│  └─────────────────────────────────────────────────┘  │
│                          ▲                            │
│           D-Bus method calls & property signals       │
│                          │                            │
│  ┌─────────┐  ┌──────────────────────┐  ┌─────────┐  │
│  │asusctl  │  │armoury-crate-linux   │  │Python   │  │
│  │  (CLI)  │  │  (Slint GUI)         │  │  TUI    │  │
│  └─────────┘  └──────────────────────┘  └─────────┘  │
└───────────────────────────────────────────────────────┘

              SESSION BUS
         xyz.ljones.AsusdUser
  ┌────────────────────────────────┐
  │  asusd-user (user session)     │
  │  ┌────────────────────────┐    │
  │  │ AnimeUser (sequences)  │    │
  │  └────────────────────────┘    │
  └────────────────────────────────┘
```

**This is the critical insight for the Python TUI:**  
The new TUI is **purely a D-Bus client**. Zero hardware access. Zero root.
All hardware mutations go through `asusd`'s safe interface.

---

## 4. The System Daemon — `asusd`

### 4.1 Startup Sequence (`asusd/src/daemon.rs`)

```
main()
 └── start_daemon()
      ├── print_board_info()           # DMI board/product info
      ├── Connection::system()         # Connect to D-Bus system bus
      ├── ObjectManager::at("/")       # Register root object manager
      ├── Config::new().load()         # Load /etc/asusd/asusd.ron
      ├── RogPlatform::new()           # sysfs platform abstraction
      ├── AsusPower::new()             # power_supply sysfs
      ├── FirmwareAttributes::new()    # asus-armoury kernel attrs
      ├── start_attributes_zbus()      # Register all firmware attrs on D-Bus
      ├── CtrlFanCurveZbus::new()      # Fan curve controller
      ├── CtrlBacklight::new()         # Screenpad backlight
      ├── CtrlPlatform::new()          # Platform profiles + PPD
      ├── DeviceManager::new()         # USB HID device discovery (udev)
      └── connection.request_name("xyz.ljones.Asusd")
```

After startup, the daemon enters an infinite `executor.tick()` loop —
all work is event-driven via async tasks.

### 4.2 Controller Trait System

Every hardware subsystem implements one or more of:

```rust
trait ZbusRun {
    async fn add_to_server(&self, server: &mut Connection);
}
trait CtrlTask {
    async fn create_tasks(&self, signal_ctxt: SignalEmitter) -> Result<()>;
}
trait Reloadable {
    async fn reload(&mut self, config: Config);
}
trait ReloadAndNotify {
    async fn reload_and_notify(&mut self, signal: &SignalEmitter, config: Config);
}
```

This is a **strategy pattern** that allows the startup code to generically
call `start_tasks(ctrl, &mut server, sig_ctx)` for any controller.

### 4.3 `CtrlPlatform` — Largest Controller

Manages: platform profile, battery charge, AC/battery detection, CPU EPP/governor.

**Internal state:**
- `power: AsusPower` — sysfs power supply abstraction
- `platform: RogPlatform` — sysfs platform_profile + GPU mode + backlight toggles
- `attributes: FirmwareAttributes` — asus-armoury ACPI attribute set
- `cpu_control: Option<CPUControl>` — `/sys/devices/system/cpu/` governor
- `config: Arc<Mutex<Config>>` — shared daemon config
- `armoury_registry: ArmouryAttributeRegistry` — per-attribute D-Bus delegates

**Config file watch:** Uses inotify with a workaround for vim's two-stage
write that would otherwise break the kernel's inotify watch on the file.

### 4.4 `DeviceManager` — Dynamic Device Lifecycle

```
DeviceManager::new(connection)
 └── init_hid_devices()          # scan current USB HID devices
 └── udev monitor thread         # watch for add/remove events
      ├── on add:  init_hid_devices() → add zbus path
      └── on remove: remove zbus path
```

Each USB device is identified by `(idVendor=0b05, idProduct=XXXX)`.
The product ID determines which handler is used:

| Product IDs | Handler |
|-------------|---------|
| N-KEY HID (`1866`, `19b6`, etc.) | `AuraZbus` (keyboard RGB) |
| Slash device | `SlashZbus` |
| AniMe device | `AniMeZbus` |
| SCSI Aura device | `ScsiZbus` |
| TUF I2C keyboard | `AuraZbus` (TUF variant) |

HID file handles (`/dev/hidraw*`) are shared via `Arc<Mutex<HidRaw>>`
to prevent multiple opens of the same device node.

### 4.5 `AsusArmouryAttribute` — Per-Attribute D-Bus Object

Each ACPI firmware attribute (e.g., `ppt_pl1_spl`) becomes its own D-Bus
object at `/xyz/ljones/asus_armoury/<attr_name>`.

Each object exposes: `name`, `current_value`, `set_current_value`, `min_value`,
`max_value`, `default_value`, `scalar_increment`, `attribute_type`, `possible_values`.

inotify watches the sysfs attribute file for external changes (e.g., another
tool writing to it) and emits `current_value_changed` signals automatically.

### 4.6 `CtrlFanCurveZbus` — Fan Curve Data Flow

```
udev scan → find hwmon with name="asus_custom_fan_curve"
         → read current curves from pwm*/temp* attributes
         → store in FanCurveProfiles { balanced, performance, quiet, custom }
         → expose via D-Bus
         → on set_fan_curve(): write to sysfs + store config
         → on profile change: load and apply saved curve for new profile
```

### 4.7 Config Lifecycle

```
/etc/asusd/asusd.ron
       │
       ├── Read on daemon start (StdConfigLoad2 trait)
       ├── Written back on any change (StdConfig::write)
       └── inotify watch → reload on external edit
```

Config uses RON (Rusty Object Notation), a Rust-native data format similar to
JSON but with explicit types and enum support.

---

## 5. The User Daemon — `asusd-user`

Runs under the user's login session (systemd user service).

### 5.1 Purpose

- Execute custom per-key RGB sequences the system daemon cannot do (requires
  user session context and `~/.config` access)
- Play AniMe matrix sequences from user-defined animation files

### 5.2 Config Structure

```
~/.config/rog/
├── rog-user.cfg          ← main config: active_aura, active_anime, led_type
├── aura-default.ron      ← per-key/zoned Aura effect definition
└── anime-default.ron     ← AniMe sequence definition
```

### 5.3 `led_type` Options

| Value | Description |
|-------|-------------|
| `Key` | Per-key RGB keyboard |
| `Zone` | Multizone keyboard |
| `None` | Zoned/unzoned fallback (TUF compatible) |

### 5.4 Custom Aura Effect RON Format

```ron
(
    name: "aura-default",
    aura: (
        effects: [
            Breathe(( led: W, start_colour1: (255,0,20), start_colour2: (20,255,0), speed: Low )),
            Static(( led: RCtrl, colour: (0,0,255) )),
            DoomFlicker(( led: N9, start_colour: (0,0,255), max_percentage: 80, min_percentage: 40 )),
        ],
        zoned: false,
    ),
)
```

Available effect types: `Static`, `Breathe`, `DoomFlicker`, `Comet`, `Flash`,
`Ripple`, `Rainbow`, `Strobe`, `Rain`, `Highlight`, `Laser`, `Pulse`, `Star`.

---

## 6. The CLI Client — `asusctl`

### 6.1 Startup Flow

```
main()
 ├── PlatformProxyBlocking::new()     # Connect & verify asusd is running
 ├── version check                    # Abort on version mismatch
 ├── supported_properties()           # Cache supported platform properties
 ├── list_iface_blocking()            # Cache available D-Bus interfaces
 └── do_parsed(CliStart)              # Dispatch to handler
```

Uses `argh` for argument parsing — a zero-allocation, derive-macro CLI parser.

### 6.2 Command Dispatch Tree

```
CliCommand
├── Aura(LedModeCommand)
│   ├── --next-mode
│   ├── --prev-mode
│   └── <SetAuraBuiltin subcommand>   # Static/Breathe/etc with colour params
├── AuraPowerOld(LedPowerCommand1)
├── AuraPower(LedPowerCommand2)
├── Brightness(BrightnessCommand)
├── Profile(ProfileCommand)
│   ├── next
│   ├── list
│   ├── get
│   └── set <profile> [-a] [-b]
├── FanCurve(FanCurveCommand)
├── Anime(AnimeCommand)
├── Slash(SlashCommand)
├── Scsi(ScsiCommand)
├── Armoury(ArmouryCommand)
│   ├── list
│   ├── get <property>
│   └── set <property> <value>
├── Backlight(BacklightCommand)
├── Battery(BatteryCommand)
│   ├── limit <percent>
│   ├── oneshot [percent]
│   └── info
└── Info(InfoCommand)
    └── --show-supported
```

### 6.3 Interface Discovery (`find_iface<T>`)

```rust
fn find_iface<T>(iface_name: &str) -> Result<Vec<T>, ...> {
    // 1. ObjectManagerProxy::new(conn, "xyz.ljones.Asusd", "/")
    // 2. iterate get_managed_objects()
    // 3. filter by interface name
    // 4. build T::builder().path(path).build()
    // returns Vec because multiple devices of same type are possible
}
```

This pattern is replicated in Python as shown in Section 15.3 of features101.md.

---

## 7. The GUI Application — `armoury-crate-linux`

The newer of the two GUI apps. Built on **Slint** (a declarative UI framework
for embedded and desktop Rust, with `.slint` template files).

### 7.1 Entry Point and Init Sequence

```
main()
 ├── gamescope detection (ROG Ally: GAMESCOPE_WAYLAND_DISPLAY → WAYLAND_DISPLAY)
 ├── CLI args (--fullscreen, --version, --help)
 ├── DMI info read
 ├── asusd version check
 ├── Config::new().load()                  # ~/.config/rog/armoury-crate-linux.cfg
 ├── SystemMonitor::new(interval, history) # background monitoring struct
 ├── monitor.start()                       # spawned tokio task
 ├── ScenarioManager::new(config_path)     # process-watcher
 ├── scenario_clone.start_monitoring(cb)   # spawned tokio task
 ├── MainWindow::new()                     # Slint window creation
 ├── ui::setup_all_pages(&ui)              # D-Bus reads → populate initial state
 ├── ui::setup_all_callbacks(&ui)          # wire UI events → D-Bus write calls
 ├── ui::setup_dashboard_page(&ui, monitor)
 ├── ui::setup_scenario_page(&ui, scenario_manager)
 ├── ui::setup_settings_page(&ui, config)
 ├── ui.show()
 └── slint::run_event_loop()               # blocks until window close
```

### 7.2 Page Module Map (`armoury-crate-linux/src/ui/`)

| Source file | Page | Key D-Bus interfaces used |
|-------------|------|--------------------------|
| `setup_dashboard.rs` | Dashboard | none (sysinfo/hwmon directly) |
| `setup_aura.rs` | Aura RGB | `xyz.ljones.Aura` |
| `setup_fans.rs` | Fan Curves | `xyz.ljones.FanCurves` |
| `setup_system.rs` | System | `xyz.ljones.Platform`, `xyz.ljones.AsusArmoury` |
| `setup_anime.rs` | AniMe | `xyz.ljones.Anime` |
| `setup_scenario.rs` | Scenario Profiles | local `ScenarioManager` |
| `setup_settings.rs` | Settings | local `Config` |

### 7.3 Interface Discovery Strategy (multi-fallback)

The `find_aura_iface()` function demonstrates the robust discovery pattern:

```
Attempt 1: ObjectManager at "/"
  → iterate managed objects for "xyz.ljones.Aura"

Attempt 2: ObjectManager at "/xyz/ljones/aura"
  → same iteration at sub-path

Attempt 3: Introspectable at "/xyz/ljones/aura"
  → parse XML, find <node name="..."> children
  → construct path "/xyz/ljones/aura/{child}"
  → probe brightness() to confirm liveness
```

The Python TUI should implement the same three-attempt pattern.

### 7.4 UI Data Flow — Slint Global Objects

Slint uses `global` data objects to pass state between Rust and `.slint` templates:

| Global | Properties exposed |
|--------|--------------------|
| `DashboardData` | cpu_usage, cpu_temp, cpu_freq_ghz, gpu_usage, gpu_temp, gpu_power_watts, ram_usage_percent, ram_total_gb, fan1_rpm, fan2_rpm, battery_percent, on_ac_power, cpu_temp_history, gpu_temp_history |
| `AuraPageData` | brightness, current_mode, supported_modes, aura_available, is_loading, error_message |
| `FansPageData` | fans_available, selected_profile, selected_fan, cpu_fan_curve_balanced/performance/quiet, gpu_fan_curve_balanced/performance/quiet, cpu_fan_enabled, gpu_fan_enabled |
| `SystemPageData` | platform_profile, charge_control_end_threshold, panel_od, gpu_mux_mode, ppt_pl1_spl, ppt_pl2_sppt, nv_dynamic_boost, nv_temp_target |
| `AnimePageData` | brightness, builtins_enabled, enable_display, off_when_lid_closed, off_when_suspended, off_when_unplugged, boot_anim, awake_anim, sleep_anim, shutdown_anim |

### 7.5 Monitoring Architecture (`monitoring/mod.rs`)

```
SystemMonitor {
    data:    Arc<RwLock<MonitoringData>>     # latest snapshot
    history: Arc<RwLock<MonitoringHistory>>  # ring buffer
    hwmon:   HwmonPaths                      # cached discovered paths
    system:  Arc<Mutex<sysinfo::System>>     # CPU/RAM
    #[cfg(nvidia)] NvidiaMonitor             # NVML handle
}
```

`start()` method spawns a tokio task that:
1. Calls `sysinfo.refresh_all()`
2. Reads CPU temp from discovered hwmon path
3. Reads GPU temp/usage from hwmon (AMD) or NVML (NVIDIA)
4. Reads fan RPMs from hwmon `fan*_input`
5. Reads battery from `/sys/class/power_supply`
6. Acquires `RwLock::write()` and atomically updates both `data` and `history`
7. Sleeps for `update_interval_ms`

UI reads via `get_data().await` and `get_history().await` (read locks).

### 7.6 Scenario Manager (`scenario_manager.rs`)

```
ScenarioManager {
    config:      Arc<RwLock<ScenarioConfig>>      # rules + settings
    config_path: PathBuf                          # ~/.config/armoury-crate-linux/scenarios.toml
    active_rule: Arc<RwLock<Option<String>>>      # currently matched rule ID
}
```

`check_and_apply()` algorithm:
```
1. Lock config (read)
2. get_running_processes() → Vec<String from /proc/*/cmdline>
3. sort rules by priority DESC
4. for rule in rules:
     if any(process.contains(rule.process_name)):
         if active_rule != rule.id:
             set active_rule = rule.id
             return Some(rule)  ← caller applies via D-Bus
         return None  ← already active, no action
5. if active_rule is Some:   ← had a rule, none match now
     active_rule = None
     return Some(default_profile_rule)
6. return None
```

### 7.7 Notification System (`notify.rs`)

Two independent monitors run in parallel:
1. **AC/BAT watcher** — `spawn_blocking` thread polling `AsusPower::get_online()`
   every 500 ms; runs `ac_command` or `bat_command` shell string on change.
2. **dGPU status watcher** — plain thread polling `supergfxctl` device
   `get_runtime_status()` every 1500 ms; sends desktop notification on change.
3. **Async D-Bus signal listeners** — `receive_*_changed()` streams on zbus
   properties for profile change, GPU mode change.

### 7.8 System Tray (`tray.rs`)

Uses `ksni` crate which implements the `StatusNotifierItem` specification.
Icons are read from `/usr/share/icons/hicolor/512x512/apps/` at startup.
Menu callbacks communicate back to the main window via a private
`ROGCCZbusProxyBlocking` (internal D-Bus interface between GUI components).

---

## 8. Legacy GUI — `rog-control-center`

The older GUI application (still in the workspace). Built on the same
Slint + zbus stack but with different page organisation. Supports X11
optionally (`cargo build --features "rog-control-center/x11"`).

The `armoury-crate-linux` crate is its modernised replacement with:
- Unified monitoring architecture (vs. scattered ad-hoc polls)
- Better separation between UI setup and D-Bus callbacks
- Scenario profile engine
- ROG Ally / Gamescope fullscreen mode

Both GUIs use the same D-Bus API — the Python TUI can use the same interface.

---

## 9. Library Crate Deep Dives

### 9.1 `rog-aura`

**Purpose:** USB protocol encoding + layout data for ASUS RGB keyboards.

**Key types:**

| Type | Description |
|------|-------------|
| `AuraEffect` | A single hardware effect with mode + colour + speed |
| `AuraModeNum` | `u8` enum: Static=0, Breathe=1, ... Flash=12 |
| `LedBrightness` | Off=0, Low=1, Med=2, High=3 |
| `AuraDeviceType` | Pre2021 / 2021+ / TUF enum |
| `PowerZones` | Zone power state matrix |
| `AuraPowerState` | Per-zone per-state (boot/awake/sleep/shutdown) |

**Layout system:**
- `aura_support.ron` — database mapping `board_name` → supported modes/zones/advanced_type
- `<layout_name>_<locale>.ron` — physical key positions for per-key RGB editor
- 80+ laptops supported as of 2023

**Encoding flow (internal to daemon, irrelevant to Python TUI):**
```
AuraEffect → usb_aura_effect() → [u8; 17] USB HID packet → HidRaw::write()
```

### 9.2 `rog-anime`

**Purpose:** AniMe matrix display control.

**Supported models:** GA401, GA402, GU604, G635L, G835L (DMI-detected).

**Key types:**

| Type | Description |
|------|-------------|
| `AnimeType` | Model enum, detected from DMI board name |
| `AnimeDataBuffer` | Byte matrix of pixel values ready for USB |
| `AnimeImage` | PNG → scaled pixel buffer |
| `AnimeGif` | GIF → sequence of `AnimeDataBuffer` frames |
| `AnimeDiagonal` | Diagonal pattern generator |
| `Animations` | `{ boot, awake, sleep, shutdown }` builtin enum values |
| `DeviceState` | All display settings snapshot |

USB packet structure:
- GA401: 2 packets with `USB_PREFIX1` + `USB_PREFIX2` (1245 pixels)
- GA402: 3 packets adding `USB_PREFIX3` (larger matrix)
- Raw pixel data starts at byte offset 7 (`BLOCK_START`)

### 9.3 `rog-profiles`

**Purpose:** Fan curve data types and sysfs access.

**Key types:**

| Type | Description |
|------|-------------|
| `FanCurvePU` | CPU=0, GPU=1, MID=2 |
| `CurveData` | `fan: FanCurvePU`, `pwm: [u8;8]`, `temp: [u8;8]`, `enabled: bool` |
| `FanCurveProfiles` | `{ balanced, performance, quiet, custom }` each `Vec<CurveData>` |

Discovery: udev scan for hwmon with `name` attribute == `"asus_custom_fan_curve"`.  
Fan detection: probe `pwm1_enable`, `pwm2_enable`, `pwm3_enable` presence.

### 9.4 `rog-platform`

**Purpose:** Sysfs/udev wrappers for non-HID hardware.

**Sub-modules:**

| Module | Wraps |
|--------|-------|
| `platform` | `/sys/devices/platform/asus-*` (RogPlatform) |
| `power` | `/sys/class/power_supply/` (AsusPower) |
| `cpu` | `/sys/devices/system/cpu/` governor + EPP |
| `backlight` | `/sys/class/backlight/` screenpad |
| `asus_armoury` | `/sys/class/firmware-attributes/asus-armoury-*/` |
| `hid_raw` | `/dev/hidraw*` USB HID raw I/O |
| `keyboard_led` | `/sys/class/leds/` brightness |
| `usb_raw` | libusb raw USB I/O |

`RogPlatform` checks for sysfs attribute existence before exposing a `has_*()` guard,
preventing D-Bus exposure of unsupported features on non-ROG hardware.

### 9.5 `rog-dbus`

**Purpose:** Generated D-Bus proxy types for client code.

The proxies are generated from the D-Bus interface (or written by hand) using
`zbus` derive macros.

**Client proxy types:**

| Proxy | Interface |
|-------|-----------|
| `PlatformProxyBlocking` / `PlatformProxy` | `xyz.ljones.Platform` |
| `AuraProxyBlocking` / `AuraProxy` | `xyz.ljones.Aura` |
| `FanCurvesProxyBlocking` / `FanCurvesProxy` | `xyz.ljones.FanCurves` |
| `AnimeProxyBlocking` / `AnimeProxy` | `xyz.ljones.Anime` |
| `SlashProxyBlocking` / `SlashProxy` | `xyz.ljones.Slash` |
| `BacklightProxyBlocking` / `BacklightProxy` | `xyz.ljones.Backlight` |
| `AsusArmouryProxyBlocking` / `AsusArmouryProxy` | `xyz.ljones.AsusArmoury` |

Blocking variants are used for `asusctl` CLI (synchronous); async variants
for GUI/daemon (tokio).

### 9.6 `config-traits`

Defines `StdConfig` and two loading variants:

```rust
trait StdConfig {
    fn new() -> Self;
    fn file_name(&self) -> String;
    fn config_dir() -> PathBuf;
    fn write(&self);           // RON serialization to config_dir()/file_name()
    fn file_path(&self) -> PathBuf;
}
trait StdConfigLoad1<T>: StdConfig {
    fn load(self) -> T;        // deserialize, fallback to default on error
}
trait StdConfigLoad2<T, T2>: StdConfig {
    fn load(self) -> T;        // supports two format versions
}
```

Python equivalent: `tomllib.loads(path.read_text())` with `dataclasses` or
`pydantic` models as the schema.

### 9.7 `dmi-id`

Reads `/sys/devices/virtual/dmi/id/{board_name, product_family, ...}`.

Used for:
- AniMe type detection (GA401 vs GA402 etc.)
- `asusctl info` board display
- Version mismatch display

Python equivalent: `pathlib.Path("/sys/devices/virtual/dmi/id/board_name").read_text().strip()`

### 9.8 `simulators`

SDL2-based visual simulators for AniMe matrix and Aura keyboards.
**Not needed for TUI rebuild.** Used only by developers testing animations
without physical hardware.

---

## 10. Cross-Cutting Concerns

### 10.1 inotify-Based Reactivity

Both the daemon and GUI use inotify to react to _external_ sysfs/file changes:

- `asusd` watches `/etc/asusd/asusd.ron` → reload config if edited externally
- `AsusArmouryAttribute` watches individual sysfs `current_value` files →
  emit D-Bus `current_value_changed` signal when another tool modifies TDP
- `CtrlPlatform` watches `platform_profile` sysfs → signal GUI on profile change

**Python TUI implication:** Subscribe to D-Bus signals instead of polling —
changes made via CLI, kernel driver, or another tool are visible instantly.

### 10.2 Error Handling Strategy

The Rust code uses a custom `RogError` enum wrapping `zbus::fdo::Error` for all
D-Bus interfaces. Errors are:
- Logged via `log::warn!` / `log::error!`
- Returned as `Err(FdoError::NotSupported(...))` when a feature is absent
- Shown as toast notifications in the GUI

Python TUI should:
- Catch `dbus.exceptions.DBusException` for all D-Bus calls
- Show error in a status bar or modal
- Treat `org.freedesktop.DBus.Error.ServiceUnknown` as "asusd not running"

### 10.3 Version Compatibility

Both `asusctl` and `armoury-crate-linux` call `platform.version()` on startup
and abort/warn if it differs from their own version. This prevents silent
breakage from API changes between versions.

The Python TUI must do the same:
```python
asusd_ver = platform_iface.Get("xyz.ljones.Platform", "version")
if asusd_ver != TUI_VERSION:
    show_warning(f"Version mismatch: TUI={TUI_VERSION}, asusd={asusd_ver}")
```

### 10.4 Wayland / Gamescope Support

The `armoury-crate-linux` app detects `GAMESCOPE_WAYLAND_DISPLAY` and
redirects `WAYLAND_DISPLAY` to it, enabling use on the ROG Ally handheld
running SteamOS + Gamescope compositor.

X11 is explicitly **not** supported.

---

## 11. Hardware Abstraction Layer

### 11.1 sysfs Paths Used

| Feature | sysfs path |
|---------|-----------|
| Platform profile | `/sys/devices/platform/asus-*/platform_profile` |
| Battery limit | `/sys/class/power_supply/BAT*/charge_control_end_threshold` |
| Charge online | `/sys/class/power_supply/AC*/online` |
| CPU governor | `/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor` |
| CPU EPP | `/sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference` |
| Fan RPM | `/sys/class/hwmon/hwmon*/fan[1-3]_input` |
| Fan curve | `/sys/class/hwmon/<asus_custom_fan_curve>/pwm[1-3]*` |
| CPU temp (AMD) | `/sys/class/hwmon/<k10temp>/temp1_input` |
| CPU temp (Intel) | `/sys/class/hwmon/<coretemp>/temp1_input` |
| GPU temp (AMD) | `/sys/class/hwmon/<amdgpu>/temp1_input` |
| GPU busy (AMD) | `/sys/class/hwmon/<amdgpu>/device/gpu_busy_percent` |
| Firmware attrs | `/sys/class/firmware-attributes/asus-armoury-*/` |
| Screenpad | `/sys/class/backlight/asus_screenpad/` |
| LED brightness | `/sys/class/leds/asus::kbd_backlight/brightness` |

### 11.2 USB HID Device Protocol

Handled **exclusively by `asusd`** — the Python TUI never touches USB.

Internal flow (for reference only):
```
USB HID packet → /dev/hidrawN → opened as HidRaw
Each effect = 17-byte packet with specific prefix bytes
Multiple packets per update transaction
```

### 11.3 ACPI Firmware Attributes

Located at `/sys/class/firmware-attributes/asus-armoury-*/`.
Requires **Linux 6.19+** for mainline support (or patched earlier kernels).

Structure per attribute:
```
/sys/class/firmware-attributes/asus-armoury-*/
├── name              ← R: "ppt_pl1_spl"
├── current_value     ← R/W: "30"
├── default_value     ← R: "35"
├── min_value         ← R: "5"
├── max_value         ← R: "54"
└── scalar_increment  ← R: "1"
```

The daemon reads/writes these files via `udev::Device::set_attribute_value()`.

---

## 12. Configuration System

### 12.1 Format: RON (Rusty Object Notation)

All daemon configs use RON. Example `/etc/asusd/asusd.ron`:
```ron
Config(
    bat_charge_limit: 80,
    base_charge_control_end_threshold: 80,
    panel_od: false,
    ac_profile: Performance,
    bat_profile: Balanced,
)
```

The Python TUI does **not** parse RON — it uses D-Bus only.
However, RON parsers exist in Python (`python-ron`) if ever needed.

### 12.2 Format: TOML

`armoury-crate-linux` uses TOML for its own config (Python-friendly).
The scenario manager also uses TOML.

### 12.3 Config Loading with Graceful Migration (`StdConfigLoad2`)

The daemon supports two config versions to avoid breaking upgrades.
If `v2` parsing fails, it falls back to `v1`, then defaults.

Python equivalent: try loading with current schema, on `KeyError` or
`ValidationError` fall back to defaults and save a fresh config.

---

## 13. Concurrency Model

### 13.1 Rust (asusctl / asusd)

- **Runtime:** `tokio` with `rt-multi-thread` feature
- **D-Bus:** `zbus` (fully async, uses tokio under the hood)
- **Shared state:** `Arc<Mutex<T>>` for configs, `Arc<RwLock<T>>` for
  monitoring data (many readers, infrequent writers)
- **Long-running tasks:** `tokio::spawn(async move { ... })` — not threads
- **Blocking I/O** (e.g., fan RPM reads): kept minimal; most sysfs reads
  are fast enough to do inline in async context
- **Signal propagation:** `SignalEmitter` shared via `Arc` into watcher tasks

### 13.2 Implications for Python TUI

| Rust concept | Python equivalent |
|-------------|------------------|
| `tokio::spawn` | `asyncio.create_task()` |
| `Arc<RwLock<T>>` | `asyncio.Lock()` protecting a shared object |
| `Arc<Mutex<T>>` | `threading.Lock()` (if threading) or `asyncio.Lock()` |
| `zbus` async | `dbus-fast` async or `dbus-next` |
| `tokio::time::interval` | `asyncio` + `asyncio.sleep()` loop |
| `inotify` | `watchdog` library or `asyncio` + inotify fd |
| D-Bus signals | `dbus-fast` signal match rules |

Textual natively runs on an asyncio event loop — all D-Bus I/O and monitoring
should be async tasks that post updates to the TUI via Textual's
`call_from_thread()` or worker patterns.

---

## 14. D-Bus Object Model (Complete Map)

```
xyz.ljones.Asusd  (system bus)
│
├── /                                    [ObjectManager]
│
├── /xyz/ljones                          [xyz.ljones.Platform]
│   Properties: version, platform_profile, platform_profile_choices,
│               charge_control_end_threshold, supported_properties, ...
│   Methods: set_platform_profile(), set_charge_control_end_threshold(),
│            one_shot_full_charge(), supported_properties()
│   Signals: platform_profile_changed, charge_control_end_threshold_changed
│
├── /xyz/ljones/aura/
│   ├── <device_id>                      [xyz.ljones.Aura]
│   │   Properties: brightness, current_mode, supported_basic_modes,
│   │               supported_basic_zones, supported_power_zones, device_type
│   │   Methods: set_mode(), set_brightness(), next_aura_mode(), prev_aura_mode()
│   │   Signals: brightness_changed, current_mode_changed
│   │
│   ├── tuf                              [xyz.ljones.Aura]  (TUF laptops)
│   ├── anime                            [xyz.ljones.Anime]
│   │   Properties: brightness, enable_display, builtins_enabled,
│   │               off_when_lid_closed, off_when_suspended, off_when_unplugged,
│   │               builtin_animations, brightness_on_battery
│   │   Methods: set_builtin_animations()
│   │
│   └── slash                            [xyz.ljones.Slash]
│       Properties: slash_mode, brightness, speed
│
├── /xyz/ljones/backlight/<device_id>    [xyz.ljones.Backlight]
│   Properties: screenpad_brightness, screenpad_gamma,
│               screenpad_sync_with_primary
│
├── /xyz/ljones/asus_armoury/
│   ├── ppt_pl1_spl                      [xyz.ljones.AsusArmoury]
│   ├── ppt_pl2_sppt                     [xyz.ljones.AsusArmoury]
│   ├── ppt_apu_sppt                     [xyz.ljones.AsusArmoury]
│   ├── nv_dynamic_boost                 [xyz.ljones.AsusArmoury]
│   ├── nv_temp_target                   [xyz.ljones.AsusArmoury]
│   ├── panel_od                         [xyz.ljones.AsusArmoury]
│   ├── gpu_mux_mode                     [xyz.ljones.AsusArmoury]
│   ├── boot_sound                       [xyz.ljones.AsusArmoury]
│   └── ...                              (all present attrs)
│
└── /xyz/ljones                          [xyz.ljones.FanCurves]
    Properties: (accessed via methods)
    Methods: fan_curve_data(), set_fan_curve(), reset_profile_curves()
```

---

## 15. Kernel & System Dependencies

### 15.1 Required Kernel Modules

| Module | Feature | Kernel version |
|--------|---------|---------------|
| `asus-wmi` | Platform, GPU MUX, battery limit | mainline |
| `asus-nb-wmi` | ACPI hotkeys, fan control | mainline |
| `asus-armoury` | Firmware attributes (TDP etc.) | 6.19+ |
| `platform_profile` | Profile switching interface | 5.15+ |
| `k10temp` / `zenpower` | AMD CPU temp | mainline |
| `coretemp` | Intel CPU temp | mainline |
| `amdgpu` | AMD GPU | mainline |
| `nvidia` | NVIDIA GPU (proprietary) | vendor |

### 15.2 System Services Required

| Service | Purpose |
|---------|---------|
| `asusd.service` | **Required** — hardware control daemon |
| `power-profiles-daemon` | Optional — alternative profile backend |
| `supergfxctl` | Optional — GPU switching notifications |
| `systemd-logind` | Suspend/resume events to daemon |

### 15.3 User Permissions

The Python TUI runs as a normal user. Required group/polkit:
- `input` group: not needed (D-Bus handles security)
- D-Bus policy (`/etc/dbus-1/system.d/asusd.conf`): grants `xyz.ljones.Asusd`
  access to all users

---

## 16. Known Limitations & Design Decisions

### 16.1 Wayland Only (GUI)
X11 explicitly unsupported in `armoury-crate-linux`.
Python TUI running in a terminal works on both X11 and Wayland.

### 16.2 Per-Key RGB in TUI
Visual per-key editor is very complex in a terminal.
Recommended TUI approach: list available `~/.config/rog/*.ron` profiles,
let the user select one, apply via `asusd-user` D-Bus.

### 16.3 Fan Curve Editor
The Slint GUI uses a drag-point `Node` widget (custom Slint component).
In a TUI, use an ASCII step-chart with arrow-key navigation on 8 editable points.

### 16.4 Firmware Attributes Kernel Version
`asus-armoury` attributes require Linux 6.19+. On older kernels the TDP
sliders will not appear (the D-Bus objects simply won't exist).
The TUI should handle this gracefully: check via ObjectManager, show
"Not available on this kernel" if absent.

### 16.5 Multiple Aura Devices
A laptop may have multiple Aura devices (keyboard + external peripheral).
The code uses `Vec<AuraProxyBlocking>` from `find_iface()`.
The Python TUI should enumerate and display all, or let the user select.

### 16.6 `supergfxctl` Optional Dependency
GPU mode switching (`armoury-crate-linux` tray icon colouring, notifications)
requires the separate `supergfxctl` daemon. If not installed, those features
are simply absent — no crash.

---

## 17. Python TUI Rewrite Strategy

### 17.1 What to Rewrite

**Rewrite:** The visual front-end only:
- Dashboard display
- Settings forms
- Fan curve editor
- Aura mode selector
- All D-Bus interaction logic

**Keep (do not touch):**
- `asusd` system daemon
- `asusd-user` user daemon
- All `.ron` config files
- All kernel modules / sysfs paths

### 17.2 Recommended Stack

```
textual          ≥ 0.50    # TUI framework, asyncio-native, CSS layout
dbus-fast        ≥ 2.21    # async D-Bus, no GLib dependency
psutil           ≥ 5.9     # CPU/RAM/disk/process monitoring
pynvml           ≥ 11.5    # NVIDIA GPU metrics (optional)
tomllib          stdlib    # read TOML configs (Python 3.11+)
tomli-w          ≥ 1.0     # write TOML configs
watchdog         ≥ 3.0     # inotify for config file watching (optional)
```

### 17.3 Proposed TUI Module Structure

```
asus_tui/
├── main.py                  ← App entry point, DI container
├── dbus/
│   ├── client.py            ← Connection, ObjectManager discovery
│   ├── platform.py          ← xyz.ljones.Platform proxy
│   ├── aura.py              ← xyz.ljones.Aura proxy
│   ├── fan_curves.py        ← xyz.ljones.FanCurves proxy
│   ├── anime.py             ← xyz.ljones.Anime proxy
│   ├── slash.py             ← xyz.ljones.Slash proxy
│   ├── backlight.py         ← xyz.ljones.Backlight proxy
│   └── armoury.py           ← xyz.ljones.AsusArmoury proxy
├── monitoring/
│   ├── system.py            ← sysinfo (CPU/RAM/disk via psutil)
│   ├── hwmon.py             ← hwmon discovery + temp/fan reads
│   ├── nvidia.py            ← pynvml wrapper
│   └── history.py           ← ring buffer for graphs
├── scenario/
│   ├── manager.py           ← rule engine, process scanner
│   └── models.py            ← ScenarioRule dataclass
├── config.py                ← TUI config load/save (TOML)
├── screens/
│   ├── dashboard.py         ← Live metrics screen
│   ├── aura.py              ← RGB mode + brightness
│   ├── fan_curves.py        ← ASCII curve editor
│   ├── system.py            ← Profile, battery, TDP
│   ├── anime.py             ← AniMe controls
│   ├── scenarios.py         ← Scenario CRUD
│   └── settings.py          ← App config editor
└── widgets/
    ├── sparkline.py         ← ASCII time-series graph
    ├── fan_curve_editor.py  ← 8-point curve widget
    ├── colour_picker.py     ← RGB/hex input widget
    └── status_bar.py        ← Current profile + battery
```

### 17.4 D-Bus Connection Pattern

```python
# dbus/client.py
import asyncio
from dbus_fast.aio import MessageBus
from dbus_fast import BusType

class AsusdClient:
    bus: MessageBus
    _objects: dict  # path → {interface → proxy}

    async def connect(self):
        self.bus = await MessageBus(bus_type=BusType.SYSTEM).connect()

    async def discover(self):
        introspection = await self.bus.introspect(
            "xyz.ljones.Asusd", "/"
        )
        obj = self.bus.get_proxy_object(
            "xyz.ljones.Asusd", "/", introspection
        )
        obj_mgr = obj.get_interface("org.freedesktop.DBus.ObjectManager")
        self._objects = await obj_mgr.call_get_managed_objects()

    def find_interface(self, iface_name: str) -> list[str]:
        return [
            path for path, ifaces in self._objects.items()
            if iface_name in ifaces
        ]
```

### 17.5 Signal Subscription Pattern

```python
# Subscribe to platform profile changes
platform_iface.on_platform_profile_changed(
    lambda value: app.call_from_thread(dashboard.update_profile, value)
)
```

### 17.6 Fan Curve ASCII Widget Concept

```
 Fan Curve — CPU — Performance Profile
 PWM%
 100 |          ●────●
  80 |      ●──╯
  60 |  ●──╯
  40 ●──╯
   0 └──────────────────── Temp °C
     30  40  50  60  70  80  90 100
           [←/→ select point] [↑/↓ adjust] [a] apply [r] reset
```

### 17.7 Dashboard Widget Layout (Textual)

```
┌─ CPU ─────────────────┐  ┌─ GPU ─────────────────┐
│ 45°C   38%   2.4 GHz  │  │ 62°C   71%   85W NVML │
│ ▂▃▅▄▃▅▆▅▃▄ (temp)    │  │ ▄▅▇▆▅▄▆▇▅▄ (temp)    │
└───────────────────────┘  └───────────────────────┘
┌─ RAM ─────────────────┐  ┌─ Fans ────────────────┐
│ 11.2 / 16.0 GB  70%   │  │ CPU: 2800 RPM         │
│ ████████████░░░░░░░░  │  │ GPU: 3100 RPM         │
└───────────────────────┘  └───────────────────────┘
┌─ Battery ─────────────────────────────────────────┐
│ ⚡ On AC   78%   Limit: 80%   [one-shot full]     │
└───────────────────────────────────────────────────┘
```

---

## 18. Glossary

| Term | Definition |
|------|-----------|
| **asusd** | System-level hardware control daemon for ASUS laptops |
| **AniMe** | The dot-matrix LED display on the laptop lid (ASUS Zephyrus G14/G16 etc.) |
| **Aura** | ASUS brand name for RGB keyboard and peripheral lighting |
| **D-Bus** | Linux inter-process communication bus; system bus (root-level services) and session bus (user services) |
| **DMI** | Desktop Management Interface — firmware table exposing board/product identity |
| **EPP** | Energy Performance Preference — CPU power governor hint |
| **FanCurvePU** | Fan Curve Processing Unit — CPU, GPU, or MID fan |
| **Gamescope** | Valve's micro-compositor used on Steam Deck / ROG Ally |
| **hwmon** | Linux kernel hardware monitoring subsystem (`/sys/class/hwmon`) |
| **inotify** | Linux kernel filesystem change notification API |
| **ksni** | KDE StatusNotifierItem — system tray protocol |
| **NVML** | NVIDIA Management Library — userspace GPU telemetry API |
| **ObjectManager** | D-Bus `org.freedesktop.DBus.ObjectManager` — lists all managed objects and their interfaces |
| **PPT** | Package Power Tracking — AMD CPU/APU power limits (PL1/PL2 equivalent) |
| **PWM** | Pulse Width Modulation — used to control fan speed (0–255) |
| **ROG** | Republic of Gamers — ASUS gaming brand |
| **RON** | Rusty Object Notation — Rust-native serialization format |
| **Slash** | The LED accent bar on some ROG laptop chassis |
| **SPL** | Sustained Power Limit — AMD equivalent of Intel PL1 |
| **SPPT** | Slow PPT — AMD equivalent of Intel PL2 |
| **supergfxctl** | Separate daemon for GPU mode switching (Integrated / Hybrid / Dedicated) |
| **TUF** | ASUS gaming laptop sub-brand (more budget-oriented, I2C keyboard) |
| **udev** | Linux userspace device manager — handles hotplug events |
| **xyz.ljones.Asusd** | The D-Bus service name for the asusd system daemon |
| **zbus** | Pure-Rust async D-Bus library (used throughout asusctl) |
