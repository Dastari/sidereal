
use bevy::prelude::*;
use std::collections::HashSet;
use sidereal::sector::Sector;

#[derive(Resource, Default, Debug)]
pub struct AssignedSectors {
    pub sectors: HashSet<Sector>,
    pub dirty: bool,
}