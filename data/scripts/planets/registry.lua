local PlanetRegistry = {}

PlanetRegistry.schema_version = 1

PlanetRegistry.planets = {
  {
    planet_id = "planet.helion",
    script = "planets/helion.lua",
    spawn_enabled = true,
    tags = { "starter", "star" },
  },
  {
    planet_id = "planet.aurelia",
    script = "planets/aurelia.lua",
    spawn_enabled = true,
    tags = { "starter", "terran" },
  },
}

return PlanetRegistry
