use bevy::prelude::App;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum VisibilityScope {
    OwnerOnly,
    Faction,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiderealComponentMeta {
    pub kind: &'static str,
    pub persist: bool,
    pub replicate: bool,
    pub predict: bool,
    pub visibility: &'static [VisibilityScope],
}

pub trait SiderealComponentMetadata {
    const META: SiderealComponentMeta;
}

#[derive(Clone, Copy)]
pub struct SiderealComponentRegistration {
    pub register_reflect: fn(&mut App),
    pub register_lightyear_client: fn(&mut App),
    pub register_lightyear_server: fn(&mut App),
    pub type_path: fn() -> &'static str,
    pub meta: SiderealComponentMeta,
}

inventory::collect!(SiderealComponentRegistration);
