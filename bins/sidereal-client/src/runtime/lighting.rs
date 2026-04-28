use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use sidereal_game::{
    BallisticProjectile, EnvironmentLightingState, SizeM, StellarLightSource, WorldPosition,
    resolve_world_position,
};

use super::backdrop::{RuntimeEffectKind, RuntimeEffectMaterial};
use super::components::{
    BallisticProjectileVisualAttached, RuntimeWorldVisualPass, RuntimeWorldVisualPassKind,
    WeaponImpactExplosion, WeaponImpactSpark, WeaponTracerBolt,
};

pub(crate) const MAX_STELLAR_LIGHTS: usize = 2;
pub(crate) const MAX_LOCAL_LIGHTS: usize = 8;
const MAX_STELLAR_LIGHT_CANDIDATES: usize = 32;
const MAX_CAMERA_LOCAL_LIGHT_EMITTERS: usize = 64;

const TRACER_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.86, 0.42);
const PROJECTILE_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.78, 0.32);
const IMPACT_SPARK_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.82, 0.46);
const IMPACT_EXPLOSION_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.56, 0.18);
const DESTRUCTION_EXPLOSION_LIGHT_COLOR: Vec3 = Vec3::new(1.0, 0.50, 0.16);

#[derive(Debug, Clone, Resource)]
pub(crate) struct WorldLightingState {
    pub fallback_primary_direction: Vec3,
    pub fallback_primary_elevation: f32,
    pub fallback_primary_color: Vec3,
    pub fallback_primary_intensity: f32,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub backlight_color: Vec3,
    pub backlight_intensity: f32,
    pub event_flash_color: Vec3,
    pub event_flash_intensity: f32,
    pub exposure: f32,
    pub stellar_lights: Vec<RuntimeStellarLight>,
}

impl Default for WorldLightingState {
    fn default() -> Self {
        Self {
            fallback_primary_direction: Vec3::new(0.76, 0.58, 0.36).normalize_or_zero(),
            fallback_primary_elevation: 0.36,
            fallback_primary_color: Vec3::new(1.0, 0.92, 0.78),
            fallback_primary_intensity: 1.15,
            ambient_color: Vec3::new(0.16, 0.20, 0.27),
            ambient_intensity: 0.12,
            backlight_color: Vec3::new(0.28, 0.42, 0.62),
            backlight_intensity: 0.08,
            event_flash_color: Vec3::new(1.0, 0.95, 0.88),
            event_flash_intensity: 0.0,
            exposure: 1.0,
            stellar_lights: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeStellarLight {
    pub source_entity: Entity,
    pub world_position: DVec2,
    pub source_radius_m: f64,
    pub color: Vec3,
    pub intensity: f32,
    pub inner_radius_m: f32,
    pub outer_radius_m: f32,
    pub elevation: f32,
    pub priority: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct LocalLightEmitter {
    pub source_entity: Entity,
    pub world_position: DVec2,
    pub color: Vec3,
    pub intensity: f32,
    pub inner_radius_m: f32,
    pub outer_radius_m: f32,
    pub elevation: f32,
    pub priority: f32,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct CameraLocalLightSet {
    pub emitters: Vec<LocalLightEmitter>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ResolvedLightSlot {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius_m: f32,
}

pub(crate) fn resolve_stellar_lights_for_position(
    state: &WorldLightingState,
    world_position: DVec2,
) -> [ResolvedLightSlot; MAX_STELLAR_LIGHTS] {
    if state.stellar_lights.is_empty() {
        let mut slots = [ResolvedLightSlot::default(); MAX_STELLAR_LIGHTS];
        if state.fallback_primary_intensity > 0.001 {
            slots[0] = ResolvedLightSlot {
                direction: state.fallback_primary_direction.normalize_or_zero(),
                color: state.fallback_primary_color,
                intensity: state.fallback_primary_intensity,
                radius_m: 0.0,
            };
        }
        return slots;
    }

    let mut candidates = Vec::with_capacity(state.stellar_lights.len().min(MAX_STELLAR_LIGHTS));
    for light in &state.stellar_lights {
        let to_light = light.world_position - world_position;
        let distance_m = to_light.length();
        let surface_distance_m = (distance_m - light.source_radius_m).max(0.0);
        let falloff = smooth_light_falloff(
            surface_distance_m as f32,
            light.inner_radius_m,
            light.outer_radius_m,
        );
        let intensity = light.intensity.max(0.0) * falloff * light.priority.max(0.0);
        if intensity <= 0.001 {
            continue;
        }
        let direction = if to_light.length_squared() > 0.0001 {
            Vec3::new(
                to_light.x as f32,
                to_light.y as f32,
                light.elevation.max(0.01),
            )
            .normalize_or_zero()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        candidates.push(ResolvedLightSlot {
            direction,
            color: light.color,
            intensity,
            radius_m: light.outer_radius_m.max(0.0),
        });
    }

    candidates.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
    let mut slots = [ResolvedLightSlot::default(); MAX_STELLAR_LIGHTS];
    for (slot, candidate) in slots.iter_mut().zip(candidates) {
        *slot = candidate;
    }
    slots
}

pub(crate) fn resolve_local_lights_for_position(
    camera_local_lights: &CameraLocalLightSet,
    world_position: DVec2,
) -> [ResolvedLightSlot; MAX_LOCAL_LIGHTS] {
    let mut candidates =
        Vec::with_capacity(camera_local_lights.emitters.len().min(MAX_LOCAL_LIGHTS));
    for emitter in &camera_local_lights.emitters {
        let to_light = emitter.world_position - world_position;
        let distance_m = to_light.length() as f32;
        let falloff =
            smooth_light_falloff(distance_m, emitter.inner_radius_m, emitter.outer_radius_m);
        let intensity = emitter.intensity.max(0.0) * falloff * emitter.priority.max(0.0);
        if intensity <= 0.001 {
            continue;
        }
        let direction = if to_light.length_squared() > 0.0001 {
            Vec3::new(
                to_light.x as f32,
                to_light.y as f32,
                emitter.elevation.max(0.01),
            )
            .normalize_or_zero()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        candidates.push(ResolvedLightSlot {
            direction,
            color: emitter.color,
            intensity,
            radius_m: emitter.outer_radius_m.max(0.0),
        });
    }

    candidates.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
    let mut slots = [ResolvedLightSlot::default(); MAX_LOCAL_LIGHTS];
    for (slot, candidate) in slots.iter_mut().zip(candidates) {
        *slot = candidate;
    }
    slots
}

fn smooth_light_falloff(distance_m: f32, inner_radius_m: f32, outer_radius_m: f32) -> f32 {
    let inner = inner_radius_m.max(0.0);
    let outer = outer_radius_m.max(inner + 1.0);
    let t = ((distance_m - inner) / (outer - inner)).clamp(0.0, 1.0);
    1.0 - (t * t * (3.0 - 2.0 * t))
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_world_lighting_state_system(
    mut world_lighting: ResMut<'_, WorldLightingState>,
    lighting_query: Query<'_, '_, (Entity, &'_ EnvironmentLightingState)>,
    stellar_query: Query<
        '_,
        '_,
        (
            Entity,
            &'_ StellarLightSource,
            Option<&'_ avian2d::prelude::Position>,
            Option<&'_ WorldPosition>,
            Option<&'_ SizeM>,
        ),
    >,
) {
    let mut selected: Option<(Entity, &EnvironmentLightingState)> = None;
    for (entity, lighting) in &lighting_query {
        if selected.is_none_or(|(current, _)| entity.index() < current.index()) {
            selected = Some((entity, lighting));
        }
    }
    if let Some((_, lighting)) = selected {
        world_lighting.fallback_primary_direction = Vec3::new(
            lighting.primary_direction_xy.x,
            lighting.primary_direction_xy.y,
            lighting.primary_elevation.max(0.01),
        )
        .normalize_or_zero();
        world_lighting.fallback_primary_elevation = lighting.primary_elevation.max(0.01);
        world_lighting.fallback_primary_color = lighting.primary_color_rgb;
        world_lighting.fallback_primary_intensity = lighting.primary_intensity.max(0.0);
        world_lighting.ambient_color = lighting.ambient_color_rgb;
        world_lighting.ambient_intensity = lighting.ambient_intensity.max(0.0);
        world_lighting.backlight_color = lighting.backlight_color_rgb;
        world_lighting.backlight_intensity = lighting.backlight_intensity.max(0.0);
        world_lighting.event_flash_color = lighting.event_flash_color_rgb;
        world_lighting.event_flash_intensity = lighting.event_flash_intensity.max(0.0);
    }

    world_lighting.stellar_lights.clear();
    for (entity, stellar, position, world_position, size_m) in &stellar_query {
        if !stellar.enabled {
            continue;
        }
        let Some(source_position) = resolve_world_position(position, world_position) else {
            continue;
        };
        let source_radius_m = size_m
            .map(|size| size.length.max(size.width).max(size.height) * 0.5)
            .unwrap_or(0.0)
            .max(0.0) as f64;
        world_lighting.stellar_lights.push(RuntimeStellarLight {
            source_entity: entity,
            world_position: source_position,
            source_radius_m,
            color: stellar.color_rgb,
            intensity: stellar.intensity.max(0.0),
            inner_radius_m: stellar.inner_radius_m.max(0.0),
            outer_radius_m: stellar.outer_radius_m.max(stellar.inner_radius_m + 1.0),
            elevation: stellar.elevation.max(0.01),
            priority: stellar.priority.max(0.0),
        });
    }
    world_lighting.stellar_lights.sort_by(|a, b| {
        b.priority
            .total_cmp(&a.priority)
            .then_with(|| a.source_entity.index().cmp(&b.source_entity.index()))
    });
    world_lighting
        .stellar_lights
        .truncate(MAX_STELLAR_LIGHT_CANDIDATES);
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_camera_local_light_emitters_system(
    mut camera_local_lights: ResMut<'_, CameraLocalLightSet>,
    plume_children: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RuntimeWorldVisualPass,
            &'_ GlobalTransform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            Option<&'_ ViewVisibility>,
            Option<&'_ Visibility>,
        ),
    >,
    projectile_visuals: Query<
        '_,
        '_,
        (
            Entity,
            &'_ GlobalTransform,
            Option<&'_ ViewVisibility>,
            Option<&'_ Visibility>,
        ),
        (
            With<BallisticProjectile>,
            With<BallisticProjectileVisualAttached>,
        ),
    >,
    tracer_bolts: Query<
        '_,
        '_,
        (
            Entity,
            &'_ GlobalTransform,
            &'_ WeaponTracerBolt,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            Option<&'_ ViewVisibility>,
            Option<&'_ Visibility>,
        ),
    >,
    sparks: Query<
        '_,
        '_,
        (
            Entity,
            &'_ GlobalTransform,
            &'_ WeaponImpactSpark,
            Option<&'_ ViewVisibility>,
            Option<&'_ Visibility>,
        ),
    >,
    explosions: Query<
        '_,
        '_,
        (
            Entity,
            &'_ GlobalTransform,
            &'_ WeaponImpactExplosion,
            Option<&'_ ViewVisibility>,
            Option<&'_ Visibility>,
        ),
    >,
    effect_materials: Res<'_, Assets<RuntimeEffectMaterial>>,
) {
    let mut emitters = Vec::new();

    for (entity, pass, transform, material_handle, view_visibility, visibility) in &plume_children {
        if pass.kind != RuntimeWorldVisualPassKind::ThrusterPlume
            || !presentation_light_visible(view_visibility, visibility)
        {
            continue;
        }
        let Some(material) = effect_materials.get(&material_handle.0) else {
            continue;
        };
        let scale = transform.to_scale_rotation_translation().0;
        let kind = material.params.identity_a.x;
        if (kind - RuntimeEffectKind::BillboardThruster as u32 as f32).abs() >= 0.5 {
            continue;
        }
        let thrust_alpha = material.params.identity_a.z.clamp(0.0, 1.0);
        let afterburner_alpha = material.params.params_b.x.clamp(0.0, 1.0);
        let alpha = material.params.identity_a.w.clamp(0.0, 1.0);
        let intensity = alpha * (0.35 + thrust_alpha * 1.2 + afterburner_alpha * 1.4);
        if intensity <= 0.02 {
            continue;
        }
        let radius_m = scale.y.max(scale.x * 0.28).max(2.0);
        let color = material
            .params
            .color_a
            .xyz()
            .lerp(material.params.color_c.xyz(), afterburner_alpha);
        emitters.push(local_emitter(
            entity,
            transform.translation().truncate().as_dvec2(),
            color,
            intensity,
            radius_m * 0.35,
            radius_m,
            0.35,
        ));
    }

    for (entity, transform, view_visibility, visibility) in &projectile_visuals {
        if !presentation_light_visible(view_visibility, visibility) {
            continue;
        }
        emitters.push(local_emitter(
            entity,
            transform.translation().truncate().as_dvec2(),
            PROJECTILE_LIGHT_COLOR,
            0.9,
            8.0,
            80.0,
            0.25,
        ));
    }

    for (entity, transform, bolt, material_handle, view_visibility, visibility) in &tracer_bolts {
        if bolt.ttl_s <= 0.0 || !presentation_light_visible(view_visibility, visibility) {
            continue;
        }
        let alpha = effect_materials
            .get(&material_handle.0)
            .map(|material| material.params.identity_a.z.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let intensity = 1.3 * alpha;
        if intensity <= 0.02 {
            continue;
        }
        emitters.push(local_emitter(
            entity,
            transform.translation().truncate().as_dvec2(),
            TRACER_LIGHT_COLOR,
            intensity,
            20.0,
            140.0,
            0.25,
        ));
    }

    for (entity, transform, spark, view_visibility, visibility) in &sparks {
        if spark.ttl_s <= 0.0 || !presentation_light_visible(view_visibility, visibility) {
            continue;
        }
        let ttl_norm = (spark.ttl_s / spark.max_ttl_s.max(0.001)).clamp(0.0, 1.0);
        emitters.push(local_emitter(
            entity,
            transform.translation().truncate().as_dvec2(),
            IMPACT_SPARK_LIGHT_COLOR,
            2.4 * ttl_norm,
            10.0,
            160.0,
            0.45,
        ));
    }

    for (entity, transform, explosion, view_visibility, visibility) in &explosions {
        if explosion.ttl_s <= 0.0 || !presentation_light_visible(view_visibility, visibility) {
            continue;
        }
        let ttl_norm = (explosion.ttl_s / explosion.max_ttl_s.max(0.001)).clamp(0.0, 1.0);
        let is_destruction = explosion.screen_distortion_scale > 0.01;
        let (color, intensity, inner_radius_m, outer_radius_m, elevation) = if is_destruction {
            (
                DESTRUCTION_EXPLOSION_LIGHT_COLOR,
                4.0 * ttl_norm,
                50.0,
                420.0,
                0.60,
            )
        } else {
            (
                IMPACT_EXPLOSION_LIGHT_COLOR,
                3.2 * ttl_norm,
                25.0,
                260.0,
                0.55,
            )
        };
        emitters.push(local_emitter(
            entity,
            transform.translation().truncate().as_dvec2(),
            color,
            intensity,
            inner_radius_m,
            outer_radius_m,
            elevation,
        ));
    }

    emitters.sort_by(|a, b| {
        b.priority
            .total_cmp(&a.priority)
            .then_with(|| a.source_entity.index().cmp(&b.source_entity.index()))
    });
    emitters.truncate(MAX_CAMERA_LOCAL_LIGHT_EMITTERS);
    camera_local_lights.emitters = emitters;
}

fn local_emitter(
    source_entity: Entity,
    world_position: DVec2,
    color: Vec3,
    intensity: f32,
    inner_radius_m: f32,
    outer_radius_m: f32,
    elevation: f32,
) -> LocalLightEmitter {
    LocalLightEmitter {
        source_entity,
        world_position,
        color,
        intensity,
        inner_radius_m,
        outer_radius_m,
        elevation,
        priority: intensity.max(0.0) * outer_radius_m.max(0.0),
    }
}

fn presentation_light_visible(
    view_visibility: Option<&ViewVisibility>,
    visibility: Option<&Visibility>,
) -> bool {
    if view_visibility.is_some_and(|value| !value.get()) {
        return false;
    }
    !matches!(visibility, Some(Visibility::Hidden))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stellar_falloff_full_inside_inner_radius() {
        assert_eq!(smooth_light_falloff(50.0, 100.0, 200.0), 1.0);
    }

    #[test]
    fn stellar_falloff_zero_after_outer_radius() {
        assert_eq!(smooth_light_falloff(250.0, 100.0, 200.0), 0.0);
    }

    #[test]
    fn stellar_resolution_uses_fallback_only_when_no_stellar_sources_exist() {
        let state = WorldLightingState::default();
        let slots = resolve_stellar_lights_for_position(&state, DVec2::ZERO);
        assert!(slots[0].intensity > 0.0);
        assert_eq!(slots[1].intensity, 0.0);
    }

    #[test]
    fn stellar_resolution_zeroes_slots_when_sources_are_out_of_range() {
        let mut state = WorldLightingState::default();
        state.stellar_lights.push(RuntimeStellarLight {
            source_entity: Entity::from_bits(1),
            world_position: DVec2::ZERO,
            source_radius_m: 0.0,
            color: Vec3::ONE,
            intensity: 1.0,
            inner_radius_m: 10.0,
            outer_radius_m: 20.0,
            elevation: 0.36,
            priority: 1.0,
        });
        let slots = resolve_stellar_lights_for_position(&state, DVec2::new(100.0, 0.0));
        assert_eq!(slots[0].intensity, 0.0);
        assert_eq!(slots[1].intensity, 0.0);
    }

    #[test]
    fn local_light_resolution_selects_top_eight() {
        let mut lights = CameraLocalLightSet::default();
        for i in 0..12 {
            lights.emitters.push(local_emitter(
                Entity::from_bits((i + 1) as u64),
                DVec2::new(i as f64, 0.0),
                Vec3::ONE,
                (i + 1) as f32,
                0.0,
                100.0,
                0.25,
            ));
        }
        let slots = resolve_local_lights_for_position(&lights, DVec2::ZERO);
        assert!(slots.iter().all(|slot| slot.intensity > 0.0));
        assert!(slots[0].intensity >= slots[7].intensity);
    }
}
