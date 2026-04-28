return {
  schema_version = 1,
  ships = {
    {
      ship_id = "ship.corvette",
      bundle_id = "ship.corvette",
      script = "ships/corvette.lua",
      spawn_enabled = true,
      tags = { "starter", "combat", "small" },
    },
    {
      ship_id = "ship.rocinante",
      bundle_id = "ship.rocinante",
      script = "ships/rocinante.lua",
      spawn_enabled = true,
      tags = { "heavy", "combat", "prototype" },
    },
  },
}
