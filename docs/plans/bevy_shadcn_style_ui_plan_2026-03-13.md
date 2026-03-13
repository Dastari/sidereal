# Sidereal Native UI Kit Plan

**Status:** Approved implementation direction  
**Date:** 2026-03-13  
**Audience:** Native client/UI contributors

Update note (2026-03-13):

- We have decided to build a **native `sidereal-ui` kit** for the Bevy client.
- The kit will be inspired by `thegridcn-ui` and should reproduce as many of its useful, non-3D components and visual patterns as accurately as practical inside Bevy.
- We are **not** adopting the React runtime, DOM/CSS implementation, or any of the 3D showcase components from `thegridcn-ui`.
- Local clone review under `~/dev/thegridcn-ui` confirmed the project is split into:
  - `src/components/ui/*`: standard shadcn-style DOM components driven by class variants and Radix primitives.
  - `src/components/thegridcn/*`: Tron-flavored browser components, many marked `"use client"`, with some visual modules rendered through `@react-three/fiber`.
- The `sidereal-ui` theming system should use a **semantic token map** at the same granularity as shadcn/Tailwind themes:
  - `background`
  - `foreground`
  - `card`
  - `popover`
  - `primary`
  - `secondary`
  - `accent`
  - `muted`
  - `border`
  - `input`
  - `ring`
  - severity/intent colors and any Sidereal-specific additions
- Theme values should be authored canonically in **OKLCH**. Bevy supports Oklch/Oklcha color types, so the runtime can keep theme data in that perceptual color space and convert only when required.
- The initial font direction should match `thegridcn-ui`:
  - `Rajdhani` for primary sans/body UI
  - `Orbitron` for display/header treatment
  - `Geist Mono` for mono/telemetry/diagnostic text
- The initial theme set should be imported from `thegridcn-ui`'s current named themes:
  - `tron`
  - `ares`
  - `clu`
  - `athena`
  - `aphrodite`
  - `poseidon`
- Layout composition should be native Bevy. Bevy already supports hierarchical nesting plus Flexbox and Grid layout, so `sidereal-ui` should expose ergonomic wrappers around those capabilities rather than inventing a parallel layout engine.
- `sidereal-ui` must be designed for heavy Lua-driven presentation authoring. Future mission/dialogue/quest flows should be able to spawn scripted dialogs, portraits/avatars, progressive text reveal, skip-to-end behavior, and choice-driven branching from Lua-authored content without requiring bespoke Rust UI for each narrative sequence.

## 1. Current Sidereal UI Baseline

The in-game/native client currently renders UI directly with `bevy_ui` nodes and ECS systems:

- auth screen: `bins/sidereal-client/src/runtime/auth_ui.rs`
- dialog system: `bins/sidereal-client/src/runtime/dialog_ui.rs`
- loading screens: `bins/sidereal-client/src/runtime/asset_loading_ui.rs`, `bins/sidereal-client/src/runtime/world_loading_ui.rs`
- pause/in-world HUD/tactical UI: `bins/sidereal-client/src/runtime/pause_menu.rs`, `bins/sidereal-client/src/runtime/ui.rs`, `bins/sidereal-client/src/runtime/scene_world.rs`
- plugin wiring: `bins/sidereal-client/src/runtime/plugins/ui_plugins/`

The current design language is already documented in `docs/ui_design_guide.md`, but the implementation is mostly hand-authored `Node` trees with repeated colors, sizes, borders, and hover/pressed state logic.

## 2. What `thegridcn-ui` Actually Is

The referenced library is a web UI/design-system project built around web assumptions:

- Next.js 16 / React 19 app structure
- Tailwind CSS 4 styling
- Radix primitives
- animation and decorative effects built with browser/web stack tools
- web-oriented package/runtime model rather than Bevy ECS rendering

Representative examples from the local clone:

- `src/components/ui/button.tsx` builds variants with `class-variance-authority` and applies Tailwind utility classes to a DOM `<button>`.
- `src/components/thegridcn/text-input.tsx` is a DOM `<input>` with Tailwind/CSS-driven focus, placeholder, and error styling.
- `src/components/thegridcn/grid.tsx` uses `@react-three/fiber` and `three` shaders for the animated grid scene.

Primary references:

- Repo: <https://github.com/educlopez/thegridcn-ui>
- README: <https://raw.githubusercontent.com/educlopez/thegridcn-ui/main/README.md>
- package manifest: <https://raw.githubusercontent.com/educlopez/thegridcn-ui/main/package.json>

That means it is not a portable component library we can directly "mount" into a Bevy app. It assumes DOM/CSS, browser layout, and React state/event handling.

## 3. Decision

Sidereal will keep **Bevy-native UI rendering** and implement a first-party `sidereal-ui` kit on top of it.

The goal is not to make "something vaguely inspired by shadcn". The goal is to build a reusable Bevy-native component system that:

1. adopts `thegridcn-ui`'s strongest design ideas,
2. imports its theme vocabulary and fonts where licensing/distribution allows,
3. reproduces its most useful non-3D components as faithfully as practical,
4. remains fully native to Sidereal's Bevy runtime and native/WASM architecture.

This means:

- no embedded browser/webview path for core UI,
- no React runtime in the client,
- no Three.js/React Three Fiber adoption,
- no attempt to import Tailwind classes directly,
- yes to native Bevy components, native Bevy layout, and native Bevy theming tokens.

## 3.1 Where `bevy_egui` fits

`bevy_egui` is a legitimate option in 2026 and now targets Bevy 0.18. `egui_shadcn` also exists as a shadcn-inspired egui component crate.

Primary references:

- `bevy_egui` changelog / Bevy 0.18 support: <https://docs.rs/crate/bevy_egui/latest/source/CHANGELOG.md>
- `bevy_egui` crate metadata / feature surface: <https://docs.rs/crate/bevy_egui/latest/source/Cargo.toml.orig>
- `egui_shadcn` crate docs: <https://docs.rs/egui-shadcn/latest/egui_shadcn/>

What this means for Sidereal:

- acceptable use:
  - debug panels
  - internal operator tools
  - dev console-adjacent utilities
  - temporary editor/workbench overlays
- not the default recommendation:
  - auth/login flow
  - pause/menu screens intended to match the documented Bevy design system
  - in-world HUD/nameplates/tactical overlays that already depend on screen-space/world-space Bevy rendering and ECS ownership

Rationale:

- `egui` is immediate-mode and rebuilds every frame, which is a good fit for tooling but a weaker fit for highly integrated game HUD structure.
- Sidereal already has a Bevy-native design guide and a Bevy-native UI implementation baseline.
- the current client has no `bevy_egui` integration today, so adopting it broadly would be a stack change, not just a styling change.
- the strongest `bevy_egui` benefits are speed of tool development and inspector-style workflows, not deep integration with the existing HUD/camera/render-layer model.

For the purposes of this plan, `bevy_egui` is explicitly **not** the chosen foundation for `sidereal-ui`.

## 4. Why an Actual React Library Is the Wrong Integration Target

### 4.1 Runtime mismatch

Sidereal's client is a Bevy 0.18 application, not a DOM app. The client depends on Bevy for rendering, ECS state, input, cameras, and platform packaging. The actual React library would require either:

- a browser DOM layered over the game canvas in WASM and a native webview on desktop, or
- a full reimplementation of the React components in Rust anyway.

The first option creates two UI runtimes. The second option proves we should just build the Bevy-native component library directly.

### 4.2 Native/WASM parity cost

Per current project constraints, we must avoid native-only architecture that makes later WASM recovery harder. A native embedded webview would diverge sharply from browser/WASM behavior and introduce a second platform boundary for input, focus, text entry, accessibility, asset loading, and auth/session messaging.

### 4.3 Input/focus integration becomes worse

The current client already manages keyboard/game input, pause state, overlay cameras, and state-scoped UI inside ECS. A DOM/webview overlay would force us to bridge:

- keyboard focus ownership,
- pointer capture,
- IME/text input,
- modal stacking,
- pause/menu state,
- transport/auth state exposure into JS.

That is extra infrastructure with little gameplay value.

### 4.4 The current problem is abstraction, not raw capability

Bevy already gives us the native primitives we need for most of the visual language:

- layout and interaction via `Node`, `Button`, `Interaction`
- borders, radius, outlines, shadows
- text/font styling
- render layering and cameras
- custom UI materials for exceptional surfaces/effects

Primary references:

- `Node`: <https://docs.rs/bevy/latest/bevy/ui/struct.Node.html>
- `BoxShadow`: <https://docs.rs/bevy/latest/bevy/ui/struct.BoxShadow.html>
- `Outline`: <https://docs.rs/bevy/latest/bevy/ui/struct.Outline.html>
- `UiMaterial`: <https://docs.rs/bevy/latest/bevy/ui/trait.UiMaterial.html>

The missing piece is a reusable component/theme layer on top of these primitives.

The local clone reinforces that conclusion:

- the variant API is portable in spirit,
- the DOM/CSS implementation is not,
- the Three.js showcase pieces should be treated as inspiration for Bevy-native materials/effects rather than code we can directly reuse.

## 5. Adopted Architecture

## 5.1 New Bevy-native UI kit

Add a dedicated Bevy-native UI kit instead of continuing to hand-author raw screen trees.

Recommended location:

- preferred: new workspace crate `crates/sidereal-ui`
- acceptable first step if we want lower churn: `bins/sidereal-client/src/runtime/ui_kit/`

This should be treated as the target shape, not a tentative preference. A crate favors:

- it keeps client app wiring thinner,
- it isolates reusable UI primitives from game-specific screen logic,
- it keeps native/WASM compilation unified under one Rust surface.

## 5.2 Design goals

The UI kit should emulate the *authoring ergonomics* and *semantic theme model* of shadcn/thegridcn, not React itself.

Required traits:

- token-driven theme resource with OKLCH-authored semantic values
- small set of reusable primitives
- explicit style variants (`default`, `secondary`, `destructive`, `ghost`)
- explicit sizes (`sm`, `md`, `lg`)
- ECS-first interaction state
- no browser runtime dependency
- no native-only platform branch
- faithful reproduction of selected thegridcn components where they map cleanly to Bevy
- reusable nesting/composition so screens can be assembled from components instead of hand-written node trees

## 5.3 Theme system

The theme system should be a first-class product feature, not an implementation detail.

Requirements:

- one canonical semantic token schema shared across all `sidereal-ui` components
- token definitions stored in OKLCH
- theme switching by named theme ID
- straightforward creation of new themes without editing every component
- no raw per-screen color literals except in narrowly justified visual effects

Minimum token categories:

- `background`
- `foreground`
- `card`
- `card_foreground`
- `popover`
- `popover_foreground`
- `panel`
- `panel_foreground`
- `primary`
- `primary_foreground`
- `secondary`
- `secondary_foreground`
- `accent`
- `accent_foreground`
- `muted`
- `muted_foreground`
- `border`
- `input`
- `ring`
- `destructive`
- `warning`
- `success`
- `info`
- `glow`
- `glow_muted`

The imported theme baselines should come directly from `thegridcn-ui`'s current semantic theme definitions in `src/app/globals.css`, then be translated into a Rust-native theme registry.

## 5.4 Fonts

The first-pass `sidereal-ui` font stack should align to thegridcn:

- primary sans: `Rajdhani`
- display: `Orbitron`
- mono: `Geist Mono`

Practical notes:

- these fonts should be vendored/self-hosted in Sidereal's asset tree rather than fetched dynamically,
- the UI kit should expose typography roles instead of components selecting font files ad hoc,
- `display` usage should stay intentional so the UI remains readable and not over-stylized.

## 5.5 Layout and nesting

Basic layout composition does **not** need a new layout engine.

Bevy UI already gives us the required structural primitives:

- nested parent/child UI hierarchies,
- Flexbox layout,
- Grid layout,
- spacing/gap/alignment controls,
- absolute positioning where needed for overlays and decorations.

`sidereal-ui` should therefore provide ergonomic layout wrappers around Bevy-native primitives, for example:

- `UiColumn`
- `UiRow`
- `UiStack`
- `UiGrid`
- `UiContainer`
- `UiInset`
- `UiSpacer`

These should compile down to Bevy-native node/layout configuration and remain fully nestable.

## 5.6 Core modules

Suggested module layout:

- `theme.rs`
  - semantic theme definitions, theme registry, theme switching
  - imported thegridcn theme data translated into Rust/Bevy types
- `tokens.rs`
  - strongly typed variant/size/intent enums
- `typography.rs`
  - font roles, scale, text treatments
- `surface.rs`
  - panel/card/dialog/sheet shells
- `button.rs`
  - button variants, icon button, segmented button
- `field.rs`
  - label, input frame, helper/error text, focus/invalid state
- `progress.rs`
  - loading bars, segmented bars, status indicators
- `layout.rs`
  - stack/row/column/grid/container helpers backed by Bevy layout
- `motion.rs`
  - hover/focus/press transitions, opacity/scale pulses
- `materials.rs`
  - optional 2D/UI material treatments for glow, scanline, glass, or border accents
- `prelude.rs`
  - stable import surface for screen code

## 5.7 Authoring model

Prefer a composition model like this:

- screen systems decide *what* exists
- UI kit primitives decide *how* common surfaces/controls look and react

That means game screens still spawn UI from ECS systems, but instead of hardcoding colors and padding they call reusable builders/bundles/helpers such as:

- `UiPanel::dialog()`
- `UiButton::primary("Login")`
- `UiField::password("Password")`
- `UiSectionHeader::new("Owned Entities")`
- `UiGrid::columns(3)`
- `UiRow::between()`

These names are illustrative, not final API.

## 5.8 Styling model

Stop scattering raw `Color::srgb` and `Color::srgba` literals through screen code. Move styling into semantic tokens:

- `surface.background.panel`
- `surface.background.overlay`
- `border.default`
- `border.focus`
- `text.primary`
- `text.muted`
- `intent.info`
- `intent.warning`
- `intent.error`
- `accent.primary`

The existing values in `docs/ui_design_guide.md` should be reconciled with the imported thegridcn semantic themes. Where the current Sidereal guide conflicts with the adopted imported themes, we should update the design guide in implementation changes rather than silently drifting in code.

## 5.9 Component adoption target

The objective is to port as many of thegridcn's useful non-3D components and patterns as make sense for Sidereal.

High-priority candidates:

- buttons
- cards/panels
- dialogs/modals
- alerts/banners
- fields/labels/helper text
- tabs
- badges/chips/tags
- dropdown/select patterns
- progress bars / status bars / segmented indicators
- tables / data rows / metric rows
- terminal/diagnostic presentation motifs
- HUD frames and corner treatments where appropriate

These should be reimplemented as native Bevy components, not wrapped web assets.

## 5.10 Decorative "gridcn" look

If we want some of the more distinctive web-demo feel, do it selectively with Bevy-native effects:

- panel background gradients/glass treatment through UI material or textured assets
- subtle scanline/grid/noise overlays as optional child layers
- animated accent borders for focused/active states
- shader-backed hero panels only on auth/menu screens, not every HUD control

Do not make the gameplay HUD depend on heavy decorative materials by default.
Do not bring over thegridcn's 3D scene components.

## 5.11 Scriptability contract for `sidereal-ui`

`sidereal-ui` must be built as a script-consumable presentation layer, not only as a Rust-authored widget library.

Required direction:

- Lua-authored gameplay/content systems should be able to request UI presentation through validated UI events/actions.
- Core UI primitives should have stable schema-backed descriptors that can be spawned/configured from scripted content.
- Narrative/dialogue UI must be expressible from script data rather than requiring hardcoded Rust screen logic per quest.

Target script-driven presentation cases:

- NPC communication dialogs with portrait/avatar media
- progressive text reveal / crawl
- player input to fast-forward reveal to full text
- one-of-N branching response choices
- mission board/job board panels
- scripted alerts, banners, and transmission overlays
- faction/contact popups and quest progression moments

Recommended authority split:

- Lua on the authoritative host decides *what* narrative/UI event should be presented and with what data.
- Rust validates, persists/queues, and replicates the presentation payload as needed.
- The client-side `sidereal-ui` renderer decides *how* to present that payload visually.
- Player dialogue/choice input goes back to the authoritative host as validated intent, not local client truth.

Illustrative future payload shape:

- `dialog_id`
- `speaker_id` or `portrait_asset_id`
- `speaker_name`
- `body_text`
- `reveal_mode`
- `allow_skip_reveal`
- `choices = [{ id, label, hotkey }]`
- optional styling/theme overrides within an allowlisted schema

This should remain schema/data-driven so content authors can assemble dialogue and mission UI in Lua without expanding Rust UI code for every new quest.

## 6. Migration Strategy

Use an incremental migration, not a flag-day replacement.

### Phase 1: Tokens and primitives

Build:

- theme registry with imported thegridcn theme definitions
- typography roles and the adopted font stack
- layout helpers (`row`, `column`, `grid`, `container`)
- panel/card primitive
- button primitive
- progress bar primitive
- alert/dialog shell primitive

Convert first:

- `dialog_ui.rs`
- `asset_loading_ui.rs`
- `world_loading_ui.rs`
- `pause_menu.rs`

These screens are structurally simple and give fast payoff while proving the token/layout stack.

### Phase 2: Form controls

Build:

- labeled text field
- password field
- status/helper/error text
- focus ring/invalid state handling
- tabs/select/dropdown basics as needed by migrated flows

Convert:

- `auth_ui.rs`
- character selection screens

This is the hardest "standard control" slice because text entry and focus behavior need to stay correct.

### Phase 2.5: Script-driven narrative UI

Build:

- dialogue panel shell
- portrait/avatar slot support
- progressive text reveal controller
- skip-to-end interaction
- branching choice list with hotkey support
- replicated/script-authored UI payload schema and renderer bridge

Convert/add:

- first scripted communication/dialogue proof of concept
- one mission-board or quest-offer flow authored primarily in Lua data/script

This slice validates that `sidereal-ui` is usable by scripted content systems rather than only by Rust-authored screens.

### Phase 3: In-world screens

Build:

- HUD cards
- segmented controls
- compact stat rows
- list items / selectable rows
- sheet/panel shell for owned-entity and tactical panels
- optional HUD frame/corner-bracket treatments inspired by thegridcn

Convert:

- pause and owned-entity panels in `ui.rs`
- tactical map panels in `scene_world.rs`

### Phase 4: Advanced visuals

Add only after the primitive kit is stable:

- shader-backed glass/grid panels
- animated decorative borders
- optional motion presets for menu surfaces
- selected thegridcn-inspired ornamental treatments that do not compromise readability

This keeps the migration from turning into a style-effects rewrite before the basics are reusable.

## 7. Screens/Systems to Keep Game-Specific

Even after introducing a component library, these should stay custom/game-native:

- tactical map overlay and markers
- nameplates anchored to world entities
- debug overlay panels tied to runtime diagnostics
- screen overlay passes tied to overlay cameras/material passes

Those are not normal "app UI" controls. They should consume shared typography/surface tokens where useful, but not be forced into generic form components.

## 8. Risks and Non-Goals

### 8.1 Non-goals

- Do not run a hidden browser/webview inside the native client for normal in-game UI.
- Do not try to interpret Tailwind classes at runtime in Rust.
- Do not create a React-like virtual DOM inside Bevy.
- Do not break native/WASM shared runtime behavior to chase exact web-demo parity.
- Do not switch the entire in-game UI stack to `bevy_egui` by default as a styling shortcut.
- Do not import or recreate thegridcn's 3D components as part of this plan.

### 8.2 Main risks

- text input and focus management complexity
- over-abstracting too early and making simple UI harder to write
- performance regressions if shader-heavy materials are attached to dense HUD/nameplate trees
- mixing game-specific overlays into generic components until the abstraction becomes muddy

## 9. Testing and Validation

For each migration slice:

- unit tests for token resolution and variant/style mapping
- targeted client tests for interaction state systems where practical
- `cargo check --workspace`
- `cargo check -p sidereal-client --target wasm32-unknown-unknown --features bevy/webgpu`
- `cargo check -p sidereal-client --target x86_64-pc-windows-gnu`

If the UI kit becomes a new crate, add focused tests there instead of inlining everything into client runtime modules.

## 10. Recommended First Implementation Slice

The best first slice is:

1. create `crates/sidereal-ui`,
2. vendor the chosen font assets (`Rajdhani`, `Orbitron`, `Geist Mono`) into Sidereal,
3. import thegridcn theme definitions into a Rust OKLCH theme registry,
4. implement `Row`, `Column`, `Grid`, `Panel`, `Button`, `Dialog`, and `ProgressBar`,
5. migrate `dialog_ui.rs`, `asset_loading_ui.rs`, and `world_loading_ui.rs`.

That gives immediate reduction in duplicated style code and validates fonts, themes, and layout composition before touching text-input-heavy auth flows or the in-world HUD.

## 11. Bottom Line

The decision is settled:

- build a native `sidereal-ui` kit,
- use Bevy's existing hierarchy and flex/grid layout support,
- adopt thegridcn's font stack and imported semantic themes,
- keep theme data canonical in OKLCH,
- port the useful non-3D component patterns as faithfully as practical,
- leave React, DOM, Tailwind runtime behavior, and 3D showcase pieces out of the game client.
