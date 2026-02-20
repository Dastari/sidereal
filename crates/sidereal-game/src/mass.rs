use avian3d::prelude::Mass;
use bevy::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::generated::components::{
    BaseMassKg, CargoMassKg, EntityGuid, Inventory, MassDirty, MassKg, ModuleMassKg, MountedOn,
    TotalMassKg,
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
    mut roots: Query<
        (
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
        ),
        Without<MountedOn>,
    >,
    modules: Query<(&EntityGuid, &MountedOn, Option<&MassKg>, Option<&Inventory>)>,
    inventories: Query<(Entity, Option<&Inventory>)>,
    child_of: Query<(Entity, &ChildOf)>,
) {
    let inventory_mass_by_entity = inventories
        .iter()
        .map(|(entity, inventory)| (entity, inventory_mass_kg(inventory)))
        .collect::<HashMap<_, _>>();
    let mut children_by_parent_entity = HashMap::<Entity, Vec<Entity>>::new();
    for (entity, parent) in &child_of {
        children_by_parent_entity
            .entry(parent.parent())
            .or_default()
            .push(entity);
    }

    let mut module_mass_by_guid = HashMap::<Uuid, f32>::new();
    let mut module_children_by_parent_guid = HashMap::<Uuid, Vec<Uuid>>::new();
    for (module_guid, mounted_on, module_mass, module_inventory) in &modules {
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
    ) in &mut roots
    {
        if mass_dirty.is_none() && total_mass.0 > 0.0 {
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
    }
}
