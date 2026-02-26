# Dynamic Procedural Asteroid Visuals - Planning & Analysis

**Date:** 2026-02-18  
**Status:** 📋 Planning Phase

## Current State

### Asteroids in the System

**Data Layer** (`sidereal-persistence/src/lib.rs`):
- 120 asteroids seeded via `seed_graph_asteroid_field_records()`
- Each asteroid has:
  - Position: Spiral distribution (350-2000m radius from origin)
  - Mass: 1,000-31,000 kg
  - Size: 4-28m diameter (stored as `size_m`)
  - Health: 60-300 HP
  - Components: `ShardAssignment`, `MassKg`, `TotalMassKg`, `SizeM`, `CollisionAabbM`, `HealthPool`, `DisplayName`
  - Labels: `Entity`, `Asteroid`

**Server Layer** (`sidereal-replication`):
- Hydrated from graph database on startup
- Physical simulation via Avian2D (implied by collision AABBs)
- No special rendering/mesh - just ECS components

**Client Layer** (`sidereal-client`):
- `spawn_entity_visual()` creates placeholder sprites
- Currently renders as **simple colored shapes** (circles/capsules)
- Uses `body_half_extents_from_state()` for sizing
- Already supports `asset_id` field (optional)
- Has `VisualAssetBinding` component

### Current Rendering Pipeline

```rust
// bins/sidereal-client/src/native.rs
fn attach_streamed_visual_assets_system() {
    // Creates placeholder sprite/quad first.
    // Resolves streamed sprite assets by VisualAssetId and swaps in when ready.
    // Optional SpriteShaderAssetId adds a pixel shader material path.
}
```

## Asset Delivery Plan Compatibility ✅

Your asset delivery contract is **well-aligned** for procedural asteroids.

### Key Alignment Points

1. **Placeholder-First Rendering** ✅
   - Plan: "Spawn with placeholder immediately, swap when asset resolves"
   - Perfect for procedural: Generate mesh async, show simple shape while generating

2. **Asset ID System** ✅
   - Plan: Entities reference `asset_id` + dimensions
   - Asteroids already have: `entity_id`, `size_m`, position
   - Can use: `asset_id: "asteroid:procedural:v1"` or seed-based ID

3. **Deterministic Dimensions** ✅
   - Plan: "Authoritative physical dimensions already replicated"
   - Asteroids: `size_m` defines both visual AND collision bounds

4. **On-Demand Loading** ✅
   - Plan: "Load when first visible/in-range"
   - Procedural: Generate mesh when asteroid enters viewport

5. **Memory Budget** ✅
   - Plan: Eviction by LRU/TTL when unreferenced
   - Procedural: Cache generated meshes, evict far asteroids

## Procedural Generation Approach

### Option 1: CPU Mesh Generation (Recommended)

**Generate meshes on CPU using deterministic algorithm seeded by entity_id**

```rust
fn generate_asteroid_mesh(
    seed: u32,
    radius: f32,
    detail_level: u32, // LOD
) -> Mesh {
    // 1. Start with icosphere or UV sphere
    // 2. Apply noise-based displacement:
    //    - Perlin/Simplex noise for large features
    //    - Multi-octave for detail (craters, bumps)
    // 3. Add crater impacts (random positions seeded)
    // 4. Generate normals for lighting
    // 5. UV unwrap for texturing
}
```

**Pros:**
- Fully deterministic (same seed = same mesh)
- Can be cached in asset system
- Works with existing asset pipeline
- Can generate multiple LODs
- Meshes can be saved/loaded later

**Cons:**
- Initial generation cost (but async/threaded)
- Memory for generated meshes

### Option 2: GPU Shader Displacement

**Use vertex shader to displace sphere mesh**

```wgsl
@vertex
fn vertex(
    @location(0) position: vec3<f32>,
    @builtin(vertex_index) vertex_idx: u32,
) -> VertexOutput {
    // Sample 3D noise texture based on position
    let noise = sample_noise(position, asteroid_seed);
    let displaced = position * (1.0 + noise * roughness);
    // ... project to screen space
}
```

**Pros:**
- Minimal memory (just base sphere mesh)
- Extremely fast (GPU parallel)
- Can animate/rotate detail without regenerating

**Cons:**
- Collision mesh still needs CPU generation
- Less control over exact crater placement
- Requires noise textures

### Option 3: Hybrid (Best Approach)

**Combine both for optimal quality/performance**

```
1. CPU generates collision mesh (simple, low-poly)
2. CPU generates base visual mesh (medium detail)
3. GPU shader adds fine surface detail
4. GPU shader applies procedural material
```

## Procedural Material/Texture

### Approach: Shader-Based Procedural Texturing

**No texture files needed - all generated in fragment shader**

```wgsl
@fragment
fn fragment(
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let seed = material.asteroid_seed;
    
    // Base rock color
    let base_color = rock_color_variation(world_pos, seed);
    
    // Crater darkening
    let crater_factor = crater_pattern(world_pos, seed);
    
    // Mineral veins
    let mineral = mineral_veins(world_pos, seed);
    
    // Combine with lighting
    let lit = pbr_lighting(base_color, normal, roughness, metallic);
    
    return lit + mineral * emissive;
}
```

**Visual Features:**

1. **Base Rock Texture**
   - 3D Perlin noise for color variation
   - Voronoi cells for rock facets
   - Roughness map from noise

2. **Craters**
   - Scattered impact sites (seeded random positions)
   - Radial gradient darkening
   - Rim highlights
   - Depth via normal mapping

3. **Mineral Deposits**
   - Rare "veins" using noise thresholds
   - Emissive glow (blue/green/gold)
   - Metallic PBR properties
   - Could indicate minable resources

4. **Surface Detail**
   - Micro-bumps from high-frequency noise
   - Dust/regolith variation
   - Erosion patterns

## Implementation Plan

### Phase 1: Basic Procedural Mesh
```rust
// Add to client
struct ProceduralAsteroidMesh {
    seed: u32,
    radius: f32,
    mesh_handle: Handle<Mesh>,
}

fn generate_asteroid_system(
    commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(Entity, &EntityGuid, &SizeM), Added<AsteroidTag>>,
) {
    for (entity, guid, size) in &query {
        let seed = guid_to_seed(guid.0);
        let mesh = generate_asteroid_mesh(seed, size.length * 0.5, 2);
        let mesh_handle = meshes.add(mesh);
        
        commands.entity(entity).insert(ProceduralAsteroidMesh {
            seed,
            radius: size.length * 0.5,
            mesh_handle,
        });
    }
}
```

### Phase 2: Asset ID Integration
```rust
// Use asset system for caching
let asset_id = format!("asteroid:proc:{}:lod2", seed);

// Check cache first
if let Some(cached) = catalog.entries.get(&asset_id) {
    // Use cached mesh
} else {
    // Generate and cache
    let mesh = generate_asteroid_mesh(seed, radius, 2);
    register_procedural_asset(&asset_id, mesh);
}
```

### Phase 3: Procedural Material
```rust
// Custom material with seed parameter
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct AsteroidMaterial {
    #[uniform(0)]
    asteroid_seed: u32,
    #[uniform(1)]
    base_color: Color,
    #[uniform(2)]
    roughness: f32,
    #[uniform(3)]
    metallic: f32,
    #[texture(4)]
    #[sampler(5)]
    noise_texture: Handle<Image>,
}

impl Material for AsteroidMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/asteroid.wgsl".into()
    }
}
```

### Phase 4: LOD System
```rust
// Generate multiple detail levels
fn generate_asteroid_lods(seed: u32, radius: f32) -> AsteroidLods {
    AsteroidLods {
        lod0: generate_asteroid_mesh(seed, radius, 4), // Near: 1024 verts
        lod1: generate_asteroid_mesh(seed, radius, 3), // Mid: 256 verts
        lod2: generate_asteroid_mesh(seed, radius, 2), // Far: 64 verts
        lod3: generate_asteroid_mesh(seed, radius, 1), // Distant: sphere
    }
}

// Switch based on camera distance
fn update_asteroid_lod(
    mut query: Query<(&Transform, &mut Handle<Mesh>)>,
    camera: Query<&Transform, With<Camera>>,
) {
    let camera_pos = camera.single().translation;
    for (transform, mut mesh) in &mut query {
        let distance = transform.translation.distance(camera_pos);
        *mesh = select_lod_mesh(distance);
    }
}
```

## Mesh Generation Algorithm

### Icosphere-Based Approach

```rust
fn generate_asteroid_mesh(seed: u32, radius: f32, subdivisions: u32) -> Mesh {
    // 1. Generate icosphere base
    let mut positions = generate_icosphere_vertices(subdivisions);
    let indices = generate_icosphere_indices(subdivisions);
    
    // 2. Apply noise displacement
    let noise = Perlin::new(seed);
    for pos in &mut positions {
        let dir = pos.normalize();
        let noise_val = multi_octave_noise(&noise, *pos, 3);
        *pos = dir * (radius + noise_val * radius * 0.3);
    }
    
    // 3. Add craters
    let crater_positions = generate_crater_positions(seed, 5..15);
    for crater_pos in crater_positions {
        apply_crater_deformation(&mut positions, crater_pos, radius * 0.2);
    }
    
    // 4. Smooth normals
    let normals = compute_smooth_normals(&positions, &indices);
    
    // 5. Generate UVs (spherical projection)
    let uvs = generate_spherical_uvs(&positions);
    
    Mesh::new(PrimitiveTopology::TriangleList)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

fn multi_octave_noise(noise: &Perlin, pos: Vec3, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    
    for _ in 0..octaves {
        value += noise.get([pos.x * frequency, pos.y * frequency, pos.z * frequency]) * amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    value
}

fn apply_crater_deformation(positions: &mut Vec<Vec3>, center: Vec3, radius: f32) {
    for pos in positions {
        let dist = pos.distance(center);
        if dist < radius {
            // Crater profile: depression with raised rim
            let t = dist / radius;
            let depth = if t < 0.7 {
                -0.2 * (1.0 - (t / 0.7).powi(2)) // Bowl
            } else {
                0.1 * ((t - 0.7) / 0.3) // Rim
            };
            *pos += (pos.normalize()) * (radius * depth);
        }
    }
}
```

## Shader Code (Procedural Material)

```wgsl
// shaders/asteroid.wgsl

struct AsteroidMaterial {
    seed: u32,
    base_color: vec4<f32>,
    roughness: f32,
    metallic: f32,
}

@group(2) @binding(0)
var<uniform> material: AsteroidMaterial;

@group(2) @binding(1)
var noise_texture: texture_3d<f32>;
@group(2) @binding(2)
var noise_sampler: sampler;

fn hash(seed: u32, x: f32) -> f32 {
    let h = seed ^ u32(x * 1000.0);
    return f32(h) / 4294967295.0;
}

fn rock_color(world_pos: vec3<f32>, seed: u32) -> vec3<f32> {
    let noise_val = textureSample(noise_texture, noise_sampler, world_pos * 0.1).r;
    
    // Base gray rock
    let gray = vec3<f32>(0.3, 0.28, 0.26);
    
    // Brownish variation
    let brown = vec3<f32>(0.25, 0.2, 0.15);
    
    return mix(gray, brown, noise_val);
}

fn crater_pattern(world_pos: vec3<f32>, seed: u32) -> f32 {
    // Generate crater positions from seed
    var darkening = 0.0;
    for (var i = 0u; i < 10u; i++) {
        let crater_seed = seed + i * 997u;
        let cx = hash(crater_seed, 0.0) * 2.0 - 1.0;
        let cy = hash(crater_seed, 1.0) * 2.0 - 1.0;
        let cz = hash(crater_seed, 2.0) * 2.0 - 1.0;
        let crater_pos = normalize(vec3<f32>(cx, cy, cz));
        
        let dist = distance(normalize(world_pos), crater_pos);
        let crater_radius = hash(crater_seed, 3.0) * 0.3 + 0.1;
        
        if (dist < crater_radius) {
            darkening += (1.0 - dist / crater_radius) * 0.4;
        }
    }
    return saturate(1.0 - darkening);
}

fn mineral_veins(world_pos: vec3<f32>, seed: u32) -> vec3<f32> {
    let vein_noise = textureSample(noise_texture, noise_sampler, world_pos * 0.5).g;
    
    // Rare mineral deposits (threshold)
    if (vein_noise > 0.85) {
        // Blue-green emissive veins
        return vec3<f32>(0.2, 0.6, 0.8) * (vein_noise - 0.85) * 5.0;
    }
    
    return vec3<f32>(0.0);
}

@fragment
fn fragment(
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    // Base rock color
    var color = rock_color(world_position, material.seed);
    
    // Apply crater darkening
    color *= crater_pattern(world_position, material.seed);
    
    // Add mineral deposits
    let minerals = mineral_veins(world_position, material.seed);
    color += minerals;
    
    // Simple directional lighting (placeholder for PBR)
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = max(dot(world_normal, light_dir), 0.0);
    color *= (0.3 + 0.7 * ndotl); // Ambient + diffuse
    
    return vec4<f32>(color, 1.0);
}
```

## Performance Considerations

### Memory Budget
- **Per asteroid mesh** (LOD2): ~4KB (256 vertices × 16 bytes)
- **120 asteroids**: ~480KB total
- **With LODs**: ~1.5MB total
- **Well within** 256MB asset budget

### Generation Performance
- **CPU mesh generation**: 1-5ms per asteroid
- **Async/threaded**: Generate during load screens
- **Cache**: Save to IndexedDB for repeat sessions

### Rendering Performance
- **Draw calls**: One per asteroid (instanced if same material)
- **Vertex count**: 256-1024 per asteroid at medium LOD
- **Shader cost**: Moderate (procedural material adds cost)

## Integration with Asset System

### Fits Perfectly with Your Plan

```rust
// Server sends entity state with:
EntityState {
    entity_id: "asteroid:0042",
    x, y, vx, vy,
    asset_id: Some("asteroid:procedural:v1"),  // Generic procedural type
    size_m: Some(12.5),                        // Authoritative dimension
    // ... other fields
}

// Client asset manager:
fn resolve_asteroid_asset(entity_id: Uuid, asset_id: &str, size: f32) {
    let seed = uuid_to_seed(entity_id); // Deterministic from entity ID
    
    // Check cache
    let cache_key = format!("{}:{}:lod2", asset_id, seed);
    if let Some(cached) = asset_cache.get(&cache_key) {
        return cached;
    }
    
    // Generate async
    spawn_task(async move {
        let mesh = generate_asteroid_mesh(seed, size * 0.5, 2);
        let material = AsteroidMaterial::new(seed);
        asset_cache.insert(cache_key, (mesh, material));
    });
    
    // Return placeholder immediately
    PlaceholderMesh::sphere(size)
}
```

## Next Steps (When Ready to Implement)

1. **Add noise library**: `noise-rs` crate for Perlin/Simplex noise
2. **Implement icosphere generator**: Base mesh topology
3. **Create asteroid material**: Custom shader with seed uniform
4. **Add `AsteroidTag` component**: Mark entities for procedural generation
5. **System for mesh generation**: Async task spawner
6. **LOD system**: Distance-based mesh swapping
7. **Cache layer**: Save/load generated meshes

## Conclusion

**✅ Fully Compatible** with your asset delivery plan!

Your existing architecture is **perfectly designed** for procedural asteroids:
- Placeholder-first rendering ✅
- Asset ID system ✅  
- Deterministic dimensions ✅
- On-demand loading ✅
- Memory budget ✅

**Recommended Approach**: Hybrid CPU mesh + GPU shader
- CPU generates collision & base mesh (cacheable)
- GPU adds surface detail & procedural material (minimal memory)
- Deterministic from entity UUID (same asteroid always looks the same)
- Fits within asset streaming paradigm

**No conflicts** with your asset delivery system - procedural generation is just another asset type!
