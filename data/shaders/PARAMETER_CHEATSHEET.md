# Planet Shader Parameter Cheat Sheet

## Active Runtime Identity

```text
body_kind:        0=Planet, 1=Star, 2=Black Hole
planet_type:      0=Terran, 1=Desert, 2=Lava, 3=Ice, 4=Gas Giant, 5=Moon
seed:             any u32
rotation_speed:   -1.0 to 1.0
```

## Surface / Terrain

```text
continent_size:   0.0 islands ---------------------------- 1.0 supercontinents
ocean_level:      0.0 water world ------------------------ 1.0 dry world
mountain_height:  0.0 flat ------------------------------- 1.0 dramatic
roughness:        0.0 smooth ----------------------------- 1.0 noisy
terrain_octaves:  1 simple ------------------------------- 8 detailed
terrain_lacunarity: 1.1 low frequency growth ------------ 4.0 aggressive
terrain_gain:     0.1 low contribution ------------------- 0.95 high contribution
crater_density:   0.0 none ------------------------------- 1.0 dense
crater_size:      0.0 small ------------------------------ 1.0 huge
volcano_density:  0.0 none ------------------------------- 1.0 dense
ice_cap_size:     0.0 none ------------------------------- 1.0 large caps
```

## Clouds / Atmosphere

```text
cloud_coverage:   0.0 clear ------------------------------ 1.0 overcast
cloud_scale:      0.1 broad ------------------------------ 6.0 tight detail
cloud_speed:     -2.0 reverse ---------------------------- 2.0 forward
cloud_alpha:      0.0 invisible -------------------------- 1.0 opaque
cloud_shadow_strength: 0.0 none ------------------------- 1.0 strong
atmosphere_thickness: 0.0 none -------------------------- 0.4 thick
atmosphere_falloff:   0.5 soft -------------------------- 8.0 sharp
atmosphere_alpha:     0.0 none -------------------------- 1.0 strong
corona_intensity:     0.0 none -------------------------- 2.0 strong
```

## Lighting / Response

```text
normal_strength:  0.0 flat ------------------------------- 2.0 strong bump
light_wrap:       0.0 hard terminator -------------------- 1.0 wrapped light
ambient_strength: 0.0 dark ------------------------------- 1.0 bright fill
specular_strength: 0.0 matte ---------------------------- 3.0 glossy
specular_power:   1 broad highlight ---------------------- 64 tight highlight
rim_strength:     0.0 none ------------------------------- 2.0 strong
rim_power:        0.5 soft rim --------------------------- 8.0 tight rim
fresnel_strength: 0.0 none ------------------------------- 2.0 strong
night_glow_strength: 0.0 none --------------------------- 1.0 visible
city_lights:      0.0 none ------------------------------- 1.0 bright
emissive_strength: 0.0 none ----------------------------- 2.0 strong
```

## Gas Giant / Star / Black Hole Controls

```text
bands_count:      0 none --------------------------------- 24 many
spot_density:     0.0 none ------------------------------- 1.0 dense
surface_activity: 0.0 calm ------------------------------- 1.0 turbulent
```

## Current Runtime Rule

- `planet_body.wgsl` owns the globe body only.
- `planet_clouds.wgsl` owns cloud overlays.
- `planet_ring.wgsl` owns rings and black-hole accretion disks.
