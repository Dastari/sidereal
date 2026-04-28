pub(super) fn ensure_weapon_tracer_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponTracerPool>,
) {
    if !pool.bolts.is_empty() {
        return;
    }
    pool.bolts.reserve(WEAPON_TRACER_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_TRACER_POOL_SIZE {
        let material = effect_materials.add(RuntimeEffectMaterial {
            params: RuntimeEffectUniforms::beam_trail(
                0.0,
                0.0,
                0.65,
                0.35,
                0.12,
                Vec4::new(1.0, 0.96, 0.7, 1.0),
                Vec4::new(1.0, 0.72, 0.22, 1.0),
            ),
            ..RuntimeEffectMaterial::default()
        });
        let bolt = commands
            .spawn((
                WeaponTracerBolt {
                    excluded_entity: None,
                    velocity: Vec2::ZERO,
                    impact_xy: None,
                    ttl_s: 0.0,
                    lateral_normal: Vec2::ZERO,
                    wiggle_phase_rad: 0.0,
                    wiggle_freq_hz: WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ,
                    wiggle_amp_mps: 0.0,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material),
                Transform::from_xyz(0.0, 0.0, 0.35).with_scale(Vec3::new(
                    WEAPON_TRACER_WIDTH_M,
                    WEAPON_TRACER_LENGTH_M,
                    1.0,
                )),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.bolts.push(bolt);
    }
}

pub(super) fn ensure_weapon_impact_spark_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponImpactSparkPool>,
) {
    if !pool.sparks.is_empty() {
        return;
    }
    pool.sparks.reserve(WEAPON_IMPACT_SPARK_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_IMPACT_SPARK_POOL_SIZE {
        let spark = commands
            .spawn((
                WeaponImpactSpark {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_SPARK_TTL_S,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(effect_materials.add(RuntimeEffectMaterial {
                    params: RuntimeEffectUniforms::impact_spark(
                        0.0,
                        1.0,
                        1.0,
                        0.95,
                        Vec4::new(1.0, 0.9, 0.55, 1.0),
                    ),
                    ..RuntimeEffectMaterial::default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.45),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.sparks.push(spark);
    }
}

pub(super) fn ensure_weapon_impact_explosion_pool_system(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut quad_mesh: ResMut<'_, RuntimeSharedQuadMesh>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut pool: ResMut<'_, WeaponImpactExplosionPool>,
) {
    if !pool.explosions.is_empty() {
        return;
    }
    pool.explosions.reserve(WEAPON_IMPACT_EXPLOSION_POOL_SIZE);
    let mesh = shared_unit_quad_handle(&mut quad_mesh, &mut meshes);
    for _ in 0..WEAPON_IMPACT_EXPLOSION_POOL_SIZE {
        let explosion = commands
            .spawn((
                WeaponImpactExplosion {
                    ttl_s: 0.0,
                    max_ttl_s: WEAPON_IMPACT_EXPLOSION_TTL_S,
                    base_scale: 1.2,
                    growth_scale: 4.4,
                    intensity_scale: 1.0,
                    domain_scale: 1.12,
                    screen_distortion_scale: 0.0,
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(effect_materials.add(RuntimeEffectMaterial {
                    params: RuntimeEffectUniforms::explosion_burst(
                        0.0,
                        1.0,
                        1.0,
                        0.92,
                        0.35,
                        1.12,
                        Vec4::new(1.0, 0.92, 0.68, 1.0),
                        Vec4::new(1.0, 0.54, 0.16, 1.0),
                        Vec4::new(0.24, 0.14, 0.08, 1.0),
                    ),
                    ..RuntimeEffectMaterial::default()
                })),
                Transform::from_xyz(0.0, 0.0, 0.43),
                Visibility::Hidden,
                WorldEntity,
                DespawnOnExit(ClientAppState::InWorld),
            ))
            .id();
        pool.explosions.push(explosion);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn bootstrap_local_ballistic_projectile_visual_roots_system(
    mut commands: Commands<'_, '_>,
    projectiles: Query<
        '_,
        '_,
        (Entity, &'_ Position, &'_ avian2d::prelude::Rotation),
        (
            With<BallisticProjectile>,
            Without<WorldEntity>,
            Without<Transform>,
        ),
    >,
) {
    for (entity, position, rotation) in &projectiles {
        let mut transform = Transform::default();
        sync_planar_projectile_transform(&mut transform, position.0, rotation.as_radians());
        let global_transform = GlobalTransform::from(transform);
        commands
            .entity(entity)
            .insert((transform, global_transform, Visibility::Visible));
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_unadopted_ballistic_projectile_visual_roots_system(
    mut projectiles: Query<
        '_,
        '_,
        (
            &'_ Position,
            &'_ avian2d::prelude::Rotation,
            &'_ mut Transform,
        ),
        (With<BallisticProjectile>, Without<WorldEntity>),
    >,
) {
    for (position, rotation, mut transform) in &mut projectiles {
        sync_planar_projectile_transform(&mut transform, position.0, rotation.as_radians());
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn attach_ballistic_projectile_visuals_system(
    mut commands: Commands<'_, '_>,
    projectiles: Query<
        '_,
        '_,
        (
            Entity,
            Has<SuppressedPredictedDuplicateVisual>,
            Option<&'_ Transform>,
        ),
        (
            With<BallisticProjectile>,
            Without<BallisticProjectileVisualAttached>,
        ),
    >,
) {
    for (entity, is_suppressed, existing_transform) in &projectiles {
        let mut transform = existing_transform.copied().unwrap_or_default();
        transform.translation.z = PROJECTILE_VISUAL_Z;
        commands.entity(entity).insert((
            BallisticProjectileVisualAttached,
            Sprite {
                color: Color::srgb(1.0, 0.84, 0.3),
                custom_size: Some(Vec2::new(
                    PROJECTILE_VISUAL_WIDTH_M,
                    PROJECTILE_VISUAL_LENGTH_M,
                )),
                ..default()
            },
            transform,
            if is_suppressed {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
        ));
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_weapon_tracer_visuals_system(
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    connected_clients: Query<
        '_,
        '_,
        (),
        (
            With<lightyear::prelude::client::Client>,
            With<lightyear::prelude::client::Connected>,
        ),
    >,
    mut pool: ResMut<'_, WeaponTracerPool>,
    mut cooldowns: ResMut<'_, WeaponTracerCooldowns>,
    controlled_roots: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ avian2d::prelude::Position,
            &'_ avian2d::prelude::Rotation,
            Option<&'_ avian2d::prelude::LinearVelocity>,
            Option<&'_ avian2d::prelude::AngularVelocity>,
            Option<&'_ ActionState<PlayerInput>>,
        ),
        (With<ControlledEntity>, With<WorldEntity>),
    >,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    weapons: Query<
        '_,
        '_,
        (
            Entity,
            &'_ MountedOn,
            &'_ BallisticWeapon,
            Option<&'_ AmmoCount>,
        ),
        With<WorldEntity>,
    >,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    // Use authoritative server tracer messages for all online clients so
    // impact-stop behavior is identical across shooter/observers.
    if connected_clients.iter().next().is_some() {
        return;
    }

    if pool.bolts.is_empty() {
        return;
    }
    let dt_s = time.delta_secs();
    let mut hardpoint_by_mount = HashMap::<(uuid::Uuid, String), (Vec2, Quat)>::new();
    for (parent_guid, hardpoint) in &hardpoints {
        hardpoint_by_mount.insert(
            (parent_guid.0, hardpoint.hardpoint_id.clone()),
            (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
        );
    }

    for cooldown in cooldowns.by_weapon_entity.values_mut() {
        *cooldown = (*cooldown - dt_s).max(0.0);
    }

    for (
        ship_entity,
        ship_guid,
        ship_position,
        ship_rotation,
        _linear_velocity,
        angular_velocity,
        action_state,
    ) in &controlled_roots
    {
        let firing =
            action_state.is_some_and(|state| state.0.actions.contains(&EntityAction::FirePrimary));
        if !firing {
            continue;
        }
        let ship_quat = Quat::from_rotation_z(ship_rotation.as_radians() as f32);

        for (weapon_entity, mounted_on, weapon, ammo) in &weapons {
            if mounted_on.parent_entity_id != ship_guid.0 {
                continue;
            }
            if weapon.uses_projectile_entities() {
                continue;
            }
            if ammo.is_some_and(|value| value.current == 0) {
                continue;
            }
            let cooldown = cooldowns
                .by_weapon_entity
                .entry(weapon_entity)
                .or_insert(0.0);
            if *cooldown > 0.0 {
                continue;
            }

            let Some((hardpoint_offset, hardpoint_rotation)) = hardpoint_by_mount
                .get(&(mounted_on.parent_entity_id, mounted_on.hardpoint_id.clone()))
            else {
                continue;
            };
            let muzzle_quat = ship_quat * *hardpoint_rotation;
            let direction = (muzzle_quat * Vec3::Y).truncate();
            if direction.length_squared() <= f32::EPSILON {
                continue;
            }
            let direction = direction.normalize();
            let muzzle_offset_world = rotate_vec2(ship_quat, *hardpoint_offset);
            let origin_world = ship_position.0 + muzzle_offset_world.as_dvec2();
            let origin = origin_world.as_vec2();
            let omega = angular_velocity.map(|v| v.0 as f32).unwrap_or(0.0);
            let lateral_normal = Vec2::new(-direction.y, direction.x);
            let spin_wiggle_amp_mps =
                (omega.abs() * 18.0).clamp(0.0, WEAPON_TRACER_WIGGLE_MAX_AMP_MPS);
            let initial_velocity = direction * WEAPON_TRACER_SPEED_MPS;
            let impact_xy = Dir2::new(direction).ok().and_then(|ray_direction| {
                let filter = SpatialQueryFilter::from_excluded_entities([ship_entity]);
                spatial_query
                    .cast_ray(
                        origin_world,
                        ray_direction,
                        f64::from(weapon.max_range_m.max(1.0)),
                        true,
                        &filter,
                    )
                    .map(|hit| origin + ray_direction.as_vec2() * hit.distance as f32)
            });

            let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
            pool.next_index = (pool.next_index + 1) % pool.bolts.len();
            if let Ok((mut transform, _material_handle, mut visibility, mut bolt)) =
                bolts.get_mut(bolt_entity)
            {
                transform.translation = Vec3::new(origin.x, origin.y, 0.35);
                transform.rotation = Quat::from_rotation_z(
                    initial_velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
                );
                bolt.excluded_entity = Some(ship_entity);
                bolt.velocity = initial_velocity;
                bolt.impact_xy = impact_xy;
                let range_ttl_s = (weapon.max_range_m.max(1.0) / WEAPON_TRACER_SPEED_MPS)
                    .clamp(WEAPON_TRACER_MIN_TTL_S, WEAPON_TRACER_LIFETIME_S);
                bolt.ttl_s = range_ttl_s;
                bolt.lateral_normal = lateral_normal;
                bolt.wiggle_phase_rad = 0.0;
                bolt.wiggle_freq_hz = WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ + omega.abs() * 2.0;
                bolt.wiggle_amp_mps = spin_wiggle_amp_mps;
                *visibility = Visibility::Visible;
            }
            *cooldown = weapon.cooldown_seconds();
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn receive_remote_weapon_tracer_messages_system(
    mut pool: ResMut<'_, WeaponTracerPool>,
    mut events: MessageReader<'_, '_, RemoteWeaponFiredRuntimeMessage>,
    controlled_roots: Query<
        '_,
        '_,
        (
            Entity,
            &'_ EntityGuid,
            &'_ avian2d::prelude::Position,
            &'_ avian2d::prelude::Rotation,
        ),
        (With<ControlledEntity>, With<WorldEntity>),
    >,
    hardpoints: Query<'_, '_, (&'_ ParentGuid, &'_ Hardpoint)>,
    weapons: Query<'_, '_, (&'_ EntityGuid, &'_ MountedOn), With<WorldEntity>>,
    world_entity_guids: Query<'_, '_, (Entity, &'_ EntityGuid), With<WorldEntity>>,
    mut bolts: Query<
        '_,
        '_,
        (
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
            &'_ mut WeaponTracerBolt,
        ),
    >,
) {
    if pool.bolts.is_empty() {
        return;
    }
    let local_controlled_by_guid: HashMap<uuid::Uuid, (Entity, DVec2, f64)> = controlled_roots
        .iter()
        .map(|(entity, guid, position, rotation)| {
            (guid.0, (entity, position.0, rotation.as_radians()))
        })
        .collect();
    let hardpoint_by_mount: HashMap<(uuid::Uuid, String), (Vec2, Quat)> = hardpoints
        .iter()
        .map(|(parent_guid, hardpoint)| {
            (
                (parent_guid.0, hardpoint.hardpoint_id.clone()),
                (hardpoint.offset_m.truncate(), hardpoint.local_rotation),
            )
        })
        .collect();
    let weapon_mount_by_guid: HashMap<uuid::Uuid, (uuid::Uuid, String)> = weapons
        .iter()
        .map(|(guid, mounted_on)| {
            (
                guid.0,
                (mounted_on.parent_entity_id, mounted_on.hardpoint_id.clone()),
            )
        })
        .collect();
    let shooter_entity_by_guid: HashMap<uuid::Uuid, Entity> = world_entity_guids
        .iter()
        .map(|(entity, guid)| (guid.0, entity))
        .collect();

    for event in events.read() {
        let message = &event.message;
        let Some(shooter_runtime_id) =
            sidereal_net::RuntimeEntityId::parse(message.shooter_entity_id.as_str())
        else {
            continue;
        };
        let predicted_muzzle = uuid::Uuid::parse_str(message.weapon_guid.as_str())
            .ok()
            .and_then(|weapon_guid| {
                local_predicted_muzzle_pose(
                    shooter_runtime_id.as_uuid(),
                    weapon_guid,
                    &local_controlled_by_guid,
                    &weapon_mount_by_guid,
                    &hardpoint_by_mount,
                )
            });

        let bolt_entity = pool.bolts[pool.next_index % pool.bolts.len()];
        pool.next_index = (pool.next_index + 1) % pool.bolts.len();
        if let Ok((mut transform, _material_handle, mut visibility, mut bolt)) =
            bolts.get_mut(bolt_entity)
        {
            let server_origin = Vec2::new(message.origin_xy[0] as f32, message.origin_xy[1] as f32);
            let server_velocity =
                Vec2::new(message.velocity_xy[0] as f32, message.velocity_xy[1] as f32);
            let (origin, velocity, excluded_entity, ttl_s) =
                if let Some((shooter_entity, predicted_origin, predicted_direction)) =
                    predicted_muzzle
                {
                    let speed = server_velocity.length().max(WEAPON_TRACER_SPEED_MPS);
                    let velocity = predicted_direction * speed;
                    let ttl_s = message
                        .impact_xy
                        .map(|impact_xy| {
                            let impact = Vec2::new(impact_xy[0] as f32, impact_xy[1] as f32);
                            ((impact - predicted_origin).length() / speed)
                                .clamp(WEAPON_TRACER_MIN_TTL_S, WEAPON_TRACER_LIFETIME_S)
                        })
                        .unwrap_or(message.ttl_s.max(WEAPON_TRACER_MIN_TTL_S));
                    (predicted_origin, velocity, Some(shooter_entity), ttl_s)
                } else {
                    (
                        server_origin,
                        server_velocity,
                        shooter_entity_by_guid
                            .get(&shooter_runtime_id.as_uuid())
                            .copied(),
                        message.ttl_s.max(WEAPON_TRACER_MIN_TTL_S),
                    )
                };
            transform.translation = Vec3::new(origin.x, origin.y, 0.35);
            if velocity.length_squared() > f32::EPSILON {
                transform.rotation =
                    Quat::from_rotation_z(velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD);
            }
            bolt.excluded_entity = excluded_entity;
            bolt.velocity = velocity;
            bolt.impact_xy = message
                .impact_xy
                .map(|impact_xy| Vec2::new(impact_xy[0] as f32, impact_xy[1] as f32));
            bolt.ttl_s = ttl_s;
            let speed = velocity.length();
            let normal = if speed > f32::EPSILON {
                let direction = velocity / speed;
                Vec2::new(-direction.y, direction.x)
            } else {
                Vec2::ZERO
            };
            bolt.lateral_normal = normal;
            bolt.wiggle_phase_rad = 0.0;
            bolt.wiggle_freq_hz = WEAPON_TRACER_WIGGLE_BASE_FREQ_HZ;
            bolt.wiggle_amp_mps = 0.0;
            *visibility = Visibility::Visible;
        }
    }
}

fn local_predicted_muzzle_pose(
    shooter_guid: uuid::Uuid,
    weapon_guid: uuid::Uuid,
    controlled_by_guid: &HashMap<uuid::Uuid, (Entity, DVec2, f64)>,
    weapon_mount_by_guid: &HashMap<uuid::Uuid, (uuid::Uuid, String)>,
    hardpoint_by_mount: &HashMap<(uuid::Uuid, String), (Vec2, Quat)>,
) -> Option<(Entity, Vec2, Vec2)> {
    let (shooter_entity, shooter_position, shooter_rotation_rad) =
        controlled_by_guid.get(&shooter_guid).copied()?;
    let (mounted_parent_guid, hardpoint_id) = weapon_mount_by_guid.get(&weapon_guid)?;
    if *mounted_parent_guid != shooter_guid {
        return None;
    }
    let (hardpoint_offset, hardpoint_rotation) =
        hardpoint_by_mount.get(&(shooter_guid, hardpoint_id.clone()))?;
    let shooter_quat = Quat::from_rotation_z(shooter_rotation_rad as f32);
    let muzzle_quat = shooter_quat * *hardpoint_rotation;
    let direction = (muzzle_quat * Vec3::Y).truncate();
    if direction.length_squared() <= f32::EPSILON {
        return None;
    }
    let origin = shooter_position.as_vec2() + rotate_vec2(shooter_quat, *hardpoint_offset);
    Some((shooter_entity, origin, direction.normalize()))
}

pub(super) fn receive_remote_destruction_effect_messages_system(
    mut pool: ResMut<'_, WeaponImpactExplosionPool>,
    mut events: MessageReader<'_, '_, RemoteEntityDestructionRuntimeMessage>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut explosions: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
) {
    if pool.explosions.is_empty() {
        return;
    }
    for event in events.read() {
        let message = &event.message;
        activate_destruction_effect(
            message.destruction_profile_id.as_str(),
            Vec2::new(message.origin_xy[0] as f32, message.origin_xy[1] as f32),
            &mut pool,
            &mut explosions,
            &mut effect_materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_weapon_tracer_visuals_system(
    time: Res<'_, Time>,
    spatial_query: SpatialQuery<'_, '_>,
    mut spark_pool: ResMut<'_, WeaponImpactSparkPool>,
    mut explosion_pool: ResMut<'_, WeaponImpactExplosionPool>,
    mut effect_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut bolts: Query<'_, '_, WeaponTracerBoltQueryItem<'_>, WeaponTracerBoltQueryFilter>,
    mut sparks: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactSpark,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactSparkQueryFilter,
    >,
    mut explosions: Query<
        '_,
        '_,
        (
            &'_ mut WeaponImpactExplosion,
            &'_ mut Transform,
            &'_ MeshMaterial2d<RuntimeEffectMaterial>,
            &'_ mut Visibility,
        ),
        WeaponImpactExplosionQueryFilter,
    >,
) {
    let dt_s = time.delta_secs();
    for (mut transform, material_handle, mut visibility, mut bolt) in &mut bolts {
        if bolt.ttl_s <= 0.0 {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        }
        bolt.ttl_s = (bolt.ttl_s - dt_s).max(0.0);
        bolt.wiggle_phase_rad += TAU * bolt.wiggle_freq_hz * dt_s;
        let lateral_speed_mps = bolt.wiggle_phase_rad.sin() * bolt.wiggle_amp_mps;
        let frame_velocity = bolt.velocity + bolt.lateral_normal * lateral_speed_mps;
        let frame_step = frame_velocity * dt_s;
        let frame_distance = frame_step.length();
        let current_pos = transform.translation.truncate();
        let mut hit_this_frame = false;
        if let Some(impact_pos) = bolt.impact_xy {
            let to_impact = impact_pos - current_pos;
            let impact_distance = to_impact.length();
            if impact_distance <= frame_distance.max(0.001) {
                transform.translation.x = impact_pos.x;
                transform.translation.y = impact_pos.y;
                transform.translation.z = 0.35;
                bolt.ttl_s = bolt.ttl_s.min(0.03);
                bolt.velocity = Vec2::ZERO;
                bolt.wiggle_amp_mps = 0.0;
                bolt.impact_xy = None;
                *visibility = Visibility::Visible;
                hit_this_frame = true;
                activate_weapon_impact_spark(
                    impact_pos,
                    &mut spark_pool,
                    &mut sparks,
                    &mut effect_materials,
                );
                activate_weapon_impact_explosion(
                    impact_pos,
                    &mut explosion_pool,
                    &mut explosions,
                    &mut effect_materials,
                );
            }
        }
        if hit_this_frame {
            continue;
        }
        if frame_distance > f32::EPSILON
            && let Ok(ray_direction) = Dir2::new(frame_step / frame_distance)
        {
            let filter = if let Some(excluded) = bolt.excluded_entity {
                SpatialQueryFilter::from_excluded_entities([excluded])
            } else {
                SpatialQueryFilter::default()
            };
            if let Some(hit) = spatial_query.cast_ray(
                current_pos.as_dvec2(),
                ray_direction,
                f64::from(frame_distance),
                true,
                &filter,
            ) {
                let impact_pos = current_pos + ray_direction.as_vec2() * hit.distance as f32;
                transform.translation.x = impact_pos.x;
                transform.translation.y = impact_pos.y;
                transform.translation.z = 0.35;
                bolt.ttl_s = bolt.ttl_s.min(0.03);
                bolt.velocity = Vec2::ZERO;
                bolt.wiggle_amp_mps = 0.0;
                bolt.impact_xy = None;
                *visibility = Visibility::Visible;
                hit_this_frame = true;
                activate_weapon_impact_spark(
                    impact_pos,
                    &mut spark_pool,
                    &mut sparks,
                    &mut effect_materials,
                );
                activate_weapon_impact_explosion(
                    impact_pos,
                    &mut explosion_pool,
                    &mut explosions,
                    &mut effect_materials,
                );
            }
        }
        if hit_this_frame {
            continue;
        }
        transform.translation.x += frame_step.x;
        transform.translation.y += frame_step.y;
        if frame_velocity.length_squared() > f32::EPSILON {
            transform.rotation = Quat::from_rotation_z(
                frame_velocity.to_angle() + WEAPON_TRACER_ROTATION_OFFSET_RAD,
            );
        }
        transform.translation.z = 0.35;
        let alpha = (bolt.ttl_s / WEAPON_TRACER_LIFETIME_S).clamp(0.0, 1.0);
        if let Some(material) = effect_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::beam_trail(
                1.0 - alpha,
                alpha * 0.95,
                0.65,
                0.35,
                (bolt.wiggle_amp_mps / WEAPON_TRACER_WIGGLE_MAX_AMP_MPS).clamp(0.0, 1.0) * 0.2,
                Vec4::new(1.0, 0.96, 0.7, 1.0),
                Vec4::new(1.0, 0.72, 0.22, 1.0),
            );
        }
        *visibility = if alpha > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn update_weapon_impact_sparks_system(
    time: Res<'_, Time>,
    mut spark_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut sparks: Query<'_, '_, WeaponImpactSparkQueryItem<'_>, Without<WeaponTracerBolt>>,
) {
    let dt_s = time.delta_secs();
    for (_entity, mut spark, mut transform, material_handle, mut visibility) in &mut sparks {
        spark.ttl_s = (spark.ttl_s - dt_s).max(0.0);
        if spark.ttl_s <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let t = (spark.ttl_s / spark.max_ttl_s).clamp(0.0, 1.0);
        let age_norm = 1.0 - t;
        if let Some(material) = spark_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::impact_spark(
                age_norm,
                1.0,
                1.0,
                t * 0.95,
                Vec4::new(1.0, 0.9, 0.55, 1.0),
            );
        }
        let scale = 1.0 + age_norm * 7.0;
        transform.scale = Vec3::splat(scale);
        *visibility = Visibility::Visible;
    }
}

pub(super) fn update_weapon_impact_explosions_system(
    time: Res<'_, Time>,
    mut explosion_materials: ResMut<'_, Assets<RuntimeEffectMaterial>>,
    mut explosions: Query<'_, '_, WeaponImpactExplosionQueryItem<'_>, Without<WeaponTracerBolt>>,
) {
    let dt_s = time.delta_secs();
    for (_entity, mut explosion, mut transform, material_handle, mut visibility) in &mut explosions
    {
        explosion.ttl_s = (explosion.ttl_s - dt_s).max(0.0);
        if explosion.ttl_s <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let t = (explosion.ttl_s / explosion.max_ttl_s).clamp(0.0, 1.0);
        let age_norm = 1.0 - t;
        transform.scale = Vec3::splat(explosion.base_scale + age_norm * explosion.growth_scale);
        if let Some(material) = explosion_materials.get_mut(&material_handle.0) {
            material.params = RuntimeEffectUniforms::explosion_burst(
                age_norm,
                explosion.intensity_scale + (1.0 - age_norm) * 0.35,
                1.0 + age_norm * 0.5,
                t * 0.95,
                0.35 + age_norm * 0.2,
                explosion.domain_scale,
                Vec4::new(1.0, 0.94, 0.72, 1.0),
                Vec4::new(1.0, 0.5, 0.15, 1.0),
                Vec4::new(0.24, 0.14, 0.08, 1.0),
            );
        }
        *visibility = Visibility::Visible;
    }
}

fn rotate_vec2(rotation: Quat, input: Vec2) -> Vec2 {
    (rotation * input.extend(0.0)).truncate()
}
