# Sidereal UI Design Guide

**Status:** Active Design System  
**Date:** 2026-02-20  
**Audience:** Frontend developers, AI agents, UI contributors

## 1. Design Philosophy

Sidereal uses a **dark space-themed aesthetic** that emphasizes:
- **Clarity and readability** over decorative elements
- **Consistent spacing and hierarchy** for predictable UX
- **Subdued colors with strategic accents** to reduce eye strain during long sessions
- **Modern, minimal design** appropriate for a professional space sim

## 2. Color Palette

### Base Colors (Backgrounds)

```rust
// Primary background (outer space)
Color::srgb(0.03, 0.04, 0.08)  // Very dark blue-black

// Panel background (UI surfaces)
Color::srgba(0.06, 0.08, 0.12, 0.92)  // Dark blue-gray, semi-transparent

// Backdrop overlay (modals)
Color::srgba(0.0, 0.0, 0.0, 0.7)  // Black semi-transparent
```

### Interactive Element Colors

```rust
// Default button/input background
Color::srgba(0.15, 0.2, 0.3, 0.9)

// Hovered button/input
Color::srgba(0.2, 0.25, 0.35, 0.9)

// Active/pressed button
Color::srgb(0.16, 0.38, 0.74)  // Brighter blue

// Focused input field
Color::srgba(0.12, 0.15, 0.21, 0.98)
```

### Text Colors

```rust
// Primary text (high emphasis)
Color::srgb(0.85, 0.92, 1.0)  // Slightly blue-tinted white

// Secondary text (medium emphasis)
Color::srgb(0.85, 0.9, 0.95)

// Tertiary text (low emphasis)
Color::srgba(0.83, 0.89, 0.95, 0.95)

// Status/success text
Color::srgb(0.8, 0.95, 0.9)  // Slightly green-tinted
```

### Severity/State Colors

```rust
// Error state
Color::srgb(1.0, 0.4, 0.35)  // Red
Border: Color::srgba(0.8, 0.2, 0.2, 0.8)

// Warning state
Color::srgb(1.0, 0.8, 0.3)  // Orange-yellow
Border: Color::srgba(0.8, 0.6, 0.2, 0.8)

// Info state
Color::srgb(0.6, 0.8, 1.0)  // Blue
Border: Color::srgba(0.3, 0.5, 0.7, 0.8)
```

### Border Colors

```rust
// Default border (subtle)
Color::srgba(0.2, 0.3, 0.45, 0.8)

// Focused/highlighted border
Color::srgba(0.3, 0.4, 0.55, 0.9)

// Hover border
Color::srgba(0.4, 0.5, 0.65, 1.0)
```

## 3. Typography

### Fonts

**Primary Font Stack:**
- Bold: `data/fonts/FiraSans-Bold.ttf` (headers, buttons, emphasis)
- Regular: `data/fonts/FiraSans-Regular.ttf` (body text, inputs)

### Font Sizes

```rust
// Large title (application name, major sections)
font_size: 42.0

// Section titles
font_size: 28.0

// Subsection headers
font_size: 18.0

// Body text, inputs
font_size: 16.0

// Small text, button labels
font_size: 13.0
```

## 4. Spacing and Layout

### Standard Spacing Units

```rust
// Component padding (buttons, inputs, panels)
padding: UiRect::all(Val::Px(28.0))  // Large panels
padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0))  // Buttons

// Gap between elements
row_gap: Val::Px(18.0)  // Related elements
row_gap: Val::Px(14.0)  // Tight grouping

// Margins
margin: UiRect::top(Val::Px(8.0))  // Button spacing
margin: UiRect::bottom(Val::Px(8.0))  // Section spacing
```

### Border Radius

```rust
// Large panels, dialogs
border_radius: BorderRadius::all(Val::Px(12.0))

// Buttons, inputs
border_radius: BorderRadius::all(Val::Px(6.0))
```

### Border Width

```rust
// Panel borders
border: UiRect::all(Val::Px(2.0))

// Input/button borders
border: UiRect::all(Val::Px(1.0))
```

## 5. Component Patterns

### 5.1 Modal Dialogs

**Location:** `bins/sidereal-client/src/dialog_ui.rs`

**Usage:**

```rust
use crate::dialog_ui::DialogQueue;

fn my_system(mut dialog_queue: ResMut<DialogQueue>) {
    // Error dialogs (red theme, for failures)
    dialog_queue.push_error(
        "Operation Failed",
        "Detailed error message with context.\n\nTroubleshooting hints go here."
    );

    // Warning dialogs (yellow theme, for caution)
    dialog_queue.push_warning(
        "Potential Issue",
        "Something needs attention but isn't blocking."
    );

    // Info dialogs (blue theme, for notifications)
    dialog_queue.push_info(
        "Success",
        "Operation completed successfully."
    );
}
```

**Design Specifications:**

```rust
// Dialog panel
width: Val::Px(600.0)
max_width: Val::Percent(90.0)
padding: UiRect::all(Val::Px(28.0))
border: UiRect::all(Val::Px(2.0))
border_radius: BorderRadius::all(Val::Px(12.0))
row_gap: Val::Px(18.0)
background: Color::srgba(0.06, 0.08, 0.12, 0.96)

// Backdrop
background: Color::srgba(0.0, 0.0, 0.0, 0.7)
z_index: 1000

// OKAY button
width: Val::Px(120.0)
height: Val::Px(44.0)
margin: UiRect::top(Val::Px(8.0))
```

**Behavior:**
- Dialogs queue if multiple are pushed (shown one at a time)
- Dismiss via: Click OKAY button, press Enter, or press Escape
- Backdrop click does NOT dismiss (intentional - requires acknowledgment)
- Dialogs persist until explicitly dismissed (won't auto-hide)

**When to Use:**
- ✅ **Use dialogs for:** Errors, critical warnings, operations requiring acknowledgment
- ❌ **Don't use for:** Success messages (use status text), real-time updates, frequent notifications

### 5.2 Auth/Login Panels

**Location:** `bins/sidereal-client/src/auth_ui.rs`

**Design Specifications:**

```rust
// Auth panel
width: Val::Px(540.0)
padding: UiRect::all(Val::Px(30.0))
border: UiRect::all(Val::Px(2.0))
border_radius: BorderRadius::all(Val::Px(12.0))
row_gap: Val::Px(14.0)
background: Color::srgba(0.06, 0.08, 0.12, 0.92)
border_color: Color::srgba(0.2, 0.3, 0.45, 0.8)

// Animated backdrop (subtle pulse)
// Pulses between ~0.03 and ~0.045 over sine wave
```

**Input Fields:**

```rust
width: Val::Px(480.0)
height: Val::Px(42.0)
padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0))
border_radius: BorderRadius::all(Val::Px(6.0))
background: Color::srgba(0.08, 0.1, 0.14, 0.95)
```

**Submit Button:**

```rust
width: Val::Px(480.0)
height: Val::Px(46.0)
margin: UiRect::top(Val::Px(12.0))
border_radius: BorderRadius::all(Val::Px(6.0))
background: Color::srgba(0.18, 0.3, 0.54, 0.95)
hover: Color::srgb(0.2, 0.35, 0.65)
active: Color::srgb(0.16, 0.38, 0.74)
```

**Flow Switch Buttons:**

```rust
height: Val::Px(34.0)
padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0))
border_radius: BorderRadius::all(Val::Px(6.0))
background: Color::srgba(0.18, 0.2, 0.26, 0.85)
```

### 5.3 HUD / In-Game UI

**Status Text:**

```rust
font_size: 18.0
color: Color::srgb(0.8, 0.95, 0.9)  // Slightly green-tinted for "active"
```

**Positioning:**
- HUD elements use absolute positioning with `Val::Px()` offsets
- Top-left for status/telemetry
- Top-right for system indicators
- Bottom-right for controls/help text
- Center-screen for critical alerts only

## 6. State Management and Cleanup

### State-Scoped Entities

Use `DespawnOnExit(state)` for UI that should be cleaned up on state transitions:

```rust
use bevy::state::state_scoped::DespawnOnExit;

commands.spawn((
    MyUiComponent,
    DespawnOnExit(ClientAppState::Auth),  // Cleaned up when leaving Auth state
));
```

### Manual Despawning

For dialogs and temporary UI that dismiss via user action:

```rust
// Despawn just the entity (children remain orphaned - not recommended)
commands.entity(entity).despawn();

// For hierarchical UI, you must track and despawn children manually
// or structure UI to avoid deep hierarchies needing recursive despawn
```

## 7. Animation and Effects

### Cursor Blink

```rust
Timer::from_seconds(0.5, TimerMode::Repeating)
// Toggle visibility on timer finish
```

### Background Pulse (Auth Screen)

```rust
let t = time.elapsed_secs();
let pulse = 0.03 + 0.015 * (t * 0.5).sin().abs();
Color::srgb(pulse, pulse * 1.2, pulse * 1.8)
```

### Hover Transitions

- Use `Changed<Interaction>` queries to detect hover/press state
- Apply color changes immediately (no lerp - instant feedback)
- Subtle brightness increase on hover (~20% brighter)

## 8. Accessibility Guidelines

### Contrast Ratios

- Ensure text-to-background contrast ratio ≥ 4.5:1 for body text
- Error/warning text should be ≥ 3:1 for large text (18pt+)
- Border contrast should be sufficient for focus indicators

### Keyboard Navigation

- All interactive elements must support keyboard equivalents
- Modal dialogs: Enter/Escape to dismiss
- Auth forms: Tab to cycle focus, Enter to submit
- No keyboard traps (user can always escape modals/menus)

### Focus Indicators

- Focused inputs change background color (not just border)
- Cursor blink indicates active text input field
- Hovered buttons show clear visual feedback

## 9. Implementation Guidelines for Agents

### When Adding New UI Components

1. **Match existing color palette** - Don't introduce new colors without design review
2. **Use standard spacing units** - 6px, 8px, 12px, 14px, 18px, 28px, 30px
3. **Follow component patterns** - Dialogs, panels, buttons should match existing specs
4. **State cleanup** - Use `DespawnOnExit` for state-scoped UI
5. **Keyboard support** - All interactive UI needs keyboard accessibility

### Error Handling UI Pattern

```rust
// ✅ CORRECT: Show persistent error dialog
dialog_queue.push_error(
    "Clear Title",
    format!(
        "User-friendly summary.\n\n\
         Details: {err}\n\n\
         Common causes:\n\
         • Specific cause 1\n\
         • Specific cause 2\n\
         • Troubleshooting hint"
    )
);

// ❌ WRONG: Just log to console or flash status text
eprintln!("Error: {err}");  // User never sees this
session.status = format!("Error: {err}");  // Disappears too fast
```

### Testing UI Changes

1. Test both mouse and keyboard interaction paths
2. Test at different window sizes (dialogs should respect `max_width: Val::Percent(90.0)`)
3. Verify focus indicators are visible
4. Check that UI cleans up properly on state transitions
5. Test error scenarios trigger dialogs (not just console logs)

## 10. Future UI Components (Planned)

These components don't exist yet but should follow this guide when implemented:

- **Confirmation Dialogs** (Yes/No choices)
- **Progress Indicators** (loading bars, spinners)
- **Tooltips** (hover hints for buttons/controls)
- **Context Menus** (right-click actions)
- **Notification Toasts** (non-blocking, auto-dismiss after 3-5s)
- **Settings Panels** (sliders, toggles, dropdowns)
- **HUD Overlays** (ship status, minimap, target info)
- **Chat/Log Window** (scrollable text feed)

### Planned Component Specs

**Confirmation Dialog** (extend existing dialog system):
```rust
dialog_queue.push_confirmation(
    "Confirm Action",
    "Are you sure you want to do this?",
    |confirmed| {
        if confirmed {
            // Execute action
        }
    }
);
```

**Toast Notification** (non-blocking, auto-dismiss):
```rust
toast_queue.push_success("Operation completed successfully");
toast_queue.push_warning("Low fuel warning");
// Auto-dismiss after 3s, stack vertically in bottom-right
```

## 11. File Locations

- **Dialog System:** `bins/sidereal-client/src/dialog_ui.rs`
- **Auth UI:** `bins/sidereal-client/src/auth_ui.rs`
- **Main Client:** `bins/sidereal-client/src/main.rs`
- **Fonts:** `data/fonts/FiraSans-*.ttf`
- **This Guide:** `docs/ui_design_guide.md`

## 12. References

- **Design Document:** `docs/sidereal_design_document.md` (overall architecture)
- **Implementation Checklist:** `docs/sidereal_implementation_checklist.md` (UI task tracking)
- **Agent Guidelines:** `AGENTS.md` (contributor rules)

---

**Changelog:**
- 2026-02-20: Initial version documenting auth UI and dialog system patterns
