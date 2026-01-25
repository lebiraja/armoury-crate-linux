# Armoury Crate Linux - Complete UI Redesign Plan

## Overview
Full application UI redesign with simulated glassmorphism, consistent theming across all pages, and new features inspired by Windows Armoury Crate.

## User Requirements
- **Style**: Simulated glassmorphism (layered transparency, no native blur)
- **Scope**: Full app redesign (all pages)
- **Reference Elements**: Sidebar grouping, detailed system metrics, real-time status
- **New Features**: Storage metrics, additional power modes (Turbo/Manual), fan speed visualization, real-time frequency monitoring

---

## Phase 1: Theme System Overhaul

### 1.1 Update `ui/theme.slint`

Add glassmorphism design tokens:

```slint
// Glassmorphism colors
out property <color> glass-bg: rgba(26, 29, 38, 0.7);        // Semi-transparent card bg
out property <color> glass-border: rgba(255, 255, 255, 0.1); // Subtle white border
out property <color> glass-highlight: rgba(255, 255, 255, 0.05); // Top edge highlight

// Enhanced shadows
out property <length> shadow-sm: 4px;
out property <length> shadow-md: 8px;
out property <length> shadow-lg: 16px;

// Consistent border radius (larger for glass effect)
out property <length> glass-radius: 16px;
```

### 1.2 Create `ui/widgets/glass_card.slint`

Reusable glassmorphism card component:
- Semi-transparent background with layered effect
- Subtle white top/left border for "light reflection"
- Soft drop shadow
- Rounded corners (16px)

### Files to Modify/Create
- **MODIFY**: `ui/theme.slint` - Add glass tokens
- **CREATE**: `ui/widgets/glass_card.slint` - Reusable glass container

---

## Phase 2: Sidebar Redesign

### 2.1 Grouped Navigation

Reorganize sidebar into logical groups like Windows Armoury Crate:

```
ARMOURY CRATE LINUX
──────────────────
System
  ├─ Dashboard
  ├─ System Control
  └─ Fan Curves

Customization
  ├─ Aura RGB
  ├─ Scenario Profiles
  └─ AniMe Matrix

Application
  ├─ Settings
  └─ About

[Quit App]
```

### 2.2 Enhanced Sidebar Styling
- Glass effect on sidebar background
- Group headers with subtle dividers
- Improved hover/active states with glow
- Icons for each menu item (emoji or custom)

### Files to Modify
- **MODIFY**: `ui/widgets/sidebar_gaming.slint` - Complete redesign
- **MODIFY**: `ui/main_window.slint` - Update sidebar integration

---

## Phase 3: Dashboard Redesign

### 3.1 New Dashboard Layout

```
┌─────────────────────────────────────────────────────────────┐
│ Dashboard                              Profile: [Balanced ▼] │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │     CPU      │  │     GPU      │  │   Fans & Power    │  │
│  │    ┌───┐     │  │    ┌───┐     │  │ CPU Fan: 6800 RPM │  │
│  │    │ 4%│     │  │    │ 0%│     │  │ GPU Fan: 6900 RPM │  │
│  │    └───┘     │  │    └───┘     │  │ Power: AC         │  │
│  │ 69°C · 1 GHz │  │ 39°C · 2W    │  │ TDP: 45W          │  │
│  │ Freq: 2.7GHz │  │ Freq: 300MHz │  │                   │  │
│  └──────────────┘  └──────────────┘  └───────────────────┘  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    Memory & Storage                   │  │
│  │  RAM: ████████████░░░░░ 6.9 / 15.2 GB (45%)          │  │
│  │  SSD: ████████░░░░░░░░░ 256 / 512 GB  (50%)          │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌────────────────────────┐  ┌────────────────────────┐   │
│  │    CPU Temperature     │  │    GPU Temperature     │   │
│  │         69°C           │  │         39°C           │   │
│  │    ═══════════════     │  │    ═══════════════     │   │
│  │    [Temp Graph]        │  │    [Temp Graph]        │   │
│  └────────────────────────┘  └────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 New Metrics to Add
- **CPU Frequency**: Real-time current frequency (from `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq`)
- **GPU Frequency**: Current GPU clock
- **Storage**: Disk usage for root partition
- **Power Draw**: Current TDP/power consumption
- **Voltage**: CPU/GPU voltage if available

### 3.3 Enhanced Gauge Component
- Glass effect on gauge cards
- Add frequency display below temperature
- Mini temperature history graph (last 60 seconds)

### Files to Modify/Create
- **MODIFY**: `ui/pages/dashboard.slint` - Complete redesign
- **MODIFY**: `ui/widgets/gauge.slint` - Add glass styling, frequency display
- **CREATE**: `ui/widgets/temp_graph.slint` - Mini temperature history
- **MODIFY**: `src/ui/setup_dashboard.rs` - Add new data sources

---

## Phase 4: System Control Redesign

### 4.1 Add Power Modes
Current: Quiet, Balanced, Performance
Add: **Turbo**, **Manual**

```
Operating Mode
┌─────┐ ┌─────────┐ ┌───────────┐ ┌───────┐ ┌────────┐
│Quiet│ │Balanced │ │Performance│ │ Turbo │ │ Manual │
└─────┘ └─────────┘ └───────────┘ └───────┘ └────────┘

Manual Mode Controls (shown when Manual selected):
├─ CPU Power Limit: [===|====] 45W
├─ GPU Power Limit: [===|====] 80W
└─ Fan Mode: [Auto ▼]
```

### 4.2 Glass Card Styling
- Apply glass effect to all section cards
- Consistent spacing and typography
- Enhanced toggle switches with glow

### Files to Modify
- **MODIFY**: `ui/pages/system.slint` - Add Turbo/Manual modes, glass styling
- **MODIFY**: `src/ui/setup_system.rs` - Wire up new power modes

---

## Phase 5: All Pages Glass Consistency

### 5.1 Pages to Update

| Page | Glass Elements | Notes |
|------|----------------|-------|
| Dashboard | All metric cards, gauges | Add new metrics |
| System Control | Profile cards, settings sections | Add Turbo/Manual |
| Aura RGB | Keyboard preview, color picker, mode cards | Already has good structure |
| Fan Curves | Graph container, preset buttons | Keep graph functionality |
| Settings | All toggle sections | Consistent cards |
| About | Info card, version section | Simple update |

### 5.2 Common Glass Card Pattern

```slint
component GlassCard inherits Rectangle {
    background: RogTheme.glass-bg;
    border-radius: RogTheme.glass-radius;
    border-width: 1px;
    border-color: RogTheme.glass-border;

    // Top highlight edge
    Rectangle {
        height: 1px;
        width: parent.width - 32px;
        x: 16px;
        y: 1px;
        background: RogTheme.glass-highlight;
        border-radius: 1px;
    }

    drop-shadow-color: rgba(0, 0, 0, 0.3);
    drop-shadow-blur: RogTheme.shadow-md;
    drop-shadow-offset-y: 4px;
}
```

---

## Phase 6: New Data Sources (Backend)

### 6.1 Additional System Metrics

| Metric | Source | File to Modify |
|--------|--------|----------------|
| CPU Frequency | `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` | `monitoring/mod.rs` |
| GPU Frequency | NVML or `/sys/class/drm/card*/gt_cur_freq_mhz` | `monitoring/mod.rs` |
| Disk Usage | `statvfs` or `sysinfo` crate | `monitoring/mod.rs` |
| Power Draw | `/sys/class/power_supply/` or NVML | `monitoring/mod.rs` |

### 6.2 Power Mode Backend

| Mode | Platform Profile | PPT Setting |
|------|------------------|-------------|
| Quiet | power-saver | Lowest TDP |
| Balanced | balanced | Default TDP |
| Performance | performance | High TDP |
| Turbo | performance + boost | Max TDP |
| Manual | Custom | User-defined PPT sliders |

### Files to Modify
- **MODIFY**: `src/monitoring/mod.rs` - Add CPU freq, disk, power metrics
- **MODIFY**: `src/ui/setup_system.rs` - Add Turbo/Manual mode logic

---

## Implementation Order

1. **Theme & Glass Components** (Foundation)
   - Update theme.slint with glass tokens
   - Create GlassCard component

2. **Sidebar Redesign** (Navigation)
   - Group navigation items
   - Apply glass styling

3. **Dashboard Overhaul** (Most visible page)
   - New layout with glass cards
   - Add CPU/GPU frequency display
   - Add storage metrics
   - Add temperature mini-graphs

4. **System Control Updates** (Power modes)
   - Add Turbo and Manual modes
   - Apply glass styling

5. **Other Pages** (Consistency)
   - Fan Curves - glass styling
   - Settings - glass cards
   - About - simple update

6. **Backend Data Sources** (New metrics)
   - CPU frequency monitoring
   - Disk usage monitoring
   - Power draw monitoring

---

## Critical Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `ui/theme.slint` | MODIFY | Add glass tokens |
| `ui/widgets/glass_card.slint` | CREATE | Reusable glass container |
| `ui/widgets/sidebar_gaming.slint` | MODIFY | Grouped navigation |
| `ui/pages/dashboard.slint` | MODIFY | New layout, glass styling |
| `ui/pages/system.slint` | MODIFY | Add Turbo/Manual modes |
| `ui/pages/fans.slint` | MODIFY | Glass styling |
| `ui/pages/settings.slint` | MODIFY | Glass styling |
| `ui/widgets/gauge.slint` | MODIFY | Add frequency, glass styling |
| `src/monitoring/mod.rs` | MODIFY | New metrics |
| `src/ui/setup_dashboard.rs` | MODIFY | Wire new metrics |
| `src/ui/setup_system.rs` | MODIFY | Turbo/Manual modes |

---

## Visual Style Guide

### Color Usage
- **Primary Accent**: `#FF0055` (ROG Red) - Selected states, primary actions
- **Secondary Accent**: `#00D9FF` (Cyan) - CPU metrics, info
- **Tertiary Accent**: `#B366FF` (Purple) - GPU metrics, Aura
- **Warning**: `#FF6B00` (Orange) - High temps, warnings
- **Glass Background**: `rgba(26, 29, 38, 0.7)` - Cards
- **Glass Border**: `rgba(255, 255, 255, 0.1)` - Subtle edges

### Typography
- **Page Titles**: 32px, Bold, White
- **Section Headers**: 20px, SemiBold, White
- **Body Text**: 14px, Regular, Secondary gray
- **Metrics Values**: 48px, Bold, Accent color

### Spacing
- **Card Padding**: 24px
- **Section Gap**: 16px
- **Element Spacing**: 8px

### Border Radius
- **Cards**: 16px (glass-radius)
- **Buttons**: 8px
- **Inputs**: 8px
- **Gauges**: 50% (circular)

---

## Verification Plan

1. **Visual Consistency Check**
   - All cards use GlassCard component
   - Consistent spacing across pages
   - Color accents match theme

2. **Functionality Test**
   - Sidebar navigation works
   - Power modes switch correctly
   - New metrics display values

3. **Performance Check**
   - Animations run smoothly (60fps)
   - No lag when switching pages
   - Memory usage stays stable

4. **Build Verification**
   ```bash
   cargo check -p armoury-crate-linux
   cargo run --bin armoury-crate-linux
   ```

---

## Estimated Effort by Phase

| Phase | Description | Files | Complexity |
|-------|-------------|-------|------------|
| 1 | Theme & Glass Components | 2 | Low |
| 2 | Sidebar Redesign | 2 | Medium |
| 3 | Dashboard Overhaul | 4 | High |
| 4 | System Control | 2 | Medium |
| 5 | Other Pages | 3 | Low |
| 6 | Backend Data | 3 | Medium |

**Total Files to Modify/Create**: ~16 files
