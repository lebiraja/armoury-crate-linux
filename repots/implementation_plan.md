# Implementation Plan - Armoury Crate Linux Replica

# Goal Description
The goal is to enhance the existing `rog-control-center` application to more closely resemble the Windows Armoury Crate experience. This involves adding features currently missing in the Linux implementation: **Scenario Profiles**, an **Enhanced Dashboard** with real-time monitoring, and **Game Visual** controls.

## User Review Required
> [!IMPORTANT]
> **Scenario Profiles**: This requires monitoring running processes or the active window. This needs valid permissions and might behave differently on Wayland vs X11 (though the app focuses on Wayland).
> **System Monitoring**: Accurate real-time CPU/GPU usage requires efficient reading of system files (`/proc`, etc.) without causing high CPU load.

## Proposed Changes

### Component: `rog-control-center` (UI)

#### [MODIFY] [mod.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/ui/pages/mod.rs)
- Register new pages: `setup_dashboard.rs`, `setup_scenario.rs`.

#### [NEW] [setup_dashboard.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/ui/pages/setup_dashboard.rs)
- **Visuals**: Create a "Home" dashboard similar to Armoury Crate.
- **Widgets**:
    - CPU/GPU Temp & Load Circular Gauges.
    - RAM / Storage Usage Bars.
    - Quick Toggles: GPUMode (Eco/Standard/Ultimate), Power Profile (Silent/Turbo).
    - Fan Speed visualizer.

#### [NEW] [setup_scenario.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/ui/pages/setup_scenario.rs)
- **Functionality**: UI to create profiles.
- **Inputs**:
    - Application Name / Binary Path.
    - Desired Power Profile (e.g., Turbo).
    - Desired Aura Mode (e.g., Static Red).
- **List**: Display active scenario rules.

### Component: `rog-control-center` (Logic)

#### [MODIFY] [main.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/main.rs)
- Initialize the new Dashboard and Scenario pages.
- Integrate a simple update loop for the monitoring widgets (1s interval).

#### [NEW] [monitor.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/monitor.rs)
- **Purpose**: Helper module to read system stats.
- **Functions**: `get_cpu_usage()`, `get_gpu_temp()`, `get_ram_usage()`.

#### [NEW] [scenario_manager.rs](file:///home/lebi/asusctl-fork/rog-control-center/src/scenario_manager.rs)
- **Purpose**: Logic to check active process against the list of rules.
- **Restriction**: On Wayland, getting the "active window" is secure/difficult. We might start with checks for "process exists" (is game running?) rather than focus.

## Verification Plan

### Automated Tests
- `cargo test`: Ensure new modules conform to Rust standards.
- Unit tests for `monitor.rs` data parsing (mocking `/proc` files).

### Manual Verification
1.  **Dashboard**:
    - Open `rog-control-center`.
    - Verify CPU/GPU graphs move and match `htop`/`nvtop` values.
    - Click "Turbo" toggle -> Verify system profile changes.
2.  **Scenario Profiles**:
    - Create a rule: "When `firefox` runs -> Set 'Silent' mode".
    - Open Firefox.
    - Verify notification or UI implies switch to Silent.
    - Close Firefox.
    - Verify return to default profile (optional logic).
