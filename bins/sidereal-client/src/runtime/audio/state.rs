use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Debug, Resource, Default)]
pub(crate) struct AudioAssetDemandState {
    pub desired_asset_ids: HashSet<String>,
    pub critical_asset_ids: HashSet<String>,
}
