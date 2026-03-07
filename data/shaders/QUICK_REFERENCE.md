# Planet Shader System - Quick Reference

## Active Runtime Stack

### Planet bodies (`body_kind = 0`)
1. `planet_body.wgsl`
2. optional `planet_clouds.wgsl`
3. optional `planet_ring.wgsl` for hero gas giants

### Stars (`body_kind = 1`)
1. `planet_body.wgsl` star branch
2. optional future dedicated atmosphere/corona shell pass

### Black holes (`body_kind = 2`)
1. `planet_body.wgsl` event-horizon branch
2. `planet_ring.wgsl` accretion disk

## Surface Families (`planet_type`)
- `0` terran / oceanic
- `1` desert
- `2` lava / volcanic
- `3` ice / frozen
- `4` gas giant
- `5` moon / rocky

## Useful Preset Direction

### Earth-like
- `body_kind: 0`
- `planet_type: 0`
- high `cloud_coverage`
- moderate `ocean_level`
- moderate `ice_cap_size`
- blue `color_tertiary_rgb`
- white `color_clouds_rgb`

### Gas giant
- `body_kind: 0`
- `planet_type: 4`
- high `bands_count`
- medium/high `spot_density`
- high `storm_intensity`
- optional ring pass triggered by hero tuning

### Star
- `body_kind: 1`
- `planet_type` unused for surface selection
- high `corona_intensity`
- high `emissive_strength`
- warm `color_primary_rgb` / `color_emissive_rgb`

### Black hole
- `body_kind: 2`
- `planet_type` unused for surface selection
- low body albedo
- higher `corona_intensity`
- brighter `color_atmosphere_rgb` and `color_emissive_rgb`
- accretion disk comes from `planet_ring.wgsl`

## Current Architectural Rule

Do not put clouds or rings back into `planet_body.wgsl`. The body pass owns the globe. Clouds and accretion/rings are separate passes.
