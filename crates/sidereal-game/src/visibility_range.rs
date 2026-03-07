use crate::generated::components::{MountedOn, VisibilityRangeBuffM};
use uuid::Uuid;

pub fn apply_visibility_range_buff(base_range_m: f32, buff: &VisibilityRangeBuffM) -> f32 {
    let multiplier = if buff.multiplier <= 0.0 {
        1.0
    } else {
        buff.multiplier
    };
    (base_range_m + buff.additive_m).max(0.0) * multiplier
}

pub fn total_visibility_range_for_parent<'a>(
    parent_guid: Uuid,
    own_buff: Option<&VisibilityRangeBuffM>,
    mounted_buffs: impl Iterator<Item = (&'a MountedOn, &'a VisibilityRangeBuffM)>,
) -> f32 {
    let mut total_range = own_buff
        .map(|buff| apply_visibility_range_buff(0.0, buff))
        .unwrap_or(0.0);
    for (mounted_on, buff) in mounted_buffs {
        if mounted_on.parent_entity_id == parent_guid {
            total_range += apply_visibility_range_buff(0.0, buff);
        }
    }
    total_range.max(0.0)
}
