use crate::generated::components::{MountedOn, ScannerComponent, ScannerRangeBuff};
use uuid::Uuid;

pub fn apply_range_buff(base_range_m: f32, buff: &ScannerRangeBuff) -> f32 {
    let multiplier = if buff.multiplier <= 0.0 {
        1.0
    } else {
        buff.multiplier
    };
    (base_range_m + buff.additive_m).max(0.0) * multiplier
}

pub fn compute_scanner_contribution(
    scanner: &ScannerComponent,
    buff: Option<&ScannerRangeBuff>,
) -> f32 {
    let level_multiplier = if scanner.level == 0 {
        1.0
    } else {
        scanner.level as f32
    };
    let base = scanner.base_range_m.max(0.0) * level_multiplier;
    if let Some(buff) = buff {
        apply_range_buff(base, buff)
    } else {
        base
    }
}

pub fn total_scanner_range_for_parent<'a>(
    parent_guid: Uuid,
    default_range_m: f32,
    own_scanner: Option<&ScannerComponent>,
    own_buff: Option<&ScannerRangeBuff>,
    mounted_scanners: impl Iterator<
        Item = (
            &'a MountedOn,
            &'a ScannerComponent,
            Option<&'a ScannerRangeBuff>,
        ),
    >,
) -> f32 {
    let mut total_range = default_range_m.max(0.0);
    if let Some(scanner) = own_scanner {
        total_range += compute_scanner_contribution(scanner, own_buff);
    } else if let Some(buff) = own_buff {
        total_range = apply_range_buff(total_range, buff);
    }
    for (mounted_on, scanner, buff) in mounted_scanners {
        if mounted_on.parent_entity_id == parent_guid {
            total_range += compute_scanner_contribution(scanner, buff);
        }
    }
    total_range.max(default_range_m)
}
