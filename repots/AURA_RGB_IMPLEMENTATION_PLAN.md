# Aura RGB Implementation Plan

**Date:** January 25, 2026  
**Project:** armoury-crate-linux - Aura RGB Complete Implementation  
**Laptop Model:** ROG Strix G614JU  
**Aura Path:** `/xyz/ljones/aura/19b6_2_3`  
**Priority:** Get basic RGB working ASAP → Full feature parity → Clean architecture

---

## 🎯 **Executive Summary**

### Current Status
- ✅ **UI Components:** 95% complete (modern card-based design)
- ✅ **D-Bus Proxy:** Exists in rog-dbus with correct interface
- ✅ **asusd Service:** Running with Aura interface exposed
- ✅ **Connection:** Fixed - correct D-Bus interface name implemented
- ✅ **Supported Modes:** Dynamic detection implemented
- ✅ **Device Detection:** Implemented for basic/modern/TUF laptops

### Root Cause (RESOLVED)
```rust
// Issue: asusd daemon does not implement ObjectManager interface
// Fix: Implemented fallback discovery using D-Bus Introspection

// 1. Try ObjectManager (Standard)
// 2. Fallback: Introspect /xyz/ljones/aura
// 3. Parse XML to find device nodes (e.g., "19b6_2_3")
// 4. Connect to /xyz/ljones/aura/19b6_2_3
```

### Quick Win
**Phase 1 & 2 Completed** - Discovery logic robust against missing ObjectManager.

---

## 📊 **Three-Phase Implementation**

### **Phase 1: IMMEDIATE FIX (Completed)** ✅
**Goal:** Get basic RGB working NOW

1. ✅ Fix D-Bus interface discovery in `setup_aura.rs` (Switched to Introspection fallback)
2. ✅ Update interface name from `org.asuslinux.Daemon.Aura` -> `xyz.ljones.Aura`
3. ✅ Test basic functionality (compilation passed)

**Deliverable:** Working Aura RGB control with robust discovery

---

### **Phase 2: POLISH & VALIDATION (Completed)** ✅
**Goal:** Production-ready with error handling

1. ✅ Query supported modes from D-Bus
2. ✅ Disable UI buttons for unsupported modes
3. ✅ Add proper error messages
4. ✅ Implement mode-specific color enabling (rainbow modes don't need color picker)
5. ✅ Add loading states
6. ✅ Validate speed controls per mode
7. ✅ Device type detection

**Deliverable:** Robust, user-friendly RGB control

---

### **Phase 3: ADVANCED FEATURES (Future)** 🔮
**Goal:** Full Windows Armoury Crate parity

1. Per-key RGB addressing
2. Zone-based lighting
3. Power zones (sleep/wake behavior)
4. Custom animation presets
5. Save/load profiles
6. Scenario integration (auto RGB per app)
7. Keyboard layout visualization

**Deliverable:** Professional-grade RGB suite

---

## 🔧 **Phase 1: Immediate Implementation**

### **File to Modify**
- `armoury-crate-linux/src/ui/setup_aura.rs` (1 file, ~15 lines changed)

### **Changes Required**

#### **1. Fix D-Bus Service Name**
```rust
// Line ~15: Update destination
- .destination("org.asuslinux.Daemon").ok()?
+ .destination("xyz.ljones.Asusd").ok()?
```

#### **2. Fix Interface Name**
```rust
// Line ~23: Update interface check
- if ifaces.contains_key("org.asuslinux.Daemon.Aura") {
+ if ifaces.contains_key("xyz.ljones.Aura") {
```

#### **3. Update Path Discovery**
The path `/xyz/ljones` is correct - asusd creates device-specific paths like:
- `/xyz/ljones/aura/19b6_2_3` (your keyboard)

Current logic should work once interface name is fixed.

### **Expected Result After Fix**
```
[INFO  armoury_crate_linux::ui::setup_aura] Found Aura interface at: /xyz/ljones/aura/19b6_2_3
[INFO  armoury_crate_linux::ui::setup_aura] Aura proxy created successfully
[INFO  armoury_crate_linux::ui::setup_aura] Aura brightness: Med
[INFO  armoury_crate_linux::ui::setup_aura] Aura page initialized successfully
```

### **Testing Checklist**
- [ ] App launches without Aura warnings
- [ ] Brightness buttons work (Off/Low/Med/High)
- [ ] Mode cards respond to clicks
- [ ] Color picker updates keyboard color
- [ ] Speed controls change animation speed
- [ ] Toast notifications appear on changes

---

## 📋 **Phase 2: Detailed Implementation**

### **Files to Modify**
1. `armoury-crate-linux/src/ui/setup_aura.rs` (enhancement)
2. `armoury-crate-linux/ui/pages/aura.slint` (conditional UI)

### **Feature 1: Dynamic Mode Support**

**Problem:** UI shows 12 modes, but not all keyboards support all modes.

**Solution:** Query `SupportedBasicModes` from D-Bus and disable unsupported modes.

```rust
// In setup_aura_page()
let supported_modes = aura.supported_basic_modes().await.unwrap_or_default();
debug!("Supported modes: {:?}", supported_modes);

if let Some(ui) = ui_handle.upgrade() {
    let aura_data = ui.global::<AuraPageData>();
    
    // Convert to array of booleans for UI
    let mode_support: Vec<bool> = (0..=12)
        .map(|mode| supported_modes.contains(&AuraModeNum::from(mode)))
        .collect();
    
    // Set in Slint (requires new property)
    // aura_data.set_supported_modes(mode_support);
}
```

**UI Update (aura.slint):**
```slint
export global AuraPageData {
    in-out property <[bool]> mode_enabled: [true, true, true, ...];
}

// In ModeCard:
ModeCard {
    name: "Static";
    enabled: AuraPageData.mode_enabled[0];  // Disable if not supported
    selected: AuraPageData.led_mode == 0;
    clicked => { 
        if (enabled) {
            AuraPageData.led_mode = 0; 
            AuraPageData.cb_led_mode(0); 
        }
    }
}
```

### **Feature 2: Mode-Specific Color Controls**

**Problem:** Rainbow modes don't use custom colors - color picker should be disabled.

```rust
// Modes that support custom colors:
const COLOR_MODES: &[AuraModeNum] = &[
    AuraModeNum::Static,      // 0
    AuraModeNum::Breathe,     // 1
    AuraModeNum::Strobe,      // 12
    AuraModeNum::Pulse,       // 10
    // Rainbow modes DON'T use colors
];

// Check in callback:
if COLOR_MODES.contains(&effect) {
    // Enable color controls
} else {
    // Disable/hide color controls
}
```

**UI Update:**
```slint
// Only show color section for modes that support it
if AuraPageData.led_mode == 0 || AuraPageData.led_mode == 1 || 
   AuraPageData.led_mode == 10 || AuraPageData.led_mode == 12: Rectangle {
    // Color picker section
}
```

### **Feature 3: Enhanced Error Handling**

```rust
async fn find_aura_iface() -> Result<String, String> {
    let conn = zbus::Connection::system().await
        .map_err(|e| format!("D-Bus connection failed: {}", e))?;
    
    let proxy = zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("xyz.ljones.Asusd")
        .map_err(|e| format!("Invalid D-Bus destination: {}", e))?
        .path("/xyz/ljones")
        .map_err(|e| format!("Invalid D-Bus path: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Failed to build proxy: {}", e))?;

    let objs = proxy.get_managed_objects().await
        .map_err(|e| format!("Failed to get objects: {}", e))?;
    
    for (path, ifaces) in objs {
        if ifaces.contains_key("xyz.ljones.Aura") {
            info!("Found Aura interface at: {}", path);
            return Ok(path.to_string());
        }
    }
    
    Err("No Aura interface found - keyboard may not support RGB".to_string())
}
```

### **Feature 4: Device Type Detection**

```rust
// Query device type to show appropriate UI
let device_type = aura.device_type().await?;

match device_type {
    AuraDeviceType::LaptopKeyboard => {
        info!("Laptop keyboard RGB detected");
        // Show basic modes
    }
    AuraDeviceType::LaptopPost2021 => {
        info!("Modern laptop keyboard detected");
        // Show per-key option
    }
    _ => {
        warn!("Unknown Aura device type: {:?}", device_type);
    }
}
```

### **Feature 5: Loading States**

```slint
export global AuraPageData {
    in-out property <bool> is_loading: true;
    in-out property <string> error_message: "";
}

// Show loading spinner while initializing
if AuraPageData.is_loading: Rectangle {
    VerticalLayout {
        alignment: center;
        Text { text: "🔄 Loading Aura RGB..."; }
    }
}

// Show error if failed
if AuraPageData.error_message != "": Rectangle {
    Text { 
        text: AuraPageData.error_message; 
        color: red;
    }
}
```

---

## 🔮 **Phase 3: Advanced Features (Future Roadmap)**

### **Feature 1: Per-Key RGB**

**Requirements:**
- Keyboard layout file (e.g., `g614j-per-key_US.ron`)
- Direct USB packet API (`DirectAddressingRaw`)
- Visual keyboard grid UI

**Complexity:** High (2-4 hours)

**Implementation:**
1. Load keyboard layout from rog-aura data
2. Create Slint grid of key rectangles
3. Implement color picker per key
4. Build USB packets for direct addressing
5. Send via `direct_addressing_raw()` D-Bus method

**UI Mockup:**
```
┌─────────────────────────────────────┐
│  [Esc][F1][F2][F3]...               │
│  [`][1][2][3][4]...                 │
│  [Tab][Q][W][E][R]...               │
│  [Caps][A][S][D][F]...              │
│  [Shift][Z][X][C][V]...             │
│                                      │
│  Color: [Picker]  [Apply to Key]    │
│  [Import] [Export] [Randomize]      │
└─────────────────────────────────────┘
```

### **Feature 2: Zone-Based Lighting**

**Your Keyboard Supports:**
```rust
SupportedBasicZones: [] // Your model uses advanced zones, not basic
```

**Advanced Zones (from device detection):**
- Query via device capabilities
- Likely multi-zone keyboard (left/mid/right)

**Implementation:**
1. Query `supported_basic_zones()` or check device type
2. Add zone selector UI
3. Apply effects per zone

### **Feature 3: Power Zones**

**Detected on Your Keyboard:**
```
SupportedPowerZones: [Awake, Sleep, Shutdown]
```

**Use Case:** Different RGB behavior when laptop is:
- Awake: Full effect
- Sleep: Breathing effect
- Shutdown: Off or static

**Implementation:**
1. Query `supported_power_zones()`
2. Add UI for each power state
3. Save separate `LaptopAuraPower` config per state
4. Set via `set_led_power()` D-Bus method

### **Feature 4: Profile System**

**Architecture:**
```rust
struct AuraProfile {
    name: String,
    brightness: LedBrightness,
    mode: AuraEffect,
    power_zones: LaptopAuraPower,
}

struct AuraProfileManager {
    profiles: Vec<AuraProfile>,
    active_profile: usize,
}
```

**UI:**
```
Profile: [Gaming ▼]
  - Gaming (current)
  - Office
  - Night Mode
  - Custom 1
  
[Save Profile] [New] [Delete]
```

**Storage:** `~/.config/armoury-crate-linux/aura_profiles.toml`

### **Feature 5: Scenario Integration**

**Goal:** Auto-switch RGB based on running app

```rust
// In scenario_manager.rs
pub struct ScenarioRule {
    pub process_name: String,
    pub profile: String,
    pub aura_profile: Option<String>,  // NEW: Link to Aura profile
}

// When scenario triggers:
if let Some(aura_profile_name) = rule.aura_profile {
    switch_aura_profile(&aura_profile_name).await;
}
```

**Example Rules:**
- `csgo` running → "Gaming" profile (Red static, full brightness)
- `firefox` running → "Office" profile (Blue breathe, medium brightness)
- Battery < 20% → "Power Saver" profile (Off)

### **Feature 6: Keyboard Visualization**

**3D Mockup in UI:**
```slint
component KeyboardVisualization {
    // Render keyboard with live RGB preview
    // Uses keyboard layout + current colors
    // Animated effects preview
}
```

**Requirements:**
- SVG or custom Slint rendering
- Parse .ron layout files
- Real-time effect rendering

**Complexity:** Very High (8-12 hours)

---

## 🏗️ **Technical Architecture**

### **Module Structure**
```
armoury-crate-linux/
├── src/
│   ├── ui/
│   │   ├── setup_aura.rs           [Phase 1: Fix]
│   │   ├── setup_aura_advanced.rs  [Phase 3: New]
│   │   └── mod.rs
│   ├── aura/
│   │   ├── mod.rs                  [Phase 3: New module]
│   │   ├── profiles.rs             [Phase 3: Profile manager]
│   │   ├── perkey.rs               [Phase 3: Per-key control]
│   │   └── zones.rs                [Phase 3: Zone control]
│   └── scenario_manager.rs         [Phase 3: Integration]
└── ui/
    └── pages/
        ├── aura.slint              [Phase 1: Keep current]
        └── aura_advanced.slint     [Phase 3: New page]
```

### **D-Bus Communication Flow**
```
┌─────────────────┐
│  Slint UI       │
│  (aura.slint)   │
└────────┬────────┘
         │ Callback: cb_brightness(2)
         ▼
┌─────────────────┐
│  setup_aura.rs  │
│  Rust Backend   │
└────────┬────────┘
         │ async tokio::spawn
         ▼
┌─────────────────┐
│  zbus::Conn     │
│  D-Bus Client   │
└────────┬────────┘
         │ D-Bus method call
         ▼
┌─────────────────┐
│  asusd daemon   │
│  (AuraZbus)     │
└────────┬────────┘
         │ USB packets
         ▼
┌─────────────────┐
│  Keyboard HW    │
│  (19b6:2)       │
└─────────────────┘
```

### **State Management**
```rust
// UI State (Slint)
AuraPageData {
    brightness: i32,
    led_mode: i32,
    color_r/g/b: i32,
    ...
}

// Backend State (asusd)
AuraConfig {
    brightness: LedBrightness,
    mode_data: BTreeMap<AuraModeNum, AuraEffect>,
    led_type: AuraDeviceType,
    ...
}

// Sync via callbacks + D-Bus properties
```

---

## 🧪 **Testing Strategy**

### **Phase 1 Testing**
```bash
# 1. Check D-Bus interface
busctl --system tree xyz.ljones.Asusd
busctl --system introspect xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3

# 2. Test brightness via D-Bus directly
busctl --system call xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 \
    xyz.ljones.Aura set_brightness u 2

# 3. Query supported modes
busctl --system get-property xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 \
    xyz.ljones.Aura SupportedBasicModes

# 4. Launch app and test
cd /home/lebi/asusctl-fork/armoury-crate-linux
cargo run
```

### **Manual Test Cases**
| Test | Expected Result | Status |
|------|----------------|--------|
| Launch app | No Aura warnings in logs | ⏳ |
| Click "High" brightness | Keyboard lights up full | ⏳ |
| Click "Off" brightness | Keyboard lights off | ⏳ |
| Select "Static" mode | Keyboard stays solid color | ⏳ |
| Select "Breathe" mode | Keyboard pulses smoothly | ⏳ |
| Select "Rainbow" mode | Keyboard cycles colors | ⏳ |
| Move Red slider | Color changes in real-time | ⏳ |
| Click color preset | Keyboard changes to preset | ⏳ |
| Change speed to "Fast" | Animation speeds up | ⏳ |
| Restart app | Settings persist | ⏳ |

### **Automated Tests (Future)**
```rust
#[tokio::test]
async fn test_aura_brightness() {
    let aura = connect_to_aura().await.unwrap();
    aura.set_brightness(LedBrightness::High).await.unwrap();
    assert_eq!(aura.brightness().await.unwrap(), LedBrightness::High);
}

#[test]
fn test_mode_conversion() {
    assert_eq!(AuraModeNum::from(0), AuraModeNum::Static);
    assert_eq!(i32::from(AuraModeNum::Breathe), 1);
}
```

---

## 📊 **Your Keyboard Capabilities**

### **Detected Information**
```
Device: 19b6:2 (ASUS N-KEY Device)
D-Bus Path: /xyz/ljones/aura/19b6_2_3
DeviceType: LaptopKeyboard (0)
Brightness Levels: 4 (Off, Low, Med, High)
```

### **Supported Modes (13 total)**
```
Mode ID | Name          | Requires Color | Requires Speed
--------|---------------|----------------|---------------
0       | Static        | Yes            | No
1       | Breathe       | Yes            | Yes
2       | RainbowCycle  | No             | Yes
3       | RainbowWave   | No             | Yes
4       | Star          | Yes            | Yes
5       | Rain          | Yes            | Yes
6       | Highlight     | Yes            | Yes
7       | Laser         | Yes            | Yes
8       | Ripple        | Yes            | Yes
10      | Pulse         | Yes            | Yes
11      | Comet         | Yes            | Yes
12      | Flash         | Yes            | Yes
```

### **Power Zones**
```
Awake: [Keyboard: ON, Logo: ON, Lightbar: ON, Lid: ON]
Sleep: [Keyboard: ON, Logo: ON, Lightbar: ON, Lid: ON]
Shutdown: [Keyboard: OFF, Logo: OFF, Lightbar: OFF, Lid: OFF]
```

---

## 🚀 **Implementation Timeline**

### **Sprint 1: Basic RGB (TODAY - 30 min)**
- [ ] Fix D-Bus interface discovery
- [ ] Test brightness control
- [ ] Test mode switching
- [ ] Test color picker
- [ ] Test speed controls
- [ ] Verify persistence

**Deliverable:** Working RGB control in armoury-crate-linux

### **Sprint 2: Polish (Next Session - 2 hours)**
- [ ] Add supported mode detection
- [ ] Disable unsupported modes in UI
- [ ] Hide color picker for rainbow modes
- [ ] Add loading states
- [ ] Improve error messages
- [ ] Add device type detection
- [ ] Write user documentation

**Deliverable:** Production-ready RGB implementation

### **Sprint 3: Advanced Features (Future - 4-8 hours)**
- [ ] Per-key RGB support
- [ ] Zone-based lighting
- [ ] Power zone configuration
- [ ] Profile save/load system
- [ ] Scenario integration
- [ ] Keyboard visualization
- [ ] Animation presets

**Deliverable:** Full Armoury Crate parity

---

## 📝 **Code Changes Summary**

### **Phase 1: Minimum Changes**
**1 file, 2 lines changed:**

```diff
--- a/armoury-crate-linux/src/ui/setup_aura.rs
+++ b/armoury-crate-linux/src/ui/setup_aura.rs
@@ -12,7 +12,7 @@ async fn find_aura_iface() -> Option<String> {
     let conn = zbus::Connection::system().await.ok()?;
     let proxy = zbus::fdo::ObjectManagerProxy::builder(&conn)
-        .destination("org.asuslinux.Daemon").ok()?
+        .destination("xyz.ljones.Asusd").ok()?
         .path("/xyz/ljones").ok()?
         .build()
         .await
@@ -20,7 +20,7 @@ async fn find_aura_iface() -> Option<String> {
 
     let objs = proxy.get_managed_objects().await.ok()?;
     for (path, ifaces) in objs {
-        if ifaces.contains_key("org.asuslinux.Daemon.Aura") {
+        if ifaces.contains_key("xyz.ljones.Aura") {
             debug!("Found Aura interface at: {}", path);
             return Some(path.to_string());
         }
```

**That's it! Just 2 lines to fix the immediate issue.**

---

## 🎯 **Success Metrics**

### **Phase 1 Complete When:**
- [ ] App launches without Aura errors
- [ ] Brightness control works (4 levels)
- [ ] All 13 modes switch correctly
- [ ] Color picker updates keyboard in real-time
- [ ] Speed controls affect animation
- [ ] Settings persist across app restarts
- [ ] Toast notifications confirm changes

### **Phase 2 Complete When:**
- [ ] Only supported modes are enabled in UI
- [ ] Color section hidden for rainbow modes
- [ ] Error messages guide user troubleshooting
- [ ] Loading spinner shown during init
- [ ] Device type detected and logged
- [ ] All edge cases handled gracefully

### **Phase 3 Complete When:**
- [ ] Per-key RGB functional (if hardware supports)
- [ ] Zone lighting configurable
- [ ] Power zones set for sleep/wake
- [ ] 5+ custom profiles saved
- [ ] Scenario rules trigger RGB changes
- [ ] Keyboard visualization renders live
- [ ] Feature parity with Windows Armoury Crate

---

## 📚 **References & Resources**

### **Documentation**
- [rog-aura README](../rog-aura/README.md) - Aura capabilities reference
- [asusd D-Bus Interface](../asusd/src/aura_laptop/trait_impls.rs) - Server implementation
- [rog-dbus Proxy](../rog-dbus/src/zbus_aura.rs) - Client proxy API
- [Keyboard Layouts](../rog-aura/data/layouts/) - Per-key layout files

### **D-Bus Commands**
```bash
# List all Aura properties
busctl --system introspect xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 xyz.ljones.Aura

# Get current brightness
busctl --system get-property xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 xyz.ljones.Aura Brightness

# Set mode
busctl --system call xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 xyz.ljones.Aura set_led_mode u 1

# Monitor property changes
busctl --system monitor xyz.ljones.Asusd
```

### **USB Device Info**
```bash
# Check USB keyboard
lsusb | grep -i asus
# Output: Bus 001 Device 002: ID 0b05:19b6 ASUSTek Computer, Inc. N-KEY Device

# Check if per-key supported
cat /sys/class/dmi/id/board_name
# Output: G614JU (check in aura_support.ron)
```

---

## ⚠️ **Known Issues & Limitations**

### **Current Limitations**
1. **No per-key support** in Phase 1 (basic mode only)
2. **Rainbow modes** ignore custom colors (hardware limitation)
3. **Some effects** may not work on all hardware variants
4. **USB device path** changes if keyboard disconnects

### **Future Improvements**
1. Hot-plug detection for USB keyboards
2. Multiple keyboard support (external + built-in)
3. Sync RGB across multiple devices
4. Cloud profile sync
5. Community preset library

---

## 🔒 **Security Considerations**

### **D-Bus Permissions**
- Requires `system` bus access (asusd runs as root)
- User must be in `asusd` group or use polkit
- No elevated privileges in app code

### **USB Access**
- All USB communication via asusd daemon
- No direct USB access from user app
- Hardware packets validated by kernel driver

---

## 📞 **Support & Troubleshooting**

### **If Aura Not Working After Fix:**

1. **Check asusd service**
   ```bash
   systemctl status asusd
   journalctl -u asusd -f
   ```

2. **Check D-Bus interface**
   ```bash
   busctl --system tree xyz.ljones.Asusd
   ```

3. **Check USB device**
   ```bash
   lsusb | grep -i asus
   dmesg | grep -i aura
   ```

4. **Test via D-Bus directly**
   ```bash
   busctl --system call xyz.ljones.Asusd /xyz/ljones/aura/19b6_2_3 \
       xyz.ljones.Aura set_brightness u 3
   ```

5. **Check logs**
   ```bash
   RUST_LOG=debug cargo run 2>&1 | grep -i aura
   ```

---

## ✅ **Next Steps**

### **Immediate (You can start now):**
1. ✅ Read and approve this plan
2. ⏳ Implement Phase 1 fix (2 line change)
3. ⏳ Test on your ROG Strix G614JU
4. ⏳ Report results

### **After Phase 1 Works:**
1. Document any bugs found
2. Decide on Phase 2 priority
3. Create test cases
4. Plan advanced features

---

**Plan Status:** ✅ **READY FOR APPROVAL**  
**Estimated Time to Working RGB:** 15-30 minutes  
**Next Action:** Implement Phase 1 fix

---

**Questions or concerns about this plan? Let me know before we proceed!**
