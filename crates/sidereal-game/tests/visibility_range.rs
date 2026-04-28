use sidereal_game::{MountedOn, VisibilityRangeBuffM, total_visibility_range_for_parent};
use uuid::Uuid;

#[test]
fn root_visibility_range_buff_provides_intrinsic_range_without_modules() {
    let root_guid = Uuid::new_v4();
    let root_buff = VisibilityRangeBuffM {
        additive_m: 300.0,
        multiplier: 1.0,
    };

    let total = total_visibility_range_for_parent(root_guid, Some(&root_buff), std::iter::empty());

    assert_eq!(total, 300.0);
}

#[test]
fn mounted_visibility_range_buffs_add_to_root_intrinsic_range() {
    let root_guid = Uuid::new_v4();
    let other_guid = Uuid::new_v4();
    let root_buff = VisibilityRangeBuffM {
        additive_m: 300.0,
        multiplier: 1.0,
    };
    let scanner_buff = VisibilityRangeBuffM {
        additive_m: 1_000.0,
        multiplier: 1.0,
    };
    let unrelated_buff = VisibilityRangeBuffM {
        additive_m: 5_000.0,
        multiplier: 1.0,
    };
    let scanner_mount = MountedOn {
        parent_entity_id: root_guid,
        hardpoint_id: "scanner_dorsal".to_string(),
    };
    let unrelated_mount = MountedOn {
        parent_entity_id: other_guid,
        hardpoint_id: "scanner_dorsal".to_string(),
    };
    let mounted = [
        (&scanner_mount, &scanner_buff),
        (&unrelated_mount, &unrelated_buff),
    ];

    let total = total_visibility_range_for_parent(root_guid, Some(&root_buff), mounted.into_iter());

    assert_eq!(total, 1_300.0);
}
