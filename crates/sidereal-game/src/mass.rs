use avian2d::prelude::{AngularInertia, Collider, Mass, RigidBody};
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::flight::angular_inertia_from_size;
use crate::generated::components::{
    BaseMassKg, CargoMassKg, CollisionAabbM, CollisionOutlineM, CollisionProfile, EntityGuid,
    Inventory, MassDirty, MassKg, ModuleMassKg, MountedOn, SizeM, TotalMassKg,
};

fn inventory_mass_kg(inventory: Option<&Inventory>) -> f32 {
    inventory
        .map(|inv| {
            inv.entries
                .iter()
                .map(|entry| entry.unit_mass_kg.max(0.0) * entry.quantity as f32)
                .sum::<f32>()
        })
        .unwrap_or(0.0)
}

fn module_tree_mass(
    root_guid: Uuid,
    module_mass_by_guid: &HashMap<Uuid, f32>,
    children_by_parent: &HashMap<Uuid, Vec<Uuid>>,
) -> f32 {
    let mut total = 0.0;
    let mut stack = children_by_parent
        .get(&root_guid)
        .cloned()
        .unwrap_or_default();
    while let Some(guid) = stack.pop() {
        total += module_mass_by_guid.get(&guid).copied().unwrap_or(0.0);
        if let Some(children) = children_by_parent.get(&guid) {
            stack.extend(children.iter().copied());
        }
    }
    total
}

fn child_inventory_tree_mass(
    root_entity: Entity,
    inventory_mass_by_entity: &HashMap<Entity, f32>,
    children_by_parent_entity: &HashMap<Entity, Vec<Entity>>,
) -> f32 {
    let mut total = 0.0;
    let mut stack = children_by_parent_entity
        .get(&root_entity)
        .cloned()
        .unwrap_or_default();
    while let Some(entity) = stack.pop() {
        total += inventory_mass_by_entity
            .get(&entity)
            .copied()
            .unwrap_or(0.0);
        if let Some(children) = children_by_parent_entity.get(&entity) {
            stack.extend(children.iter().copied());
        }
    }
    total
}

#[allow(clippy::type_complexity)]
pub fn recompute_total_mass(
    mut roots: ParamSet<(
        Query<(
            Entity,
            &EntityGuid,
            Option<&MassKg>,
            Option<&BaseMassKg>,
            Option<&Inventory>,
            &mut CargoMassKg,
            &mut ModuleMassKg,
            &mut TotalMassKg,
            Option<&MassDirty>,
            Option<&mut Mass>,
            Option<&SizeM>,
            Option<&mut AngularInertia>,
        )>,
        Query<(&TotalMassKg, Option<&MassDirty>)>,
    )>,
    modules: Query<(
        Entity,
        &EntityGuid,
        &MountedOn,
        Option<&MassKg>,
        Option<&Inventory>,
    )>,
    inventories: Query<(Entity, Option<&Inventory>)>,
) {
    let needs_recompute = roots
        .p1()
        .iter()
        .any(|(total_mass, mass_dirty)| mass_dirty.is_some() || total_mass.0 <= 0.0);
    if !needs_recompute {
        // Even on clean ticks, keep Avian mass/inertia aligned with persisted gameplay mass.
        // This preserves server/client parity when loading older persisted values.
        for (
            _entity,
            _guid,
            _mass,
            _base_mass,
            _inventory,
            _cargo_mass,
            _module_mass,
            total_mass,
            _mass_dirty,
            maybe_avian_mass,
            maybe_size,
            maybe_avian_inertia,
        ) in &mut roots.p0()
        {
            if total_mass.0 <= 0.0 {
                continue;
            }
            if let Some(mut avian_mass) = maybe_avian_mass
                && avian_mass.0 != total_mass.0
            {
                *avian_mass = Mass(total_mass.0);
            }
            if let (Some(mut avian_inertia), Some(size)) = (maybe_avian_inertia, maybe_size) {
                let expected_inertia = angular_inertia_from_size(total_mass.0, size);
                if avian_inertia.0 != expected_inertia.0 {
                    *avian_inertia = expected_inertia;
                }
            }
        }
        return;
    }

    let inventory_mass_by_entity = inventories
        .iter()
        .map(|(entity, inventory)| (entity, inventory_mass_kg(inventory)))
        .collect::<HashMap<_, _>>();

    // Build parent entity -> child entities from roots and MountedOn (uses the UUID-based
    // relationship directly rather than traversing Bevy ChildOf hierarchy).
    let root_guid_to_entity: HashMap<Uuid, Entity> = roots
        .p0()
        .iter()
        .map(|(entity, guid, ..)| (guid.0, entity))
        .collect();
    let mut children_by_parent_entity = HashMap::<Entity, Vec<Entity>>::new();
    for (child_entity, _guid, mounted_on, ..) in &modules {
        if let Some(&parent_entity) = root_guid_to_entity.get(&mounted_on.parent_entity_id) {
            children_by_parent_entity
                .entry(parent_entity)
                .or_default()
                .push(child_entity);
        }
    }

    let mut module_mass_by_guid = HashMap::<Uuid, f32>::new();
    let mut module_children_by_parent_guid = HashMap::<Uuid, Vec<Uuid>>::new();
    for (_entity, module_guid, mounted_on, module_mass, module_inventory) in &modules {
        let module_total =
            module_mass.map(|m| m.0).unwrap_or(0.0) + inventory_mass_kg(module_inventory);
        module_mass_by_guid.insert(module_guid.0, module_total);
        module_children_by_parent_guid
            .entry(mounted_on.parent_entity_id)
            .or_default()
            .push(module_guid.0);
    }

    for (
        entity,
        guid,
        mass,
        base_mass,
        inventory,
        mut cargo_mass,
        mut module_mass,
        mut total_mass,
        mass_dirty,
        maybe_avian_mass,
        maybe_size,
        maybe_avian_inertia,
    ) in &mut roots.p0()
    {
        if mass_dirty.is_none() && total_mass.0 > 0.0 {
            // Keep Avian mass/inertia synchronized even when aggregate totals are not dirty.
            // Persisted worlds can carry stale Avian inertia values from older formulas.
            if let Some(mut avian_mass) = maybe_avian_mass
                && avian_mass.0 != total_mass.0
            {
                *avian_mass = Mass(total_mass.0);
            }
            if let (Some(mut avian_inertia), Some(size)) = (maybe_avian_inertia, maybe_size) {
                let expected_inertia = angular_inertia_from_size(total_mass.0, size);
                if avian_inertia.0 != expected_inertia.0 {
                    *avian_inertia = expected_inertia;
                }
            }
            continue;
        }

        let base = base_mass
            .map(|m| m.0)
            .or_else(|| mass.map(|m| m.0))
            .unwrap_or(0.0);
        let own_inventory = inventory_mass_kg(inventory);
        let child_inventory = child_inventory_tree_mass(
            entity,
            &inventory_mass_by_entity,
            &children_by_parent_entity,
        );
        let cargo_total = own_inventory + child_inventory;
        let module_total = module_tree_mass(
            guid.0,
            &module_mass_by_guid,
            &module_children_by_parent_guid,
        );
        let computed_total = (base + cargo_total + module_total).max(1.0);

        cargo_mass.0 = cargo_total;
        module_mass.0 = module_total;
        total_mass.0 = computed_total;
        if let Some(mut avian_mass) = maybe_avian_mass {
            *avian_mass = Mass(computed_total);
        }
        if let (Some(mut avian_inertia), Some(size)) = (maybe_avian_inertia, maybe_size) {
            *avian_inertia = angular_inertia_from_size(computed_total, size);
        }
    }
}

/// Ensures ship entities have the derived mass-tracking components that
/// `recompute_total_mass` requires. Handles entities hydrated from older
/// graph records that predate these components.
#[allow(clippy::type_complexity)]
pub fn bootstrap_root_dynamic_mass_components(
    mut commands: Commands<'_, '_>,
    roots: Query<
        '_,
        '_,
        (
            Entity,
            Option<&'_ MassKg>,
            Has<BaseMassKg>,
            Has<CargoMassKg>,
            Has<ModuleMassKg>,
            Has<TotalMassKg>,
        ),
        (With<RigidBody>, Without<MountedOn>),
    >,
) {
    for (entity, mass_kg, has_base, has_cargo, has_module, has_total) in &roots {
        if has_base && has_cargo && has_module && has_total {
            continue;
        }
        let hull = mass_kg.map(|m| m.0).unwrap_or(1.0);
        let mut cmds = commands.entity(entity);
        if !has_base {
            cmds.insert(BaseMassKg(hull));
        }
        if !has_cargo {
            cmds.insert(CargoMassKg(0.0));
        }
        if !has_module {
            cmds.insert(ModuleMassKg(0.0));
        }
        if !has_total {
            cmds.insert((TotalMassKg(hull), MassDirty));
        }
    }
}

/// Ensures root collidable entities with `SizeM` have an Avian collider.
/// This covers hydrated entities that may carry `RigidBody` but miss `Collider`.
#[allow(clippy::type_complexity)]
pub fn bootstrap_collision_profiles_from_aabb(
    mut commands: Commands<'_, '_>,
    entities: Query<
        '_,
        '_,
        (Entity, Option<&'_ MountedOn>),
        (With<CollisionAabbM>, Without<CollisionProfile>),
    >,
) {
    for (entity, mounted_on) in &entities {
        if mounted_on.is_some() {
            continue;
        }
        commands
            .entity(entity)
            .insert(CollisionProfile::solid_aabb());
    }
}

pub fn collider_from_collision_shape(
    size: &SizeM,
    collision_aabb: Option<&CollisionAabbM>,
    collision_outline: Option<&CollisionOutlineM>,
) -> Collider {
    if let Some(outline) = collision_outline
        && outline.points.len() >= 3
    {
        let len = outline.points.len();
        let indices = (0..len)
            .map(|idx| [idx as u32, ((idx + 1) % len) as u32])
            .collect::<Vec<_>>();
        return Collider::convex_decomposition(outline.points.clone(), indices);
    }

    let (width, length) = if let Some(aabb) = collision_aabb {
        (
            (aabb.half_extents.x * 2.0).max(0.1),
            (aabb.half_extents.y * 2.0).max(0.1),
        )
    } else {
        (size.width.max(0.1), size.length.max(0.1))
    };
    Collider::rectangle(width, length)
}

/// Ensures root collidable entities with `SizeM` have an Avian collider.
/// This covers hydrated entities that may carry `RigidBody` but miss `Collider`.
#[allow(clippy::type_complexity)]
pub fn bootstrap_root_dynamic_entity_colliders(
    mut commands: Commands<'_, '_>,
    entities: Query<
        '_,
        '_,
        (
            Entity,
            &'_ SizeM,
            Option<&'_ CollisionAabbM>,
            Option<&'_ CollisionOutlineM>,
            Option<&'_ CollisionProfile>,
            Option<&'_ MountedOn>,
            Option<&'_ RigidBody>,
            Has<Collider>,
        ),
    >,
) {
    for (
        entity,
        size,
        collision_aabb,
        collision_outline,
        collision_profile,
        mounted_on,
        rigid_body,
        has_collider,
    ) in &entities
    {
        if mounted_on.is_some() || has_collider {
            continue;
        }
        let is_collidable = collision_profile
            .copied()
            .is_some_and(CollisionProfile::is_collidable);
        if !is_collidable {
            continue;
        }
        let Some(rigid_body) = rigid_body else {
            continue;
        };
        if !matches!(
            rigid_body,
            RigidBody::Dynamic | RigidBody::Kinematic | RigidBody::Static
        ) {
            continue;
        }
        commands
            .entity(entity)
            .insert(collider_from_collision_shape(
                size,
                collision_aabb,
                collision_outline,
            ));
    }
}
