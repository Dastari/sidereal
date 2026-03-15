# Bevy Planet Rendering Recommendations

_A practical rendering and generation plan for a top-down space ARPG using Bevy + Lightyear + Avian2D_

## Summary

For this project, I recommend:

- Keep the **gameplay world 2D**
- Add a **small isolated 3D planet rendering path** for large visible planets
- Use **seed-driven precomputed planet textures/maps**
- Use **real 3D sphere meshes** for close-up planets that take up 40–60% of the screen
- Use **2D impostors or sprites** for distant/small planets
- Avoid using a **fully procedural fragment shader** as the main runtime planet solution

This gives the best mix of:

- visual quality
- scalability
- style control
- easy sun-direction lighting
- simple rotation
- predictable performance

---

## Why I recommend a small dedicated 3D planet path

Your planets are not tiny decorative dots.

They are:

- large on screen
- mostly stationary
- rotating only
- lit by a known sun direction
- usually only 1–2 visible at a time
- effectively **hero background objects**

That makes them a strong fit for **real sphere meshes** rather than a 2D shader trying to simulate a giant sphere on a quad.

### Why this is better than the current procedural 2D shader

Your current shader is doing too much live work:

- layered noise / FBM
- biome-ish color synthesis
- clouds
- atmosphere
- rings
- height re-sampling for perturbed normals
- per-pixel procedural reconstruction every frame

That is appropriate for a shader experiment or very rare hero object, but not ideal as the main production pipeline.

Problems with the current direction:

1. **Expensive per-pixel work**
2. **Hard to art-direct**
3. **Can still look soft / low-resolution**
4. **Gets more fragile as planet screen size increases**
5. **Rotation and lighting are more complicated than they need to be**

### Why 3D helps here

A real sphere gives you:

- correct normals for lighting
- easy rotation
- stable day/night terminator
- easier cloud shell layering
- easier ring orientation
- fewer giant-billboard artifacts

Since only a few planets are visible at once, the cost is very manageable.

---

## Recommended architecture

Use a **tiered planet system**.

### Tier A — Galaxy/System data

Store planet data as compact deterministic parameters:

- `planet_seed`
- `planet_class`
- `radius`
- `palette_id`
- `surface_style_id`
- `atmosphere_type`
- `cloud_coverage`
- `cloud_speed`
- `rotation_speed`
- `ring_params`
- `sun_response_params`
- `biome_params`
- `height_params`

This is the true gameplay/network representation.

### Tier B — Generated/baked assets

Generate or cache these per planet seed:

- `albedo_map`
- `height_map`
- `normal_map`
- `cloud_map`
- `night_map` (optional)
- `roughness/spec mask` (optional)
- `biome mask` (optional)

These should be generated:

- offline
- at content-build time
- or on-demand and cached to disk

Do **not** regenerate them every frame in the render shader.

### Tier C — Runtime render LOD

Use different render paths depending on importance:

#### 1. Tiny / distant planets

Use:

- icon
- simple sprite
- low-cost billboard

#### 2. Mid-size visible planets

Use:

- 2D impostor shader
- baked albedo + normal + clouds
- fake sphere shading

#### 3. Large close-up planets

Use:

- real 3D sphere
- custom stylized material
- optional cloud shell
- atmosphere shell or rim shader
- optional ring mesh

This is the tier I recommend as your primary visible in-game planet presentation.

---

## The main recommendation in one sentence

**Keep the game 2D, but render large visible planets as seeded, baked, stylized 3D spheres.**

---

## Recommended close-up planet rendering model

Each close-up planet should be composed from a few simple parts.

### 1. Surface sphere

Inputs:

- albedo texture
- normal texture
- optional height texture
- sun direction
- stylized light curve parameters
- palette grading values

Responsibilities:

- base surface look
- day/night split
- stylized terminator
- optional ice/lava/desert/emissive features

### 2. Cloud sphere

A second slightly larger sphere.

Inputs:

- cloud texture
- alpha
- cloud rotation speed
- cloud lighting strength

Responsibilities:

- cloud motion independent from surface
- visible large-scale weather bands
- soft lighting on cloud tops

### 3. Atmosphere shell or atmosphere rim shader

A third shell or a rim function in the fragment shader.

Responsibilities:

- colored atmospheric edge
- thicker glow on sunlit side
- optional night-side rim bleed
- stylistic planetary silhouette boost

### 4. Ring mesh (optional)

Separate mesh:

- flat ring
- own texture/mask
- own orientation
- can be partially shadowed or faded behind planet

---

## Lighting recommendation

Do **not** use full realistic world lighting unless you specifically want it.

Instead, use a **custom stylized directional light model** per planet.

### Suggested lighting model

For each fragment:

- compute surface normal `N`
- normalize sun direction `L`
- compute `ndotl = max(dot(N, L), 0.0)`

Then remap that into a stylized curve.

Examples:

- smoothstep terminator
- widened penumbra
- artist-controlled night tint
- boosted rim on lit edge
- optional cloud highlight curve

### Why this is better

You do not need:

- dynamic shadows from gameplay entities
- full PBR correctness
- expensive multi-light scene logic

You only need:

- stable “sun is coming from this direction” shading

That is ideal for a custom planet material.

---

## Generation pipeline recommendation

The rendering should consume **baked maps**, not live procedural FBM.

## Suggested pipeline

### Step 1 — Seed to parameter set

Start from a deterministic seed and derive:

- temperature bias
- moisture bias
- elevation roughness
- ocean level
- biome weights
- mountain sharpness
- polar cap size
- cloud density
- atmosphere hue
- ring chance
- volcanic activity
- crater density
- gas giant banding parameters

### Step 2 — Generate base scalar fields

Generate equirectangular or cubemap-space fields:

- continental elevation
- mountain noise
- detail noise
- temperature map
- moisture/humidity map
- crater mask
- volcanic mask
- cloud coverage map

### Step 3 — Resolve biome map

Use combined fields:

- height
- temperature
- moisture
- latitude
- volcanic/crater modifiers

Then classify into biomes like:

- ocean
- coast
- desert
- grassland
- forest
- jungle
- tundra
- ice
- mountain
- lava
- ashlands
- crystal/exotic

### Step 4 — Bake runtime textures

Export:

- albedo
- normal
- height
- cloud alpha
- night lights/emissive
- masks for stylization if needed

### Step 5 — Cache

Save them in:

- asset cache
- generated texture folder
- content-addressed seed cache

That way planets remain deterministic but cheap at runtime.

---

## Resolution recommendations

For close-up visible planets:

- albedo: `1024x512` minimum
- normal: `1024x512`
- clouds: `1024x512`
- hero planets: `2048x1024`

For smaller planets:

- `512x256` may be enough

If the planet fills half the screen, do not expect a tiny procedural disc shader to look crisp. Use a proper baked map.

---

## Texture projection recommendation

For simplicity, use:

- **equirectangular map** for baked textures
- sample via sphere UVs in the 3D material

Alternative:

- cubemap workflow

But for your use case, equirectangular is probably simpler and perfectly acceptable.

### Why equirectangular is enough

You are not building a planet-walking simulator.
You are rendering large scenic planets from a controlled camera distance.

This means:

- easier generation tooling
- easier debugging
- easy longitude scrolling / rotation
- easier export/import workflow

---

## Biome generation recommendation

Use biomes as a **classification layer**, not as separate handwritten textures.

### Good biome inputs

- normalized elevation
- latitude
- temperature
- moisture
- rain shadow / erosion proxy
- volcanic activity
- ocean proximity

### Good biome outputs

- biome ID
- biome color palette
- roughness/spec values
- cloud tendency
- emissive tendency
- local normal detail scale

This gives you a strong style system:

- realistic-ish planets
- stylized planets
- alien planets
- faction-themed planets
- corrupted planets
- biome blends unique to your world

---

## Planet classes to support

At minimum, I would support these classes:

### Rocky terrestrial

- height-driven continents
- mountain ranges
- deserts / forests / tundra
- normal map matters

### Ice world

- flatter palette
- glacial streaks
- strong polar response
- softer cloud look

### Lava / volcanic

- emissive fissures
- dark crust
- low cloud or ash cloud layer

### Desert world

- strong dune colors
- low moisture
- large plateau features

### Ocean world

- high water coverage
- cloud-heavy
- softer land contrast

### Gas giant

- no true height map needed
- layered band textures
- storm masks
- fast cloud/band motion

### Barren moon / asteroid moon

- crater-heavy
- minimal clouds
- very simple lighting

Each class can share the same rendering framework but use different generation rules.

---

## Best option for height maps

For your use case, the height map should mainly serve:

- normal generation
- biome classification
- silhouette-independent detail
- optional parallax-ish enhancement
- crater/mountain placement

It does **not** need to drive a heavily displaced render mesh unless you later want very dramatic close-up orbit views.

### Recommendation

Use the height map to derive:

- normal map
- biome thresholds
- optional roughness/spec changes

Avoid true heavy displacement at this stage.

Reason:

- your current need is scenic planet rendering, not terrain traversal
- normal mapping gives most of the value for much less complexity

---

## 2D vs 3D conclusion

## Use 3D when:

- a planet occupies 20%+ of screen area
- lighting direction matters
- cloud shell rotation matters
- atmosphere/rim needs to look correct
- you want high visual credibility

## Use 2D when:

- the planet is small or distant
- it is only decorative
- it is in a system map / tactical view
- you need many low-importance bodies visible

### Final decision for this game

For your described close-up parallax planets:
**use 3D spheres for visible hero/background planets**

---

## Bevy integration strategy

Do **not** convert the whole game into a 3D gameplay architecture.

Instead:

### Option A — Separate 3D render layer in same app

Have:

- main 2D gameplay camera
- secondary 3D planet camera
- render planets on specific render layers
- compose them visually as a background/scenic pass

This is probably the most practical solution.

### Option B — Two-pass rendering

Render:

1. planet pass
2. main 2D gameplay pass

Useful if you want tighter control over ordering and post-processing.

### Option C — Planet-only offscreen texture

Render 3D planets to an offscreen texture and composite into the main 2D scene.

Good when:

- you want extra post FX
- you want strong decoupling
- you want easy parallax control

This is more advanced, but clean.

---

## My preferred Bevy integration choice

I would start with:

- one 2D gameplay world
- one 3D planet camera
- render layers to isolate the planets
- custom material on sphere meshes
- no Bevy PBR dependency beyond what is strictly useful

That keeps the feature contained.

---

## Suggested ECS layout

### Components

- `PlanetSeed`
- `PlanetVisualClass`
- `PlanetRotationSpeed`
- `PlanetSunLightParams`
- `PlanetAtmosphereParams`
- `PlanetCloudParams`
- `PlanetRingParams`
- `PlanetLodState`
- `PlanetGeneratedAssetsHandle`

### Systems

- `spawn_visible_planets`
- `despawn_hidden_planets`
- `request_planet_asset_generation`
- `poll_generated_planet_assets`
- `update_planet_rotation`
- `update_planet_sun_direction`
- `update_planet_lod`
- `sync_planet_render_layers`

### Asset types

- `PlanetSurfaceMaps`
- `PlanetCloudMaps`
- `PlanetMaterialConfig`
- `PlanetGenerationRecipe`

---

## Networking / Lightyear recommendation

Do not network planet textures directly.

Network only:

- seed
- class
- generation parameters
- style parameters

Each client can:

- generate
- load from cache
- or download from prebuilt assets

For deterministic consistency:

- keep the generation algorithm versioned
- include a generation version hash in planet metadata

Example:

- `planet_seed`
- `planet_gen_version`
- `planet_style_version`

This avoids desync caused by content pipeline drift.

---

## Asset generation recommendation

You have three viable paths:

### Path 1 — Offline pre-bake

Best for shipping quality and style control.

Pros:

- predictable quality
- easiest to debug
- best runtime performance

Cons:

- larger content storage
- less “infinite” spontaneity

### Path 2 — On-demand generation + disk cache

Great middle ground.

Pros:

- still seed-driven
- scalable
- no need to pre-bake entire galaxy

Cons:

- more engineering work
- first-load generation stalls must be handled cleanly

### Path 3 — Runtime GPU generation every frame

Not recommended as your main approach.

Pros:

- flashy
- compact source data

Cons:

- unstable cost
- harder style control
- harder debugging
- worst fit for giant close-up planets

### My recommendation

Use **Path 2**:

- generate maps from seed when first needed
- cache them
- reuse them forever

---

## Style-control recommendation

To keep planets visually cohesive, define a **style bible**.

Each planet should not be “random noise with arbitrary colors.”
Instead, define:

- palette families
- cloud softness families
- atmosphere families
- terrain sharpness presets
- biome thresholds by world class
- ring palette sets
- faction/culture ownership overlays if relevant

This will make the galaxy feel authored rather than noise-generated.

---

## Performance recommendation

The expensive thing is not “3D exists.”
The expensive thing is:

- too many transparent shells
- very high-poly spheres
- too many large overdraw layers
- too many hero planets at once
- regenerating textures repeatedly

### Recommended budget approach

- only 1–2 hero planets active at a time
- medium sphere tessellation
- 1 cloud shell max
- 1 atmosphere shell or shader rim
- ring mesh only when needed
- precomputed textures
- strong LOD cutoffs

---

## Concrete implementation roadmap

## Phase 1 — Fastest route to success

1. Add a 3D sphere in a separate layer/pass
2. Feed it a static test planet texture
3. Add directional stylized lighting in shader
4. Add surface rotation
5. Add cloud shell
6. Add atmosphere rim

Goal:
prove the visual approach quickly

## Phase 2 — Seeded generation

1. Build deterministic seed-to-parameter generator
2. Generate albedo / height / normal / cloud maps
3. Cache to disk
4. Hook those assets into the runtime sphere material

Goal:
replace handmade test texture with your actual content pipeline

## Phase 3 — Planet classes

1. rocky
2. ice
3. lava
4. desert
5. gas giant
6. moon

Goal:
lock down the visual language of your universe

## Phase 4 — LOD system

1. small planets = impostor/sprite
2. large planets = 3D
3. map view = icon/simple sprite

Goal:
production scalability

---

## What I would avoid

Avoid these as primary solutions:

### 1. Fully procedural fragment shader as final runtime planet pipeline

Too hard to scale and art direct.

### 2. Making the whole game 3D just because planets are 3D

Unnecessary complexity.

### 3. Heavy geometry displacement for surface relief

Probably not worth it unless you later add very close orbital views.

### 4. Rebuilding generated textures too often

Generate once, cache, reuse.

---

## Final recommendation

For this specific game:

**Use seeded precomputed planet textures mapped onto real 3D spheres for large visible planets, while keeping the gameplay world 2D and using cheaper 2D representations for distant planets.**

That is the strongest balance of:

- quality
- performance
- maintainability
- style control
- simple sun-direction lighting
- easy rotation

---

# Reference links

## Official Bevy references

### Shader/material examples

- https://bevyengine.org/examples/shaders/shader-material-2d/
- https://bevyengine.org/examples/shaders/shader-material-glsl/
- https://bevyengine.org/examples/shaders/shader-material-wesl/

### 3D mesh / shapes / scene references

- https://bevyengine.org/examples/3d-rendering/3d-shapes/
- https://bevyengine.org/examples/3d-rendering/3d-scene/
- https://bevyengine.org/examples-webgpu/3d-rendering/texture/

### Camera / render layering / pass structure

- https://bevyengine.org/examples/camera/first-person-view-model/
- https://bevyengine.org/examples/3d-rendering/two-passes/
- https://bevyengine.org/examples/camera/2d-top-down-camera/

### Transparency / shells / atmosphere-adjacent concerns

- https://bevyengine.org/examples/3d-rendering/transparency-3d/

### Extra rendering references

- https://bevyengine.org/examples/3D%20Rendering/parallax-mapping/
- https://bevyengine.org/examples/math/custom-primitives/

---

## Procedural planet / height map / biome references

These are mainly useful for **algorithm ideas** and generation-pipeline structure.

### Planet-scale maps / spherical world generation

- https://github.com/MightyBOBcnc/nixis

### Procedural planet generator example

- https://github.com/Hoimar/Planet-Generator

### Heightmap + moisture + biome classification example

- https://github.com/strwdr/Procedural-Maps

### Heightmap + moisture + multiple biome bands

- https://github.com/dgattey/genera

### Biome-oriented PCG reference

- https://github.com/GrandPiaf/Biome-and-Vegetation-PCG

### Heightmap import / terrain generation workflow reference

- https://github.com/IceCreamYou/THREE.Terrain

---

# Suggested next document

If needed, the next document should be:

## `bevy_planet_implementation_plan.md`

Containing:

- ECS structure
- asset types
- Bevy plugins/resources
- render-layer setup
- camera setup
- material bind group design
- WGSL material inputs
- seed-to-texture generation pipeline
- LOD state machine
- debug tooling checklist

---

# Short version

If I were building this today, I would do this:

- Keep game logic in 2D
- Render large planets as 3D spheres
- Use precomputed seeded textures
- Add cloud shell + atmosphere rim
- Use fake directional sunlight in custom shader
- Reserve 2D impostors for distant bodies
- Never rely on a giant live procedural shader as the main shipping planet renderer
