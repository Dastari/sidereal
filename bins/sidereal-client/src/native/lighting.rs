use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;
use sidereal_game::{
    EnvironmentLightingState, PlanetBodyShaderSettings, SizeM, WorldPosition,
    resolve_world_position,
};

use super::backdrop::{RuntimeEffectKind, RuntimeEffectMaterial};
use super::components::{RuntimeWorldVisualPass, RuntimeWorldVisualPassKind};

#[derive(Debug, Clone, Resource)]
pub(crate) struct WorldLightingState {
    pub primary_direction: Vec3,
    pub primary_elevation: f32,
    pub primary_source_position: Option<Vec2>,
    pub primary_color: Vec3,
    pub primary_intensity: f32,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub backlight_color: Vec3,
    pub backlight_intensity: f32,
    pub event_flash_color: Vec3,
    pub event_flash_intensity: f32,
}

impl Default for WorldLightingState {
    fn default() -> Self {
        Self {
            primary_direction: Vec3::new(0.76, 0.58, 0.82).normalize_or_zero(),
            primary_elevation: 0.82,
            primary_source_position: None,
            primary_color: Vec3::new(1.0, 0.97, 0.92),
            primary_intensity: 1.0,
            ambient_color: Vec3::new(0.22, 0.3, 0.42),
            ambient_intensity: 0.18,
            backlight_color: Vec3::new(0.28, 0.42, 0.62),
            backlight_intensity: 0.16,
            event_flash_color: Vec3::new(1.0, 0.95, 0.88),
            event_flash_intensity: 0.0,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct LocalLightEmitter {
    pub source_entity: Entity,
    pub world_position: Vec2,
    pub color: Vec3,
    pub intensity: f32,
    pub radius_m: f32,
    pub priority: f32,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct CameraLocalLightSet {
    pub emitters: Vec<LocalLightEmitter>,
}

impl WorldLightingState {
    pub fn resolved_primary_direction(&self, world_position: Vec2) -> Vec3 {
        let fallback = self.primary_direction.normalize_or_zero();
        let Some(source_position) = self.primary_source_position else {
            return fallback;
        };
        let to_light = source_position - world_position;
        if to_light.length_squared() <= 0.0001 {
            return fallback;
        }
        Vec3::new(to_light.x, to_light.y, self.primary_elevation.max(0.01)).normalize_or_zero()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ResolvedLocalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub radius_m: f32,
}

pub(crate) fn resolve_local_light_for_position(
    camera_local_lights: &CameraLocalLightSet,
    world_position: Vec2,
) -> ResolvedLocalLight {
    let mut best: Option<(f32, ResolvedLocalLight)> = None;
    for emitter in &camera_local_lights.emitters {
        let to_light = emitter.world_position - world_position;
        let distance = to_light.length();
        let falloff = 1.0 - (distance / emitter.radius_m.max(0.001));
        if falloff <= 0.0 {
            continue;
        }
        let weight = emitter.intensity * falloff * falloff;
        if weight <= 0.001 {
            continue;
        }
        let direction = if to_light.length_squared() > 0.0001 {
            Vec3::new(to_light.x, to_light.y, 0.35).normalize_or_zero()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let resolved = ResolvedLocalLight {
            direction,
            color: emitter.color,
            intensity: weight,
            radius_m: emitter.radius_m,
        };
        if best.is_none_or(|(current, _)| weight > current) {
            best = Some((weight, resolved));
        }
    }
    best.map(|(_, light)| light).unwrap_or_default()
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_world_lighting_state_system(
    mut world_lighting: ResMut<'_, WorldLightingState>,
    lighting_query: Query<'_, '_, (Entity, &'_ EnvironmentLightingState)>,
    star_query: Query<
        '_,
        '_,
        (
            Entity,
            &'_ PlanetBodyShaderSettings,
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
    let Some((_, lighting)) = selected else {
        return;
    };
    world_lighting.primary_direction = Vec3::new(
        lighting.primary_direction_xy.x,
        lighting.primary_direction_xy.y,
        lighting.primary_elevation.max(0.01),
    )
    .normalize_or_zero();
    world_lighting.primary_elevation = lighting.primary_elevation.max(0.01);
    world_lighting.primary_color = lighting.primary_color_rgb;
    world_lighting.primary_intensity = lighting.primary_intensity.max(0.0);
    world_lighting.ambient_color = lighting.ambient_color_rgb;
    world_lighting.ambient_intensity = lighting.ambient_intensity.max(0.0);
    world_lighting.backlight_color = lighting.backlight_color_rgb;
    world_lighting.backlight_intensity = lighting.backlight_intensity.max(0.0);
    world_lighting.event_flash_color = lighting.event_flash_color_rgb;
    world_lighting.event_flash_intensity = lighting.event_flash_intensity.max(0.0);

    let mut selected_star: Option<(Entity, Vec2, f32)> = None;
    for (entity, settings, position, world_position, size_m) in &star_query {
        if !settings.enabled || settings.body_kind != 1 {
            continue;
        }
        let Some(source_position) = resolve_world_position(position, world_position) else {
            continue;
        };
        let radius_m = size_m
            .map(|size| size.length.max(size.width).max(size.height) * 0.5)
            .unwrap_or(0.0);
        if selected_star.is_none_or(|(current, _, _)| entity.index() < current.index()) {
            selected_star = Some((entity, source_position, radius_m));
        }
    }
    world_lighting.primary_source_position = selected_star.map(|(_, position, _)| position);
}

pub(super) fn collect_thruster_local_light_emitters_system(
    mut camera_local_lights: ResMut<'_, CameraLocalLightSet>,
    plume_children: Query<
        '_,
        '_,
        (
            Entity,
            &'_ RuntimeWorldVisualPass,
            &'_ GlobalTransform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ ViewVisibility,
        ),
        (),
    >,
    plume_materials: Res<'_, Assets<RuntimeEffectMaterial>>,
) {
    let mut emitters = Vec::new();
    for (entity, pass, transform, material_handle, view_visibility) in &plume_children {
        if pass.kind != RuntimeWorldVisualPassKind::ThrusterPlume {
            continue;
        }
        if !view_visibility.get() {
            continue;
        }
        let Some(material) = plume_materials.get(&material_handle.0) else {
            continue;
        };
        if (material.params.identity_a.x - RuntimeEffectKind::BillboardThruster as u32 as f32).abs()
            >= 0.5
        {
            continue;
        }
        let thrust_alpha = material.params.identity_a.z.clamp(0.0, 1.0);
        let afterburner_alpha = material.params.params_b.x.clamp(0.0, 1.0);
        let alpha = material.params.identity_a.w.clamp(0.0, 1.0);
        let intensity = alpha * (0.35 + thrust_alpha * 1.2 + afterburner_alpha * 1.4);
        if intensity <= 0.02 {
            continue;
        }
        let scale = transform.to_scale_rotation_translation().0;
        let radius_m = scale.y.max(scale.x * 0.28).max(2.0);
        let color = material
            .params
            .color_a
            .xyz()
            .lerp(material.params.color_c.xyz(), afterburner_alpha);
        emitters.push(LocalLightEmitter {
            source_entity: entity,
            world_position: transform.translation().truncate(),
            color,
            intensity,
            radius_m,
            priority: intensity * radius_m,
        });
    }
    emitters.sort_by(|a, b| b.priority.total_cmp(&a.priority));
    emitters.truncate(8);
    camera_local_lights.emitters = emitters;
}
