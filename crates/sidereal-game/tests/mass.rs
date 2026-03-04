use avian2d::prelude::{AngularInertia, Mass};
use bevy::prelude::*;
use sidereal_game::{
    CargoMassKg, EntityGuid, ModuleMassKg, SizeM, TotalMassKg, angular_inertia_from_size,
    recompute_total_mass,
};
use uuid::Uuid;

#[test]
fn recompute_total_mass_syncs_avian_mass_inertia_without_mass_dirty() {
    let mut app = App::new();
    app.add_systems(Update, recompute_total_mass);

    let total_mass = 15_000.0_f32;
    let size = SizeM {
        width: 25.0,
        length: 25.0,
        height: 8.0,
    };
    let expected_inertia = angular_inertia_from_size(total_mass, &size).0;

    let entity = app
        .world_mut()
        .spawn((
            EntityGuid(Uuid::new_v4()),
            CargoMassKg(0.0),
            ModuleMassKg(0.0),
            TotalMassKg(total_mass),
            size,
            Mass(1.0),
            AngularInertia(1.0),
        ))
        .id();

    app.update();

    let mass = app.world().entity(entity).get::<Mass>().unwrap();
    let inertia = app.world().entity(entity).get::<AngularInertia>().unwrap();
    assert_eq!(mass.0, total_mass);
    assert_eq!(inertia.0, expected_inertia);
}
