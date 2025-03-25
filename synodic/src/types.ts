import { string } from "three/tsl";

// Sector coordinate type (matches server-side implementation)
export interface SectorCoord {
  x: number;
  y: number;
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
    "bevy_core::name::Name": string;
    "sidereal::ecs::components::id::Id": string;
    "bevy_transform::components::transform::Transform": Transform;
    "sidereal::ecs::components::object::Object": string;
    "avian2d::dynamics::rigid_body::LinearVelocity": [number, number];
    "sidereal::ecs::components::sector::Sector": SectorCoord;
  };
  entity: number;
}

// API response structure
export interface ApiResponse {
  jsonrpc: string;
  id: number;
  result: Entity[];
}
