# features101.md — Complete Feature Inventory for Python TUI Rebuild

> Exhaustive feature catalogue extracted directly from the `asusctl` /
> `armoury-crate-linux` codebase (v6.3.1).  
> Every item here maps to real, working code in the repo and must be
> reimplemented in the Python TUI via D-Bus unless stated otherwise.

---

## Table of Contents

1. [System Monitoring Dashboard](#1-system-monitoring-dashboard)
2. [Platform Power Profiles](#2-platform-power-profiles)
3. [Battery Management](#3-battery-management)
4. [Aura RGB Lighting](#4-aura-rgb-lighting)
5. [Fan Curve Control](#5-fan-curve-control)
6. [Firmware Attributes — TDP & BIOS Toggles](#6-firmware-attributes--tdp--bios-toggles)
7. [AniMe Matrix Display](#7-anime-matrix-display)
8. [Slash LED Bar](#8-slash-led-bar)
9. [SCSI Aura Peripherals](#9-scsi-aura-peripherals)
10. [Screenpad / Secondary Display Backlight](#10-screenpad--secondary-display-backlight)
11. [Scenario Profiles — Automatic Rule Engine](#11-scenario-profiles--automatic-rule-engine)
12. [Notifications](#12-notifications)
13. [System Tray Integration](#13-system-tray-integration)
14. [Application Configuration](#14-application-configuration)
15. [D-Bus Interface Reference](#15-d-bus-interface-reference)
16. [CLI Feature Parity Reference](#16-cli-feature-parity-reference)
17. [Python TUI Feature Priority Map](#17-python-tui-feature-priority-map)

---

## 1. System Monitoring Dashboard

The dashboard polls hardware every **500 ms** and keeps a rolling history
buffer (default 60 seconds) for sparkline/graph display.

### 1.1 CPU

| Metric | Source | Notes |
|--------|--------|-------|
| Usage % | `sysinfo` crate / `/proc/stat` | Average across all cores |
| Temperature °C | `/sys/class/hwmon/hwmon*/temp*_input` | `k10temp` / `zenpower` (AMD), `coretemp` (Intel) |
| Frequency MHz | `sysinfo` | Average across all cores |

**Hwmon driver mapping:**

| Driver name | CPU vendor | Typical temp node |
|-------------|-----------|-------------------|
| `k10temp` | AMD Ryzen | `temp1_input` |
| `zenpower` | AMD Ryzen (alt) | `temp2_input` |
| `coretemp` | Intel | `temp1_input` (package) |

### 1.2 GPU

| Metric | AMD source | NVIDIA source |
|--------|-----------|--------------|
| Usage % | `/sys/class/hwmon/<amdgpu>/device/gpu_busy_percent` | NVML `utilization_rates().gpu` |
| Temperature °C | `/sys/class/hwmon/<amdgpu>/temp1_input` | NVML `temperature(TemperatureSensor::Gpu)` |
| Power draw W | hwmon `power1_average` (if present) | NVML `power_usage()` ÷ 1000 (mW→W) |

NVIDIA support requires `nvml_wrapper` (Rust) or `pynvml` (Python).  
Discovered at runtime — graceful fallback if NVML unavailable.

### 1.3 RAM

| Metric | Value |
|--------|-------|
| Usage % | `(used / total) * 100` |
| Used GB | bytes ÷ 1 073 741 824 |
| Total GB | bytes ÷ 1 073 741 824 |

### 1.4 Storage (Disk)

| Metric | Value |
|--------|-------|
| Usage % | first detected disk mount |
| Used GB | — |
| Total GB | — |

### 1.5 Fans

- Up to 3 fans detected from hwmon: `fan1_input`, `fan2_input`, `fan3_input`
- Dynamically discovered — number varies per laptop model
- Values in RPM

### 1.6 Power & Battery

| Metric | Source |
|--------|--------|
| Total system power draw W | hwmon `power*_input` |
| Battery % | `sysinfo` / `/sys/class/power_supply/` |
| On AC power | `/sys/class/power_supply/AC*/online` |

### 1.7 History Graphs

- Rolling time-series for: CPU temp, GPU temp, CPU usage, GPU usage
- Buffer length = `history_duration_sec * (1000 / update_interval_ms)` samples
- In Python TUI: render as sparklines or ASCII bar graphs using Textual `Sparkline` widget

---

## 2. Platform Power Profiles

### 2.1 Available Profiles

| Profile | kernel `platform_profile` value | Typical use |
|---------|--------------------------------|-------------|
| `Balanced` | `balanced` | Default everyday use |
| `Performance` | `performance` | Gaming / heavy workloads |
| `Quiet` / `Low-power` | `quiet` / `low-power` | Silent operation |
| `Custom` | `custom` | User-defined curve |

### 2.2 D-Bus Methods (via `xyz.ljones.Platform`)

```
get  platform_profile()           → String
set  set_platform_profile(value)  → ()
get  platform_profile_choices()   → Array<String>
```

### 2.3 AC / Battery Profile Binding

- Per-power-source default profile stored in `/etc/asusd/asusd.ron`
- D-Bus: `set_platform_profile` with `ac=true` or `battery=true` flag path
- TUI must offer separate selectors for AC and battery modes

### 2.4 CPU Governor & EPP (applied internally by `asusd`)

| Profile | Governor | EPP |
|---------|----------|-----|
| Performance | `performance` | `performance` |
| Balanced | `schedutil` | `balance_performance` |
| Quiet | `powersave` | `power` |

These are applied by `asusd` automatically — the TUI only needs to set the profile.

### 2.5 Cycling Profiles

The CLI `asusctl profile next` cycles through discovered profiles in order.
The TUI should expose a single "cycle" keybind (e.g. `F5`) in addition to the selector.

---

## 3. Battery Management

### 3.1 Charge Control End Threshold

- Limits the maximum charge percentage to preserve battery health
- Range: 1–100 (typically 60–80 recommended for longevity)
- Stored persistently in `/etc/asusd/asusd.ron`
- Restored after suspend/resume by the daemon

**D-Bus:**
```
get  charge_control_end_threshold()         → u8
set  set_charge_control_end_threshold(u8)   → ()
```

### 3.2 One-Shot Full Charge

- Temporarily overrides the limit and charges to 100% exactly once
- Returns to the configured limit after that cycle

**D-Bus:**
```
call  one_shot_full_charge()   → ()
```

### 3.3 TUI Representation

- Slider or numeric input for limit (20–100)
- Toggle button: "One-shot full charge"
- Status line: "Currently charging to X%" with AC/Battery icon

---

## 4. Aura RGB Lighting

### 4.1 Built-in Hardware Modes

Each mode is a firmware-level effect executed by the keyboard MCU.

| Mode name | Enum value | Colour params | Speed param |
|-----------|-----------|---------------|-------------|
| `Static` | 0 | colour1 | — |
| `Breathe` | 1 | colour1, colour2 | Low/Med/High |
| `Strobe` | 2 | — | Low/Med/High |
| `Rainbow` | 3 | — | Low/Med/High |
| `Star` | 4 | colour1, colour2 | Low/Med/High |
| `Rain` | 5 | colour1, colour2 | Low/Med/High |
| `Highlight` | 6 | colour1 | Low/Med/High |
| `Laser` | 7 | colour1 | Low/Med/High |
| `Ripple` | 8 | colour1 | Low/Med/High |
| `Pulse` | 10 | colour1 | — |
| `Comet` | 11 | colour1 | — |
| `Flash` | 12 | colour1 | — |

Not all modes are available on all laptops — the supported set is read from
`supported_basic_modes()` at runtime.

### 4.2 Zones (Multi-Zone Keyboards)

| Zone | Description |
|------|-------------|
| `Key1`–`Key4` | Main keyboard quadrants |
| `Logo` | ROG logo |
| `BarLeft`, `BarRight` | Lightbar segments |
| `ZonedKbLeft/LeftMid/RightMid/Right` | Keyboard regions (newer gen) |
| `LightbarRight/RightCorner/RightBottom` | Lightbar sub-zones |
| `LightbarLeftBottom/LeftCorner/Left` | Lightbar sub-zones |

Available zones returned by `supported_basic_zones()`.

### 4.3 Brightness Levels

| Level | Value |
|-------|-------|
| Off | 0 |
| Low | 1 |
| Med | 2 |
| High | 3 |

**D-Bus:** `set_brightness(level: u8)` / `brightness() → u8`

### 4.4 LED Power Zones

Independent power control per zone for each system state:

| State | Description |
|-------|-------------|
| `boot` | BIOS/pre-boot |
| `awake` | Normal operation |
| `sleep` | Suspend/hibernate |
| `shutdown` | Shutdown sequence |

Two API generations:
- **Pre-2021** (`LedPowerCommand1`): single zone on/off
- **2021+** (`LedPowerCommand2`): per-zone per-state matrix

**D-Bus Interface:** `xyz.ljones.Aura`  
**Path:** `/xyz/ljones/aura/<device_id>` (discovered via ObjectManager)

### 4.5 Mode Navigation

```
call  next_aura_mode()   → ()
call  prev_aura_mode()   → ()
```

TUI keybinds: `[` prev, `]` next.

### 4.6 Colour Representation

Colours are 3-byte RGB `(r: u8, g: u8, b: u8)`.  
The Slint GUI uses an HSV picker which converts to RGB internally.  
Python TUI should use RGB hex input `#RRGGBB` or three sliders.

### 4.7 Device Type Detection

```
get  device_type() → AuraDeviceType
     # LaptopKeyboardPre2021
     # LaptopKeyboard2021
     # LaptopKeyboardTuf
```

Used to decide which power API and zone layout to expose. Read once on startup.

### 4.8 Supported Modes / Zones Query

```
get  supported_basic_modes()   → Array<AuraModeNum>
get  supported_basic_zones()   → Array<String>
get  supported_power_zones()   → Array<String>
get  supported_brightness()    → Array<u8>
```

Always read these first — do not hard-code the mode list.

---

## 5. Fan Curve Control

### 5.1 Supported Fans

Auto-detected from `asus_custom_fan_curve` hwmon node:

| Fan | PWM node | Label |
|-----|----------|-------|
| CPU | `pwm1` | CPU fan |
| GPU | `pwm2` | GPU / dGPU fan |
| MID | `pwm3` | Mid-plane (some models) |

### 5.2 Profiles with Curves

Each of the 4 platform profiles has its own stored curve:

- `balanced`, `performance`, `quiet`, `custom`

### 5.3 Curve Format

8 control points, each being `(temperature_°C: u8, pwm_0_255: u8)`.  
Points must be ascending by temperature.

```
Example: [(30,0),(40,10),(50,25),(60,60),(70,100),(80,150),(90,200),(100,220)]
```

### 5.4 D-Bus Methods (`xyz.ljones.FanCurves`)

```
get   fan_curve_data(profile: String)            → Array<CurveData>
set   set_fan_curve(profile: String, curve: CurveData) → ()
call  reset_profile_curves(profile: String)      → ()
```

`CurveData` fields: `fan: FanCurvePU`, `pwm: [u8;8]`, `temp: [u8;8]`, `enabled: bool`

### 5.5 Enable / Disable

Each fan curve can be individually enabled or disabled per profile.
When disabled, the EC's firmware default curve is used.

### 5.6 TUI Representation

ASCII curve visualisation: 8-point graph plotted as a step chart.  
X-axis: temperature 0–100 °C  
Y-axis: PWM 0–255 (or 0–100%)  
Arrow keys to move selected point; Enter to confirm; `a` to apply.

### 5.7 Reading Current RPM

Fan RPM is read from hwmon `fan1_input`, `fan2_input` — shown live on
the dashboard alongside the curve configuration.

---

## 6. Firmware Attributes — TDP & BIOS Toggles

Exposed via the `asus-armoury` kernel driver (mainline since Linux 6.19)
and surfaced over D-Bus as `xyz.ljones.AsusArmoury`.

Each attribute is a separate D-Bus object at:
`/xyz/ljones/asus_armoury/<attribute_name>`

### 6.1 Power / TDP Attributes

| Attribute | Description | Typical range (W) |
|-----------|-------------|-------------------|
| `ppt_pl1_spl` | CPU Sustained Power Limit (PL1 / SPL) | 5–54 W |
| `ppt_pl2_sppt` | CPU Short-term Boost Limit (PL2 / SPPT) | 5–64 W |
| `ppt_apu_sppt` | APU SPPT (iGPU+CPU combined) | 5–54 W |
| `ppt_platform_sppt` | Platform-level short-term power | 5–64 W |
| `ppt_fppt` | Fast PPT | model-specific |
| `nv_dynamic_boost` | NVIDIA dGPU dynamic boost power | 0–25 W |
| `nv_temp_target` | NVIDIA GPU target temperature °C | 60–90 °C |

### 6.2 Display / BIOS Toggles

| Attribute | Description | Values |
|-----------|-------------|--------|
| `panel_od` | Panel overdrive (faster pixel response) | 0 = off, 1 = on |
| `gpu_mux_mode` | GPU MUX switch | 0 = hybrid (iGPU+dGPU), 1 = dedicated dGPU only |
| `boot_sound` | BIOS POST sword sound | 0 = off, 1 = on |
| `dgpu_disable` | Disable discrete GPU entirely | 0 / 1 |
| `egpu_enable` | Enable external GPU | 0 / 1 |
| `tuf_rgb_control` | TUF keyboard LED enable | 0 / 1 |

### 6.3 D-Bus Interface per Attribute

```
get  name()            → FirmwareAttribute   # enum variant = attribute identifier
get  current_value()   → i32
set  set_current_value(i32)  → ()
get  default_value()   → i32
get  min_value()       → i32
get  max_value()       → i32
get  scalar_increment()→ i32                 # step size (e.g. 1 W)
get  attribute_type()  → String             # "Integer" or "Enum"
get  possible_values() → PossibleValues     # { strings: [], nums: [] }
```

### 6.4 TUI Representation

- Numeric attributes: horizontal slider with min/max labels and current value
- Boolean attributes: toggle switch
- Enum attributes: dropdown / selection list
- Changes take effect immediately via D-Bus; no "apply" button needed
- `gpu_mux_mode` and `dgpu_disable` require a reboot — show warning

---

## 7. AniMe Matrix Display

Available on: **GA401, GA402, GU604, G635L, G835L**.

### 7.1 Display Toggles

| Setting | D-Bus property | Type |
|---------|---------------|------|
| Enable display | `enable_display` | bool |
| Global brightness | `brightness` | `Brightness` enum (Off/Low/Med/High) |
| Off when lid closed | `off_when_lid_closed` | bool |
| Off when suspended | `off_when_suspended` | bool |
| Off when unplugged | `off_when_unplugged` | bool |
| Brightness on battery | `brightness_on_battery` | `Brightness` |
| Enable builtins | `builtins_enabled` | bool |

### 7.2 Builtin Animations (Selectable per State)

Each state maps to an enum value:

```
boot     : AnimBooting   { Off, Variant1, Variant2, ... }
awake    : AnimAwake     { Off, Variant1, Variant2, ... }
sleep    : AnimSleeping  { Off, Variant1, Variant2, ... }
shutdown : AnimShutdown  { Off, Variant1, Variant2, ... }
```

**D-Bus:**
```
get  builtin_animations()   → Animations { boot, awake, sleep, shutdown }
set  set_builtin_animations(Animations)  → ()
```

### 7.3 Custom Sequence Types (advanced, for file-based playback)

| Type | Description |
|------|-------------|
| `AnimeImage` | Static PNG scaled to AniMe resolution |
| `AnimeGif` | Animated GIF playback with timing control |
| `AnimeDiagonal` | Diagonal stripe pattern from image |
| `AnimeGrid` | Grid overlay from image data |

These are driven by `asusd-user` (userspace daemon), not `asusd`.  
The TUI can expose a file-picker to point the user daemon at a file.

### 7.4 Hardware-Specific Formats

| Model | USB prefix | Matrix dimensions |
|-------|-----------|------------------|
| GA401 | `USB_PREFIX1` | 1245 pixels |
| GA402 | `USB_PREFIX1` + `USB_PREFIX2` + `USB_PREFIX3` | larger matrix |
| GU604 / G635L / G835L | model-specific | varies |

The TUI does **not** need to handle raw USB — the daemon abstracts this.

---

## 8. Slash LED Bar

A decorative LED strip on select ROG models.

### 8.1 SlashMode Values

Exact enum values are model-specific; the TUI reads supported modes from `asusd`.

### 8.2 Controllable Properties

| Property | D-Bus key | Type |
|----------|-----------|------|
| Active mode | `slash_mode` | `SlashMode` enum |
| Brightness | `brightness` | u8 (0–3) |
| Animation speed | `speed` | u8 |

**D-Bus Interface:** `xyz.ljones.Slash`  
**Path:** `/xyz/ljones/aura/slash`

---

## 9. SCSI Aura Peripherals

USB-connected ASUS peripherals that expose Aura RGB via SCSI/HID.

- Discovered automatically by the same `DeviceManager` that handles keyboards
- Each device registers on D-Bus with its USB product ID in the path
- Mode selection via `AuraMode` enum (device-specific values)

**D-Bus Interface:** Interface name varies; use ObjectManager discovery.

---

## 10. Screenpad / Secondary Display Backlight

For models with a secondary "ScreenPad Plus" display (e.g., Zenbook Duo, Zenbook Pro Duo).

### 10.1 Controllable Properties

| Property | Range | Description |
|----------|-------|-------------|
| `screenpad_brightness` | 0–100 | Secondary display brightness % |
| `screenpad_gamma` | 0.5–2.2 | Gamma correction (1.0 = linear) |
| `screenpad_sync_with_primary` | bool | Lock to primary display brightness |

**D-Bus Interface:** `xyz.ljones.Backlight`  
**Path:** discovered via ObjectManager

### 10.2 TUI Representation

Slider 0–100 for brightness; numeric input for gamma with preset buttons
(0.5 / 1.0 / 1.5 / 2.2); checkbox for sync.

---

## 11. Scenario Profiles — Automatic Rule Engine

Monitors running processes and applies hardware settings automatically.

### 11.1 Rule Object

```toml
[[rules]]
id            = "gaming-mode"
name          = "Gaming Mode"
process_name  = "cyberpunk2077"   # matched against /proc/*/cmdline
power_profile = "Performance"     # Optional
fan_profile   = "Performance"     # Optional
priority      = 80                # 0–100; higher wins

[rules.aura_mode]
mode      = "Breathe"
color_r   = 255
color_g   = 0
color_b   = 0
brightness = 3
```

### 11.2 Engine Behaviour

- Scans `/proc` every `check_interval_ms` ms (default: 2000)
- Sorts rules by priority descending; first match wins
- Tracks `active_rule_id` to avoid duplicate applications
- When no rule matches: reverts to `default_profile` (if one was active)
- Config file: `~/.config/armoury-crate-linux/scenarios.toml`

### 11.3 D-Bus Integration Points

When a rule fires, the TUI applies changes via:
- `set_platform_profile()` on `xyz.ljones.Platform`
- `set_mode()` on `xyz.ljones.Aura`

### 11.4 TUI Scenario Manager

- Table listing all rules (name, process, profile, priority)
- Add / Edit / Delete rule form
- Enable / Disable toggle for the entire engine
- "Current active rule" status line (with matched process name)
- Default fallback profile selector

---

## 12. Notifications

Desktop notifications sent via `libnotify` / `notify-rust`.  
In the Python TUI, these can be relayed through `notify-send` / `plyer` /
`desktop-notifier` or shown as in-TUI status bar messages.

### 12.1 Notification Events

| Event | Trigger |
|-------|---------|
| dGPU power changed | `supergfxctl` device status: Active ↔ Suspended |
| GPU mode changed | `supergfxctl`: Integrated / Hybrid / Dedicated |
| Platform profile changed | `platform_profile` D-Bus signal |
| AC/Battery transition | Power supply online sysfs |
| Aura mode changed | `aura_mode` D-Bus signal |

### 12.2 AC / Battery Hook Commands

User-configurable shell commands executed on power source change:

```toml
ac_command  = "notify-send 'AC connected'"
bat_command = "systemctl start tlp"
```

Executed via `Command::spawn()` — not blocking.

### 12.3 Notification Config Flags

| Flag | Default | Description |
|------|---------|-------------|
| `enabled` | true | Master on/off |
| `show_gpu_status` | true | dGPU power state changes |
| `show_profile_changes` | true | Platform profile switches |
| `show_charging_status` | true | AC/battery transitions |
| `receive_notify_gfx` | true | GPU mode changes (supergfxctl) |
| `receive_notify_gfx_status` | true | dGPU active/suspend status |

---

## 13. System Tray Integration

`ksni`-based desktop status tray icon.  
For the Python TUI, a system tray is optional — the TUI itself is the primary
interface. Consider using `pystray` if tray support is wanted.

### 13.1 Icon State Mapping

| State | Icon colour | Condition |
|-------|------------|-----------|
| Normal | Blue (ROG) | Default |
| GPU dedicated | Green | dGPU MUX = dedicated |
| Warning | Red | Error condition |
| Integrated | White + GPU icon | iGPU only mode |

### 13.2 Tray Menu Items

- **Open Control Center** → sends `AppState::MainWindowShouldOpen` D-Bus signal
- **Quit** → `std::process::exit(0)`

---

## 14. Application Configuration

The TUI stores its own config file independently of `asusd`.

### 14.1 Config File Location

`~/.config/rog/armoury-crate-linux.cfg` (TOML / RON format)

### 14.2 Config Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `theme` | String | `"rog_gaming"` | Colour theme name |
| `run_in_background` | bool | `true` | Keep running after close |
| `startup_in_background` | bool | `false` | Start hidden |
| `enable_tray_icon` | bool | `true` | System tray |
| `start_fullscreen` | bool | `false` | ROG Ally mode |
| `fullscreen_width` | u32 | 1920 | — |
| `fullscreen_height` | u32 | 1080 | — |
| `default_page` | String | `"dashboard"` | First tab on launch |
| `ac_command` | String | `""` | Shell cmd on AC |
| `bat_command` | String | `""` | Shell cmd on battery |
| `monitoring.enabled` | bool | `true` | — |
| `monitoring.update_interval_ms` | u64 | 500 | Poll rate |
| `monitoring.history_duration_sec` | u64 | 60 | Graph window |
| `monitoring.show_gpu_metrics` | bool | `true` | GPU panel |
| `monitoring.show_power_metrics` | bool | `true` | Power panel |
| `notifications.enabled` | bool | `true` | All notifications |
| `notifications.show_gpu_status` | bool | `true` | — |
| `notifications.show_profile_changes` | bool | `true` | — |
| `notifications.show_charging_status` | bool | `true` | — |
| `notifications.receive_notify_gfx` | bool | `true` | — |
| `notifications.receive_notify_gfx_status` | bool | `true` | — |
| `scenarios.enabled` | bool | `true` | Scenario engine |
| `scenarios.check_interval_ms` | u64 | 2000 | Process scan rate |
| `scenarios.default_profile` | String | `"Balanced"` | Fallback profile |

### 14.3 Daemon Config Files (read-only from TUI perspective)

| File | Location | Format |
|------|----------|--------|
| Main daemon config | `/etc/asusd/asusd.ron` | RON |
| Aura state | `/etc/asusd/aura.ron` | RON |
| AniMe sequences | `/etc/asusd/anime.conf` | RON |
| Fan curve profiles | `/etc/asusd/profile.conf` | RON |
| User Aura config | `~/.config/rog/rog-user.cfg` | RON |
| Per-key Aura effects | `~/.config/rog/<name>.ron` | RON |

The TUI does **not** write to these files — it uses D-Bus exclusively.

---

## 15. D-Bus Interface Reference

### 15.1 Service

**Bus:** System bus (`org.freedesktop.DBus.ObjectManager` at `/`)  
**Service name:** `xyz.ljones.Asusd`

### 15.2 Interface Summary

| Interface | Path | Main methods/props |
|-----------|------|--------------------|
| `xyz.ljones.Asusd` | `/xyz/ljones` | `version()` |
| `xyz.ljones.Platform` | `/xyz/ljones` | platform_profile, charge_control_end_threshold, one_shot_full_charge, supported_properties |
| `xyz.ljones.Aura` | `/xyz/ljones/aura/<id>` | brightness, supported_basic_modes, supported_basic_zones, set_mode, next_aura_mode, prev_aura_mode, device_type |
| `xyz.ljones.FanCurves` | `/xyz/ljones` | fan_curve_data, set_fan_curve, reset_profile_curves |
| `xyz.ljones.Anime` | `/xyz/ljones/aura/anime` | brightness, enable_display, builtins_enabled, builtin_animations, off_when_lid_closed, off_when_suspended, off_when_unplugged |
| `xyz.ljones.Slash` | `/xyz/ljones/aura/slash` | slash_mode, brightness, speed |
| `xyz.ljones.Backlight` | via ObjectManager | screenpad_brightness, screenpad_gamma, screenpad_sync_with_primary |
| `xyz.ljones.AsusArmoury` | `/xyz/ljones/asus_armoury/<attr>` | name, current_value, set_current_value, min_value, max_value, default_value, scalar_increment, attribute_type |

### 15.3 ObjectManager Discovery Pattern

```python
import dbus

bus = dbus.SystemBus()
obj_mgr = bus.get_object("xyz.ljones.Asusd", "/")
ifaces = dbus.Interface(obj_mgr, "org.freedesktop.DBus.ObjectManager")
objects = ifaces.GetManagedObjects()

# Find Aura interface
for path, interfaces in objects.items():
    if "xyz.ljones.Aura" in interfaces:
        aura = bus.get_object("xyz.ljones.Asusd", path)
        break
```

### 15.4 Signal Subscriptions

Every writable property emits `<PropertyName>Changed(value)` signals.
Subscribe to avoid polling:

```python
bus.add_signal_receiver(
    handler,
    signal_name="platform_profile_changed",
    dbus_interface="xyz.ljones.Platform",
    bus_name="xyz.ljones.Asusd"
)
```

### 15.5 Version Compatibility Check

On startup, call `version()` and compare to your TUI version.
Warn the user if they differ — method signatures may not match.

---

## 16. CLI Feature Parity Reference

The following maps `asusctl` subcommands to their D-Bus equivalents.
The Python TUI replaces the GUI entirely; the CLI is kept for power users.

| `asusctl` command | D-Bus equivalent |
|-------------------|-----------------|
| `profile next` | `Platform.set_platform_profile(next_in_list)` |
| `profile set Performance` | `Platform.set_platform_profile("performance")` |
| `aura --next-mode` | `Aura.next_aura_mode()` |
| `aura --prev-mode` | `Aura.prev_aura_mode()` |
| `aura Static -c 255 0 0` | `Aura.set_mode(AuraEffect{Static, (255,0,0)})` |
| `battery limit 80` | `Platform.set_charge_control_end_threshold(80)` |
| `battery oneshot` | `Platform.one_shot_full_charge()` |
| `fan-curve set ...` | `FanCurves.set_fan_curve(profile, CurveData)` |
| `armoury list` | Enumerate all `xyz.ljones.AsusArmoury` objects |
| `armoury set ppt_pl1_spl 30` | `AsusArmoury.set_current_value(30)` at ppt_pl1_spl path |
| `backlight --screenpad-brightness 80` | `Backlight.set_screenpad_brightness(80)` |
| `anime ...` | `Anime.*` properties |
| `slash ...` | `Slash.*` properties |

---

## 17. Python TUI Feature Priority Map

Use this to phase the development: implement P1 first for a usable MVP.

| Priority | Feature | Complexity | D-Bus dependency |
|----------|---------|-----------|-----------------|
| **P1** | Real-time dashboard (CPU/GPU/RAM/fans/battery) | Medium | No (direct sysfs) |
| **P1** | Platform profile switcher | Low | `xyz.ljones.Platform` |
| **P1** | Battery charge limit slider | Low | `xyz.ljones.Platform` |
| **P1** | Aura mode selector + brightness | Medium | `xyz.ljones.Aura` |
| **P1** | Version check & error display | Low | `xyz.ljones.Asusd` |
| **P2** | Fan curve viewer (ASCII graph) | High | `xyz.ljones.FanCurves` |
| **P2** | Fan curve editor (draggable points) | High | `xyz.ljones.FanCurves` |
| **P2** | TDP / PPT sliders | Medium | `xyz.ljones.AsusArmoury` |
| **P2** | Panel OD / Boot sound toggle | Low | `xyz.ljones.AsusArmoury` |
| **P2** | GPU MUX toggle (with reboot warning) | Low | `xyz.ljones.AsusArmoury` |
| **P2** | AC/battery hook commands config | Low | local config |
| **P2** | D-Bus signal subscriptions for live updates | Medium | all interfaces |
| **P3** | Scenario profile manager (CRUD) | High | local + D-Bus |
| **P3** | Scenario engine (process monitor) | Medium | local (`/proc`) |
| **P3** | AniMe builtin animation selector | Medium | `xyz.ljones.Anime` |
| **P3** | AniMe display toggles | Low | `xyz.ljones.Anime` |
| **P3** | Slash LED mode/brightness | Low | `xyz.ljones.Slash` |
| **P3** | Screenpad brightness/gamma | Low | `xyz.ljones.Backlight` |
| **P3** | Notification config editor | Low | local config |
| **P3** | History sparkline graphs | Medium | local ring buffer |
| **P4** | Per-key Aura profile loader | Very High | `xyz.ljones.Aura` |
| **P4** | AniMe file picker (GIF/PNG) | Medium | `asusd-user` D-Bus |
| **P4** | System tray (background mode) | Low | `pystray` |
| **P4** | Firmware attribute enum dropdowns | Medium | `xyz.ljones.AsusArmoury` |

### Recommended Python TUI Stack

| Layer | Library | Rationale |
|-------|---------|-----------|
| TUI framework | `Textual` (≥ 0.50) | asyncio-native, rich widgets, CSS theming |
| D-Bus | `dbus-fast` or `dbus-next` | async-native D-Bus for Python |
| System metrics | `psutil` | CPU/RAM/disk; cross-platform fallback |
| GPU (NVIDIA) | `pynvml` | Direct NVML bindings matching Rust side |
| GPU (AMD) | `/sys/class/hwmon` reads | No extra library needed |
| Config | `tomllib` (stdlib 3.11+) + `tomli-w` | Matches daemon TOML format |
| Process scan | `psutil.process_iter()` | Safer than raw `/proc` traversal |
| Notifications | `desktop-notifier` or `notify2` | libnotify bridge |
| Async loop | `asyncio` (stdlib) | Native Textual integration |
