# Python TUI Rewrite: Feature Requirements

This document outlines all the features exposed by the `asusd` ecosystem that must be implemented in the new Python-based Terminal User Interface (TUI). Using Python and a library like `Textual` or `Urwid`, the application will communicate with the existing `asusd` D-Bus API to manage the hardware.

## 1. System Monitoring Dashboard
The TUI should feature a live dashboard mimicking the "Armoury Crate" home page.
*   **CPU Metrics**: Live temperature, utilization, and power draw (TDP/Package power).
*   **GPU Metrics**: 
    *   NVIDIA monitoring (utilizing NVML bindings like `pynvml` or daemon metrics).
    *   AMD / Integrated GPU monitoring.
    *   GPU Temperature, VRAM usage, Core usage, and Wattage.
*   **Fans & Thermal**: Real-time RPM for CPU, GPU, and system fans.
*   **Battery Status**: Current charge percentage, discharging/charging wattage, and status.

## 2. Power & Performance Management
*   **Platform Power Profiles**: Ability to switch between the core power profiles:
    *   `Balanced`
    *   `Performance`
    *   `Quiet`
*   **Battery Charge Limiting**: A slider or input field to set the maximum battery charge limit (%) to preserve battery health.
*   **Fan Curves**: 
    *   An interface to view and apply custom fan curves for the CPU and GPU.
    *   Must allow setting pairs of `Temperature: Percentage` (e.g., `30c:0%, 50c:10%, 80c:55%`).
*   **GPU Mux Switch (Ultimate/Standard/Eco)**: Allow the user to toggle the GPU MUX switch (requires reboot, typically toggles the dGPU enablement).

## 3. Aura Keyboard & RGB Lighting
*   **Lighting Modes (System)**: Dropdown to select core hardware lighting effects natively supported by the EC:
    *   Static, Breathe, Color Cycle, Rainbow, Star, Rain, Highlight, Pulse, etc.
*   **Color Picker**: Ability to specify hexadecimal/RGB values for modes that support custom colors (like Static or Breathe).
*   **Brightness Control**: A slider/input from 0.0 to 1.0 (or 0% to 100%) to control global LED brightness.
*   **Advanced / User Aura**:
    *   Triggering `asusd-user` custom profiles (Per-key / multizone configs).
    *   *Note: Building a fully visual per-key RGB editor in a TUI might be complex; it is recommended to allow users to load predefined `.ron` profiles from `~/.config/rog/`.*

## 4. AniMe Matrix Controller (For Supported Models)
*   **Sequence Selection**: Allow selecting active animations for:
    *   `System` (Continuous loop)
    *   `Boot` (Played on startup)
    *   `Wake` (Played on resume from suspend)
    *   `Shutdown`
*   **Configuration Modifiers**:
    *   Global AniMe Brightness control.
    *   Image/GIF scaling, angle (rotation), and X/Y translation adjustments.
*   **Media Support**: Ability to browse the filesystem to assign GIFs (`ImageAnimation`) or PNGs (`Image`) to the lid display.

## 5. Hardware & BIOS Settings
*   **POST Sound Toggle**: An on/off switch for the ASUS sword swoosh sound that plays during BIOS POST.
*   **Panel Overdrive**: Toggle for the screen overdrive feature (if supported by the panel module).

## 6. Scenario Profiles (Automation)
*   **Process Watcher**: Allow users to define "Scenarios" where specific power profiles and Aura modes are automatically applied when a specific Linux process or window is focused/running.
*   *Note: In Wayland, tracking focused windows is difficult, so tracking running processes (e.g., via `psutil`) is the standard workaround.*

## 7. TUI Architecture Considerations
*   **D-Bus Integration**: The TUI must not run as root. It must use the `dbus` Python module to call methods on `xyz.ljones.Asusd` on the System Bus (and `xyz.ljones.AsusdUser` on the Session Bus).
*   **Async Interface**: Because monitoring requires continuous polling and D-Bus signals are asynchronous, the Python TUI should rely on `asyncio`. Frameworks like `Textual` natively support `asyncio` loops.
*   **Responsive Layout**: The TUI should dynamically split into tabs or panes:
    1.  *Dashboard* (Monitoring)
    2.  *Device Settings* (GPU, Charge, POST)
    3.  *Aura* (RGB Settings)
    4.  *AniMe Matrix*
    5.  *Scenarios* (Automation rules)
