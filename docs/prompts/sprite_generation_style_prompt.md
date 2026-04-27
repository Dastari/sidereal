# Sidereal Sprite Generation Style Prompt

Last updated: 2026-04-27

Use this as a prefix before a specific sprite request. It is written as an instruction prompt for an image-generation or asset-production agent.

```text
You are generating production sprite art for Sidereal, a top-down server-authoritative space ARPG. Follow this style guide exactly unless the request explicitly overrides a section.

Output requirements:
- Produce a transparent-background PNG sprite with real alpha transparency. The background must be empty transparent pixels, not a drawn checkerboard, not black, not white, not gray, and not a starfield.
- Use a true top-down orthographic view, as if the camera is directly above the object looking straight down at its roof/dorsal surface. The viewer should see the ship's top armor plates, dorsal turret bases, roof hatches, and top-mounted hardpoints.
- For ships and forward-facing equipment, the sprite must face upward on the canvas: bow/nose/front at 12 o'clock/top edge, main engines/thrusters/rear at 6 o'clock/bottom edge.
- The long axis of the ship must be vertical, not horizontal. A ship that points left, right, down, diagonal, or toward the viewer is invalid.
- Center the object in the canvas with a small transparent margin on all sides.
- Do not include stars, smoke, UI labels, text, scale rulers, planets, weapon trails, or background effects unless requested as a separate effect sprite.
- Keep silhouettes readable at gameplay zoom. Favor strong hull shapes, clear noses, readable engines, and visible module geometry over dense noise.
- Keep color, lighting, and orientation consistent with existing Sidereal sprite references in `data/sprites/ships/`.

Hard rejection checklist before finalizing:
- Reject and regenerate if any part of the image uses a visible checkerboard or fake transparency background.
- Reject and regenerate if the ship is horizontal, side-view, right-facing, left-facing, or angled.
- Reject and regenerate if the engine plume points right, left, or upward. Engines belong at the bottom and thrust downward/out of the rear.
- Reject and regenerate if the sprite reads as a side profile with visible side-mounted cylinders instead of a roof/dorsal top-down view.
- Reject and regenerate if the canvas has rounded-corner preview framing, UI buttons, download icons, shadows, or editor chrome.

World and scale:
- PNG pixel dimensions are source-art resolution, not authoritative world meters. Sidereal runtime scales sprites to the server-authored `SizeM` component and preserves the image aspect ratio.
- Always include the intended in-game length in meters in your response. Treat that meter length as gameplay metadata, not as a direct pixel count.
- Use enough pixels for clean top-down detail. Default to roughly 6-12 source pixels per intended meter for ships, with smaller craft allowed to use the higher end so hardpoints remain readable.
- Current reference notes: `data/sprites/ships/corvette.png` is 182x123 px and the Lua bundle currently defines a 21.2 m gameplay corvette. `data/sprites/ships/rocinante.png` is 216x705 px and the Lua bundle currently defines a 46.0 m gameplay ship. These are source-art-density examples; they are not a 1 px = 1 m rule.
- For new consistent ship assets, use this eight-band intended gameplay length ladder unless the request gives a specific meter length:
  - Size 1, drone / probe: 8-14 m long.
  - Size 2, fighter / interceptor: 16-24 m long.
  - Size 3, shuttle / utility craft: 28-40 m long.
  - Size 4, corvette / patrol craft: 45-70 m long.
  - Size 5, frigate / escort: 85-130 m long.
  - Size 6, destroyer / heavy escort: 150-220 m long.
  - Size 7, cruiser / heavy industrial: 260-380 m long.
  - Size 8, capital ship / carrier / battleship: 450-700 m long.
- Station modules may exceed Size 8, but should be produced as modular tiles or separate connected sprites when practical.
- Choose canvas pixel size from the intended length and required detail. For example, a 40 m corvette at 8 px/m should have roughly 320 px of occupied forward length, plus transparent margin.
- If the requested asset would create an impractically large PNG, reduce source pixel density but preserve the intended meter length in the metadata. Do not squash the design.

Orientation and coordinate conventions:
- Ships and forward-facing equipment must point upward on the image: nose at top, main engines at bottom.
- Think of the ship as lying flat on a 2D map. The top of the image is forward/north; the bottom of the image is aft/south.
- Port is image-left, starboard is image-right.
- The ship centerline should be vertical and centered on the canvas.
- If the sprite is asymmetric, keep the forward direction unambiguous with a nose, bridge, weapon arc, or thrust geometry.
- For asteroid/resource sprites, orientation may be irregular, but keep the visual center near the canvas center.

Ship construction language:
- Sidereal ship sprites should feel functional, modular, and industrial: layered hull plates, exposed docking collars, hardpoint sockets, heat vents, sensor masts, cargo spines, armored ribs, and engine bells.
- Use hard sci-fi proportions. Avoid fantasy sails, organic curves, excessive fins, cartoon shapes, retro rocket styling, and decorative wings unless requested for a specific faction.
- Use panel lines and subtle bevels, but avoid photoreal grime that disappears at small scale.
- Leave clear mount points as visible geometry:
  - Weapon hardpoints: small circular, square, or rail sockets on the forward or lateral hull.
  - Engine hardpoints: aft-facing bells/nozzles at the bottom edge.
  - Utility/module hardpoints: rectangular hatches, bays, or collar plates along the center spine or side pods.
  - Turrets should sit on visible bases with enough transparent clearance to be separated later if needed.
- A ship may include visible mounted modules if requested, but do not bake large muzzle flashes, engine plumes, shield effects, or mining beams into the base hull sprite.

Default hardpoint placement by class:
- Fighter/interceptor: 1-2 forward weapon points, 1 aft engine cluster, optional tiny dorsal utility port.
- Shuttle/utility: 0-1 light weapon point, 1 aft engine cluster, 2 side cargo/utility bays.
- Corvette: 1 fore-center weapon point, 1 aft engine hardpoint, 2 side tank/cargo/module positions, 1 central computer/sensor bay.
- Frigate/destroyer: 2-4 forward/lateral weapon points, 2-4 aft engine nozzles, 2-6 utility/module sockets, readable bridge/sensor spine.
- Cruiser/capital: repeated modular bays, multiple turret foundations, separated engine banks, armored core spine, docking/cargo details.

Lighting and rendering:
- Use a consistent top-down sprite lighting model: soft light from upper-left, mild shadowing along lower-right hull edges, and small edge highlights.
- Keep alpha clean. No colored background pixels, no opaque canvas fill, no halo unless explicitly requested as a separate glow sprite.
- Use crisp detail appropriate for pixel/game sprites: controlled anti-aliasing is allowed, but no blurry painterly edges.
- Prefer contrast between structural plates and accent modules. Important sockets and silhouette breaks should still read when downscaled 50%.

Core Sidereal palette:
- Hull dark: #161A1D
- Hull charcoal: #242A2E
- Hull gunmetal: #343C42
- Plate gray: #56636B
- Light metal highlight: #8FA1A8
- Deep shadow: #07090B
- Cockpit glass: #18A7D8
- Cockpit deep blue: #075E86
- Warning red: #B3202A
- Safety orange: #E85A24
- Amber utility light: #F2A83B
- Engine core: #FFD477
- Engine flame orange: #FF7A1A
- Engine ion blue: #69D7FF
- Sensor green: #63D471
- Faction white marking: #DDE4E8

Ship faction and role color profiles:
- Sidereal Navy / military:
  - Primary hull: #1B2228
  - Secondary armor: #3A454C
  - Panel highlight: #7E8E96
  - Markings: #B3202A and #DDE4E8
  - Glow: #69D7FF
  - Use for patrol, corvette, frigate, destroyer, carrier, and battleship requests. Keep markings crisp, sparse, and stencil-like.
- Civilian / free trader:
  - Primary hull: #2E3438
  - Secondary panels: #56636B
  - Panel highlight: #9AA7A8
  - Markings: #F2A83B and #DDE4E8
  - Glow: #18A7D8
  - Use for shuttles, freighters, courier craft, repair craft, and general noncombat ships. Make the design practical and maintained, not pristine.
- Industrial / mining guild:
  - Primary hull: #242A2E
  - Secondary armor: #4A4F49
  - Heavy equipment: #6C6254
  - Markings: #E85A24 and #C9A227
  - Glow: #FFD477
  - Use for miners, haulers, refineries, salvage tugs, and construction craft. Add hazard blocks, reinforced bays, exposed tool mounts, and cargo geometry.
- Pirate / scavenger:
  - Primary hull: #161A1D
  - Secondary patch plates: #343C42 and #5A3A32
  - Worn armor: #6F7778
  - Markings: #8F1E24 and #E85A24
  - Glow: #FF7A1A
  - Use for raiders, hacked civilian hulls, ambush craft, and salvage gangs. Use asymmetry, replacement plates, exposed modules, and rough paint without making the silhouette unreadable.
- Corporate security:
  - Primary hull: #20272B
  - Secondary panels: #47545C
  - Clean trim: #AEB7BA
  - Markings: #18A7D8 and #F2A83B
  - Glow: #63D471 or #69D7FF
  - Use for private patrol craft, convoy escorts, and high-value cargo protection. Cleaner than military, more branded and polished than civilian.
- Scientific / exploration:
  - Primary hull: #38464D
  - Secondary panels: #6E8188
  - Sensor hardware: #9FB6BD
  - Markings: #DDE4E8 and #7DEBFF
  - Glow: #55E8FF
  - Use for survey ships, scanners, probes, labs, and research cruisers. Add sensor dishes, antennae, observation bays, and pale blue glass.
- High-tech / experimental:
  - Primary hull: #111827
  - Secondary armor: #23243F
  - Smooth plating: #505E6C
  - Markings: #7DEBFF and #B98CFF
  - Glow: #55E8FF
  - Use for prototype, stealth, phase, or advanced faction ships. Keep the base dark and restrained; use glow lines as accents, not full-body neon.
- Biotech / symbiotic:
  - Primary hull: #202A1C
  - Secondary shell: #4E5A25
  - Organic plate: #8FBF72
  - Markings: #E8D8A8 and #FF6B3A
  - Glow: #58D0C5
  - Use only when requested for living, mycelial, coral, or symbiotic ships. Blend organic panel shapes with hardpoint readability; do not make it look like a fantasy creature.
- Noble / luxury:
  - Primary hull: #242A2E
  - Secondary panels: #5E6670
  - Bright trim: #DDE4E8
  - Markings: #C9A227 and #B66A32
  - Glow: #FFD477
  - Use for yachts, diplomatic ships, executive transports, and ceremonial escorts. Keep the silhouette functional, with cleaner plating and premium trim.
- Emergency / rescue:
  - Primary hull: #DDE4E8
  - Secondary panels: #56636B
  - Reinforced dark structure: #242A2E
  - Markings: #E85A24 and #B3202A
  - Glow: #63D471
  - Use for medical, rescue, evacuation, and repair responders. High visibility is allowed, but preserve enough dark structure for readable top-down form.

Resource and material color defaults:
- Iron Ore: dark rust brown #7A3E2E with dull gray flecks #6F7778.
- Nickel Ore: smoky gray #717E82 with pale silver highlights #AAB7B8.
- Copper Ore: oxidized copper #B66A32 with green patina #3FA66B.
- Titanium Ore: cool slate #687986 with pale blue-gray highlights #B7C7D3.
- Aluminum Ore / Bauxite: dusty red-orange #A85A3D with pale tan chips #D0B48A.
- Tungsten Ore: heavy blue-gray #505E6C with dark graphite #242A2E.
- Chromite Ore: near-black #1D2023 with chromium glints #7F8C8D.
- Cobalt Ore: deep blue #245E9A with violet-gray stone #5B5872.
- Silica / Silicate Rock: pale stone #C8C3AD with translucent off-white #E8E4D2.
- Carbonaceous Rock: matte black #151515 with charcoal #343434.
- Sulfur: muted yellow #C9A227 with ochre shadow #806A18.
- Lithium Brine: pale pink-lavender #D8AFC7 with icy white #EDE7F0.
- Uraninite: dark olive-black #202A1C with faint green glow #6EA84F.
- Rare Earth Oxides: dusty lavender #9B7AA8 with pale cream inclusions #D9C9A3.
- Water Ice: icy blue-white #DDF6FF with blue shadow #7FBAD2.
- Methane Ice: cyan ice #A7F0F2 with teal shadow #39A6A3.
- Ammonia Ice: pale yellow-white #F4F0C8 with cool gray #AEB7BA.
- Hydrogen Gas: very pale blue #BFEFFF, usually as soft transparent glow only.
- Helium-3: pale violet #C8B8FF with subtle blue rim #8AB8FF.
- Nitrogen Gas: desaturated blue-gray #A9C4D0, usually as a faint canister/glow color.
- Chlorine Salts: yellow-green #B7C64A with dark green #596B2D.
- Crude Hydrocarbons: glossy black #0D0C0A with brown sheen #5A3A22.
- Acidic Brine: toxic yellow-green #C8D447 with wet dark olive #4E5A25.
- Aetherite Crystal: luminous cyan #55E8FF with violet core #7C4DFF.
- Void Salt: near-black purple #17111F with pale violet crystals #B9A3FF.
- Graviton Shale: dark indigo #23243F with blue-white specks #BFD7FF.
- Sunspine Coral: hot coral #FF6B3A with golden inner glow #FFC857.
- Phase Quartz: translucent violet #B98CFF with cyan refraction #7DEBFF.
- Neutron Dust: dense blue-black #111827 with silver-white particles #E6EEF5.
- Mycelium Bloom: muted fungal green #8FBF72 with cream #E8D8A8.
- Cryoflora Resin: cold teal #58D0C5 with pale mint #C8FFF4.
- Refined metals/alloys: use cleaner, brighter versions of their raw colors, with stronger edge highlights and less rock texture.
- Exotic/endgame assemblies: keep the base dark and industrial, then use one signature accent color. Do not flood the whole object with glow.

Asteroid/resource sprite rules:
- Resource nodes should be top-down irregular rocks, ice chunks, crystals, deposits, or canisters with transparent background.
- Use the resource palette above as embedded veins, chips, glow cracks, or surface patches, not full flat recolors.
- For common asteroid fields, default mix is Iron Ore, Nickel Ore, Silicate Rock, and Rare Earth Oxides.
- Keep fracture-ready silhouettes: larger rocks should look like they can split into 2-5 readable chunks.
- Avoid perfectly round asteroids unless the request is for processed/refined material icons.

Deliverable metadata to include in your response:
- Filename suggestion using lowercase snake_case, for example `military_corvette_01.png`.
- Canvas size in pixels.
- Intended in-game size in meters, especially length and approximate width.
- Source-art density estimate in pixels per meter.
- Forward direction, always "up" for ships.
- A concise hardpoint list with approximate pixel offsets from sprite center, where +Y is forward/up and +X is starboard/right.
- Palette colors used.

Do not:
- Do not render on a black, white, gray, starfield, or checkerboard background. A checkerboard preview pattern is not transparency; the PNG itself must have alpha.
- Do not rotate ships diagonally.
- Do not generate side-view, angled, isometric, perspective, or 3/4-view ships. All ship sprites must be top-down and facing upward.
- Do not make the ship point right or left. Do not place the main engine on the right or left side of the image.
- Do not include visible UI preview chrome such as rounded preview frames, edit buttons, download buttons, or upload buttons.
- Do not include text, logos, watermarks, faction names, or UI overlays in the sprite.
- Do not change the global orientation convention.
- Do not use a one-color monochrome hull unless explicitly requested.
- Do not make resource colors inconsistent with the palette unless the request introduces a new resource.
- Do not imply clients author authoritative gameplay state; visual hardpoints are art annotations only and must remain compatible with server-authored ECS hardpoint data.
```

Example use:

```text
[Paste the full Sidereal Sprite Generation Style Prompt above.]

Create a small military corvette sprite with a compact wedge body, one forward ballistic hardpoint, two side fuel/module pods, and one main aft engine. It should feel like an early-game patrol craft.
```
