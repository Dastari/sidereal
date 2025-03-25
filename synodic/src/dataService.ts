import axios from "axios";
import { ApiResponse, Entity } from "./types";

export class DataService {
  private apiUrl: string;

  constructor(apiUrl: string) {
    this.apiUrl = apiUrl;
  }

  async fetchEntities(): Promise<Entity[]> {
    try {
      const response = await axios.post<ApiResponse>(
        this.apiUrl,
        {
          method: "bevy/query",
          jsonrpc: "2.0",
          id: 0,
          params: {
            data: {
              components: [
                "bevy_core::name::Name",
                "sidereal::ecs::components::id::Id",
                "sidereal::ecs::components::object::Object",
                "bevy_transform::components::transform::Transform",
                "avian2d::dynamics::rigid_body::LinearVelocity",
                "sidereal::ecs::components::sector::Sector",
              ],
            },
          },
        },
        {
          headers: {
            "Content-Type": "application/json",
          },
        }
      );

      return response.data.result;
    } catch (error) {
      console.error("Error fetching entities:", error);
      return [];
    }
  }
}
