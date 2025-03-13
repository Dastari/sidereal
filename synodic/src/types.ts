// Sector coordinate type (matches server-side implementation)
export interface SectorCoord {
  x: number;
  y: number;
}

// Entity component types
export interface Name {
  hash: number;
  name: string;
}

export interface Id {
  [key: string]: string;
}

export interface Transform {
  rotation: [number, number, number, number];
  scale: [number, number, number];
  translation: [number, number, number];
}

export interface Object {
  [key: string]: string;
}

// Entity structure from API
export interface Entity {
  components: {
    "bevy_core::name::Name": Name;
    "sidereal_core::ecs::components::id::Id": string;
    "bevy_transform::components::transform::Transform": Transform;
    "sidereal_core::ecs::components::object::Object": string;
    "avian2d::dynamics::rigid_body::LinearVelocity": [number, number];
    "sidereal_core::ecs::components::in_sector::InSector": SectorCoord;
  };
  entity: number;
}

// API response structure
export interface ApiResponse {
  jsonrpc: string;
  id: number;
  result: Entity[];
}
