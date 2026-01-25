# Aura RGB UI Improvement Plan

**Date:** January 25, 2026  
**Status:** Planning Phase  
**Goal:** Enhance the Aura RGB page with modern color picker and improved widget design

---

## Current State Analysis

### Existing Components
1. **BrightnessButton** - 4 buttons (Off/Low/Med/High)
   - Fixed size: 60x48px
   - Basic styling with accent-cyan
   - Functional but could use more visual feedback

2. **ModeCard** - 12 LED effect cards
   - Size: 100x80px
   - Shows mode name and color preview
   - Grid layout (3 rows x 4 columns)
   - Good but preview could be more dynamic

3. **ColorSlider** - 3 RGB sliders
   - Linear gradient backgrounds
   - Basic handle design
   - **Major limitation:** Not intuitive for color selection

4. **ColorSwatch** - 6 preset colors
   - 40x40px buttons
   - Basic border indication for selection
   - Limited color options

### Current Issues
- RGB sliders are not intuitive for choosing colors
- Color preview is small and separate from controls
- Limited preset colors (only 6)
- Widget animations are minimal
- No HSV/HSL color space support (only RGB)
- Brightness buttons could be more visual

---

## Proposed Improvements

### 1. Color Wheel Component (Priority: HIGH)

**Replace RGB sliders with a modern color wheel picker**

#### Features:
- **Circular HSV color wheel** with saturation/value selection
- Large, interactive design (~200x200px minimum)
- Real-time color preview in center
- Smooth dragging interaction
- Optional RGB value display below wheel
- Hue ring on outer circle
- Saturation-Value square in center

#### Implementation Details:
```slint
component ColorWheel inherits Rectangle {
    in-out property <float> hue: 0.0;          // 0-360
    in-out property <float> saturation: 1.0;   // 0-1
    in-out property <float> value: 1.0;        // 0-1
    callback color-changed(int, int, int);     // RGB output
    
    // Size: 240x240px for good usability
    // Center: Color preview with current RGB
    // Ring: Hue selector (360 degrees)
    // Inner area: Saturation-Value picker
}
```

#### Benefits:
- More intuitive than sliders
- Visual color space representation
- Faster color selection
- Professional appearance
- Better for gaming aesthetics

---

### 2. Enhanced Brightness Control

**Improve brightness buttons with visual indicators**

#### Changes:
- Add LED icon with different glow intensities
- Animate transitions between levels
- Show percentage (0%, 33%, 66%, 100%)
- Larger touch targets (70x56px)
- Add glow effect when selected
- Subtle hover animations

#### Design:
```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   💡 OFF    │  │  💡 LOW     │  │  💡 MED     │  │  💡 HIGH    │
│     0%      │  │    33%      │  │    66%      │  │    100%     │
└─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘
```

---

### 3. Enhanced Mode Cards

**Make LED effect cards more engaging**

#### Improvements:
- **Animated previews** showing actual effect pattern
- Larger cards: 110x90px
- Better hover effects with scale transition
- Show speed indicator on animated modes
- Add subtle border glow matching effect color
- Tooltip on hover with mode description

#### Animation Examples:
- Static: Solid color block
- Breathe: Pulsing opacity animation
- Rainbow: Animated gradient sweep
- Wave: Moving wave pattern
- etc.

---

### 4. Speed Control Enhancement

**Visual speed selector with animation preview**

#### Design:
- Replace text buttons with visual speed bars
- Show animation speed graphically
- Icons: 🐢 Slow | ⚡ Medium | 🚀 Fast
- Add subtle animation to selected speed
- Larger touch areas

---

### 5. Enhanced Color Presets

**Expand and improve preset color selection**

#### Changes:
- Increase from 6 to 12 preset colors
- Add 2 rows of swatches
- Larger swatches: 48x48px
- Add color names on hover
- Gaming-themed color palette:
  - ROG Red (#FF0055)
  - Cyan (#00D9FF)  
  - Purple (#B366FF)
  - Green (#00FF66)
  - Orange (#FFAA00)
  - White (#FFFFFF)
  - Blue (#4444FF)
  - Yellow (#FFFF00)
  - Pink (#FF66FF)
  - Lime (#AAFF00)
  - Teal (#00FFAA)
  - Gold (#FFD700)

---

### 6. Overall Layout Improvements

**Better visual hierarchy and spacing**

#### Changes:
1. **Section Cards:**
   - Add subtle background gradients
   - Better shadow effects
   - Consistent padding (24px)

2. **Spacing:**
   - Increase section spacing to 32px
   - Better card internal spacing
   - Proper alignment of elements

3. **Visual Feedback:**
   - Smooth transitions (300ms)
   - Hover states for all interactive elements
   - Click animations (scale down slightly)
   - Color change animations

4. **Responsive Design:**
   - Better layout on different window sizes
   - Flexible grid for mode cards
   - Collapsible sections if needed

---

## Implementation Plan

### Phase 1: Color Wheel Component (2-3 hours)
- [ ] Create ColorWheel component in separate file
- [ ] Implement HSV to RGB conversion functions
- [ ] Add circular hue selector
- [ ] Add saturation-value picker area
- [ ] Test color accuracy
- [ ] Add smooth drag interactions

### Phase 2: Widget Enhancements (2-3 hours)
- [ ] Enhance BrightnessButton with icons and percentages
- [ ] Improve ModeCard animations
- [ ] Redesign speed controls
- [ ] Expand color presets

### Phase 3: Layout & Polish (1-2 hours)
- [ ] Update main page layout
- [ ] Add transition animations
- [ ] Improve section cards
- [ ] Test all interactions
- [ ] Fine-tune spacing and sizing

### Phase 4: Backend Integration (1 hour)
- [ ] Connect ColorWheel to Rust backend
- [ ] Test color conversions
- [ ] Verify all callbacks work
- [ ] Test with actual hardware

---

## Technical Considerations

### Color Space Conversion
Need to implement HSV ↔ RGB conversion in Slint:
```slint
// Utility functions needed:
function hsv_to_rgb(h: float, s: float, v: float) -> {r: int, g: int, b: int}
function rgb_to_hsv(r: int, g: int, b: int) -> {h: float, s: float, v: float}
```

### Math Operations
- Angle calculations for hue wheel (0-360°)
- Distance calculations for saturation-value
- Coordinate transformations (Cartesian ↔ Polar)

### Touch/Mouse Handling
- Precise drag tracking
- Bounds checking
- Smooth visual updates
- Multi-touch considerations

---

## Expected Outcomes

### User Experience
- ✅ Faster color selection
- ✅ More intuitive controls
- ✅ Better visual feedback
- ✅ Modern, gaming-aesthetic UI
- ✅ Smoother animations

### Visual Quality
- ✅ Professional appearance
- ✅ Consistent with gaming theme
- ✅ Better use of screen space
- ✅ Improved contrast and readability

### Technical
- ✅ Maintainable component structure
- ✅ Reusable color wheel component
- ✅ Clean callback architecture
- ✅ Performance optimized

---

## References

### Design Inspiration
- ASUS ROG Armoury Crate (official app)
- Modern color pickers (Figma, Photoshop)
- Gaming peripheral software (Razer Synapse, Corsair iCUE)

### Color Wheel Resources
- HSV color space documentation
- Circle geometry for color picking
- Touch interaction patterns

---

## Next Steps

1. **Review this plan** and get approval
2. **Start with ColorWheel component** as it's the biggest change
3. **Iteratively improve** other widgets
4. **Test thoroughly** with real hardware
5. **Get user feedback** and iterate

---

**Estimated Total Time:** 6-9 hours  
**Complexity:** Medium (color math) to High (UI polish)  
**Priority:** High (major UX improvement)
