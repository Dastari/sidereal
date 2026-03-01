# Sidereal Planet Shader System

Complete procedural celestial body generation system for Bevy game engine.

## Quick Start

1. **Basic Rocky Planet**
   - Use: `planet_core.wgsl` with `planet_type = 0`
   - Seed: Any u32 for unique generation
   - See: `README_PLANETS.md` for full parameter list

2. **Gas Giant with Rings**
   - Planet: `planet_core.wgsl` with `planet_type = 4`
   - Rings: `planetary_rings.wgsl` on disk mesh
   - Atmosphere: Optional `planet_atmosphere.wgsl` layer

3. **Star with Corona**
   - Core: `planet_core.wgsl` with `planet_type = 6`
   - Corona: `stellar_corona.wgsl` on larger sphere
   - Blend mode: Additive for corona

## File Overview

### Shader Files
| File | Purpose | Complexity |
|------|---------|------------|
| `planet_core.wgsl` | Main surface generation (all types) | Medium |
| `planet_normal.wgsl` | Normal/bump map generation | Low |
| `planet_atmosphere.wgsl` | Atmospheric scattering & effects | High |
| `stellar_corona.wgsl` | Star corona, flares, prominences | Medium |
| `planetary_rings.wgsl` | Ring systems (Saturn-style) | Medium |
| `space_background.wgsl` | Nebulas, stars, galaxies, lightning | High |
| `asteroid.wgsl` | Mineral-rich asteroids with fracture support | Medium-High |
| `starfield.wgsl` | Simple background star field (legacy) | Low |

**Streamed shaders (starfield, space_background):** Edit only the files in `data/shaders/`. The client loads from `data/cache_stream/shaders/`, which is overwritten/copied from source; do not edit the cache files.

### Documentation Files
| File | Contains |
|------|----------|
| `README_PLANETS.md` | Complete parameter reference & examples |
| `README_SPACE_BACKGROUND.md` | Space environment system documentation |
| `README_ASTEROID.md` | Asteroid shader & mesh generation guide |
| `QUICK_REFERENCE.md` | Preset recipes & performance tips |
| `PARAMETER_CHEATSHEET.md` | Visual parameter ranges |
| `INDEX.md` | This file - overview & getting started |

## Supported Celestial Bodies

### Planets
- **Rocky** (Earth-like): Continents, oceans, mountains, craters
- **Desert** (Mars-like): Dunes, minimal water, dusty atmosphere
- **Lava** (Volcanic): Active volcanism, glowing lava flows
- **Ice** (Frozen): Ice formations, polar caps, reflective
- **Gas Giant** (Jupiter-like): Atmospheric bands, storms
- **Moon** (Barren): Heavily cratered, no atmosphere

### Stars
- **Main Sequence**: Yellow/white stars with active corona
- **Red Dwarf**: Cool, frequent flares
- **Blue Giant**: Hot, massive corona

### Features
- **Rings**: Multi-band systems with gaps and density waves
- **Atmospheres**: Rayleigh/Mie scattering, clouds, aurora
- **Special Effects**: City lights, volcanic glow, magnetic fields

### Space Environments
- **Nebulas**: Multi-color gas clouds with domain warping
- **Star Fields**: Multiple layers (distant, medium, bright)
- **Galaxies**: Distant spiral structures
- **Lightning**: Electrical discharges through nebulas
- **Dust & Wisps**: Subtle environmental detail
- **Emission Lighting**: Background affects scene lighting

## Architecture

### Rendering Layers
```
┌─────────────────────────────────┐
│   Optional: Corona Sphere       │  ← stellar_corona.wgsl (additive)
│   (1.2-2.0x radius, stars only) │
├─────────────────────────────────┤
│   Optional: Atmosphere Sphere   │  ← planet_atmosphere.wgsl (transparent)
│   (1.05-1.1x radius)            │
├─────────────────────────────────┤
│   Core Planet Sphere            │  ← planet_core.wgsl (opaque, PBR)
│   (base radius)                 │     + optional planet_normal.wgsl
└─────────────────────────────────┘
        Ring Disk (if applicable)    ← planetary_rings.wgsl (transparent)
```

### Deterministic Generation
All shaders use the `seed` parameter to generate consistent results:
- Same seed = identical planet every time
- Suitable for procedural universe generation
- No external texture assets required

### Parameter Philosophy
- **Sliders**: Most parameters are 0.0-1.0 range
- **Seed-based**: Randomness comes from hash functions, not random()
- **Composable**: Mix features for endless variety
- **Performance-aware**: Octaves and sample counts control quality/speed

## Performance Guide

### Optimization Hierarchy
1. **LOD System** (most important)
   - Near: Full detail (octaves=6-8)
   - Medium: Reduced (octaves=4)
   - Far: Minimal (octaves=2-3) or billboard

2. **Feature Culling**
   - Disable atmosphere for distant planets
   - Skip corona for off-screen stars
   - Reduce ring segments for distant objects

3. **Sample Counts**
   - Atmosphere: 8-16 samples (16-32 for hero planets)
   - Terrain: 3-8 octaves based on distance
   - Corona: Can be disabled entirely when not visible

### Typical Budget (1080p, 60fps)
- Hero planet (on-screen focus): ~3-5ms
- Background planet: ~0.5-1ms
- Distant planets: <0.1ms (simplified)
- Star with corona: ~2-4ms

## Color Space & Lighting

All shaders work in **linear color space**:
- Input colors in sRGB get converted automatically by Bevy
- Emissive values can exceed 1.0 for HDR bloom
- PBR materials use standard metallic/roughness workflow

Sun/light direction is typically passed as normalized `vec3`:
```rust
sun_direction: Vec3::new(0.7, 0.5, 0.3).normalize()
```

## Common Patterns

### Pattern 1: Earth-like Planet
```rust
planet_type: 0 (Rocky)
+ moderate craters (0.15)
+ continents/oceans (0.6/0.4)
+ clouds (0.5)
+ blue atmosphere
+ polar ice caps (0.2)
+ city lights (0.3)
```

### Pattern 2: Gas Giant with Rings
```rust
planet_type: 4 (Gas Giant)
+ many bands (10-15)
+ storms (0.6-0.8)
+ thick atmosphere (0.4-0.5)
+ ring system (inner: 1.2x, outer: 2.4x radius)
```

### Pattern 3: Lava World
```rust
planet_type: 2 (Lava)
+ high volcano density (0.7+)
+ rough terrain (0.8+)
+ glowing emissive secondary color
+ thin volcanic atmosphere (0.1-0.2)
```

### Pattern 4: Dead Moon
```rust
planet_type: 5 (Moon)
+ very high crater density (0.8+)
+ no atmosphere (0.0)
+ gray monochrome palette
+ high roughness (0.9)
```

## Integration Examples

### Bevy Material Setup
```rust
#[derive(AsBindGroup, TypePath, Asset, Clone)]
pub struct PlanetMaterial {
    #[uniform(0)]
    pub params: PlanetParams,
}

impl Material for PlanetMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/planet_core.wgsl".into()
    }
    
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
```

### Update Time Each Frame
```rust
fn update_planets(
    time: Res<Time>,
    mut query: Query<&mut Handle<PlanetMaterial>>,
    mut materials: ResMut<Assets<PlanetMaterial>>,
) {
    for material_handle in query.iter() {
        if let Some(material) = materials.get_mut(material_handle) {
            material.params.time = time.elapsed_seconds();
        }
    }
}
```

### Spawn Planet with Atmosphere
```rust
fn spawn_earth(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_mats: ResMut<Assets<PlanetMaterial>>,
    mut atmo_mats: ResMut<Assets<AtmosphereMaterial>>,
) {
    let planet_radius = 10.0;
    let atmo_radius = planet_radius * 1.08;
    
    // Core planet
    let planet_mesh = meshes.add(Sphere::new(planet_radius).mesh().ico(5).unwrap());
    let planet_material = planet_mats.add(PlanetMaterial {
        params: earth_params(),
    });
    
    let planet_entity = commands.spawn(MaterialMeshBundle {
        mesh: planet_mesh,
        material: planet_material,
        ..default()
    }).id();
    
    // Atmosphere layer
    let atmo_mesh = meshes.add(Sphere::new(atmo_radius).mesh().ico(4).unwrap());
    let atmo_material = atmo_mats.add(AtmosphereMaterial {
        params: earth_atmosphere_params(),
    });
    
    commands.spawn(MaterialMeshBundle {
        mesh: atmo_mesh,
        material: atmo_material,
        ..default()
    }).set_parent(planet_entity);
}
```

## Troubleshooting

### Planet appears flat/solid colored
- Check that mesh has proper normals pointing outward
- Verify seed is not 0 (use any other value)
- Ensure terrain_octaves > 0

### Atmosphere not visible
- Increase atmosphere_thickness (try 0.3-0.5)
- Check that atmosphere sphere is slightly larger than planet
- Verify material is set to Transparent blend mode

### Rings look wrong
- Ensure ring mesh is flat (Y=0 plane)
- Check inner_radius < outer_radius
- Verify planet_center matches planet position

### Performance issues
- Reduce terrain_octaves (try 3-4)
- Lower atmosphere sample_count
- Disable effects for distant objects
- Use LOD system

### Colors look washed out
- Input colors in sRGB space (0-1 range)
- Check that color components don't all sum to gray
- Increase lighting intensity

## Best Practices

1. **Seed Management**: Use a hash of planet coordinates for deterministic universe generation
2. **Parameter Validation**: Clamp user inputs to valid ranges
3. **LOD Transitions**: Smoothly interpolate octaves/quality based on distance
4. **Caching**: Generate static planets once, cache the rendered result
5. **Batching**: Group similar planet types for efficient rendering
6. **Profiling**: Use GPU profiler to identify bottlenecks

## Extending the System

### Adding New Planet Types
1. Add type to `planet_type` enum in `planet_core.wgsl`
2. Create generator function (see `generate_rocky_planet` as template)
3. Add color scheme and feature parameters
4. Document in `README_PLANETS.md`

### Custom Features
- Add new noise octave in feature function
- Gate with parameter (density, intensity, etc.)
- Blend with existing surface data
- Test performance impact

### Atmosphere Extensions
- Add new scattering layer in `planet_atmosphere.wgsl`
- Implement new optical depth calculation
- Blend with existing contributions
- Consider performance budget

## References & Credits

### Noise Functions
- PCG hash: [www.pcg-random.org](http://www.pcg-random.org/)
- Simplex noise: Stefan Gustavson, Ian McEwan
- WGSL implementations: [munrocket's gist](https://gist.github.com/munrocket/236ed5ba7e409b8bdf1ff6eca5dcdc39)

### Techniques
- FBM: Inigo Quilez ([iquilezles.org](https://iquilezles.org/))
- Atmospheric scattering: Sean O'Neil, GPU Gems 2
- Procedural patterns: Shadertoy community

### License
MIT License where applicable. Original noise implementations retain their respective licenses.

## Support & Community

For questions, examples, and discussion:
- Check `README_PLANETS.md` for detailed parameter docs
- See `QUICK_REFERENCE.md` for preset recipes
- Review shader source comments for implementation details

## Changelog

### v1.0 (Current)
- Complete planet type coverage (7 types)
- Atmospheric scattering system
- Stellar corona effects
- Planetary ring systems
- Normal map generation
- Full parameter documentation
- Performance optimization guide
