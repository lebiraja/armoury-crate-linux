# Features Analysis & Comparison

This document compares the features of the official Windows Armoury Crate application with the current Linux `rog-control-center` / `asusctl` implementation.

## 1. Feature Comparison Matrix

| Feature Category | Feature Name | Windows (Armoury Crate) | Linux (Current Codebase) | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Device Control** | **System Performance Modes** | Yes (Silent, Performance, Turbo, Manual) | Yes (Silent, Balanced, Turbo, Manual) | ✅ Implemented |
| | **Fan Control & Curves** | Yes (Custom curves) | Yes (Custom curves supported) | ✅ Implemented |
| | **GPU Mode Switching** | Yes (Ultimante, Standard, Eco, Optimized) | Yes (via `supergfxctl` integration) | ✅ Implemented |
| | **Panel Overdrive** | Yes | Yes (Hardware support required) | ✅ Implemented |
| | **MUX Switch** | Yes | Yes | ✅ Implemented |
| **Lighting (Aura)** | **Basic Effects** | Yes (Static, Breathing, Strobing, Color Cycle) | Yes | ✅ Implemented |
| | **Per-Key RGB** | Yes | Yes | ✅ Implemented |
| | **Aura Sync** | Yes (Syncs peripherals) | Partial (Codebase has `rog-aura`, mostly laptop-focused) | ⚠️ Partial |
| | **Aura Creator** | Yes (Advanced timeline-based editor) | No (Has "Fancy LED modes" but no timeline editor) | ❌ Missing |
| | **Aura Wallpaper** | Yes | No | ❌ Missing |
| **AniMe Matrix** | **Animation/GIFs** | Yes | Yes (Specific models like G14) | ✅ Implemented |
| | **System Stats** | Yes | Yes | ✅ Implemented |
| | **AniMe Vision** | Yes (Newer models) | Partial/WIP | ⚠️ Partial |
| **System Info** | **Real-time Monitoring** | Yes (CPU/GPU Temp, Usage, Frequency, RAM, Storage) | Basic (Fan speeds, some temps) | ⚠️ Needs Improvement |
| | **Resource Monitor** | Yes (Kill processes, free RAM) | No | ❌ Missing |
| **Gaming/Usage** | **Scenario Profiles** | Yes (Auto-switch profiles per game/app) | No (Manual toggle primarily, basic AC/Bat switch) | ❌ Missing |
| | **Game Launcher** | Yes (Library view) | No | ❌ Missing |
| | **Game Visual** | Yes (Screen color profiles) | No | ❌ Missing |
| | **Macros** | Yes | No (Only basic key bindings if supported) | ❌ Missing |
| **Audio** | **Noise Cancellation** | Yes (AI Noise Cancellation settings) | No | ❌ Missing |
| **Settings** | **Update Center** | Yes (Drivers/BIOS) | No (Handled by Distro/Package Manager) | ⚪ N/A (Distro handled) |

## 2. Identified Issues & limitations

Based on codebase analysis and `CHANGELOG.md`:

1.  **X11 Support**: Officially unsupported. The app works primarily on Wayland.
2.  **Peripheral Support**: Limited compared to Windows. Managing external ASUS mice/headsets is not as comprehensive.
3.  **UI Layout**: While functional, it lacks the "Control Center" dashboard feel of Armoury Crate (graphs, unified utilization views).
4.  **Scenario Profiles**: A major user-convenience feature is missing. Users cannot set "Turbo mode" to auto-engage when launching "Cyberpunk 2077".

## 3. Recommended Features to Add (The "Replica" Plan)

To make this a true "Replica" of the Armoury Crate experience on Linux, we should prioritize:

1.  **Scenario Profiles**:
    *   **Goal**: Allow users to define rules (e.g., "If App 'X' is in foreground -> Set Profile 'Turbo' + Aura 'Rainbow'").
    *   **Implementation**: A background watcher in `rog-control-center` or `asusd` that monitors active window/process.

2.  **Enhanced Dashboard**:
    *   **Goal**: A "Home" tab that looks like the Armoury Crate dashboard.
    *   **Content**: Real-time graphs for CPU/GPU usage, temperature, and RAM. Quick tiles for GPU Mode and Power Profile.

3.  **Game Visual / Screen Color**:
    *   **Goal**: Presets for screen color (FPS, RTS, Cinema).
    *   **Implementation**: Use `colord` or GPU gamma tables if possible, or just screen brightness/temp controls.

4.  **Audio Control Center**:
    *   **Goal**: A tab to control input/output volume and potentially noise cancellation (if using PipeWire/EasyEffects).
