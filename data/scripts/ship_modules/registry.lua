return {
  schema_version = 1,
  modules = {
    {
      module_id = "module.computer.flight_mk1",
      script = "ship_modules/flight_computer_mk1.lua",
      tags = { "computer", "control" },
    },
    {
      module_id = "module.engine.main_mk1",
      script = "ship_modules/engine_main_mk1.lua",
      tags = { "engine", "propulsion" },
    },
    {
      module_id = "module.fuel.tank_mk1",
      script = "ship_modules/fuel_tank_mk1.lua",
      tags = { "fuel", "tank" },
    },
    {
      module_id = "module.scanner.array_mk1",
      script = "ship_modules/scanner_array_mk1.lua",
      tags = { "scanner", "sensor" },
    },
    {
      module_id = "module.weapon.ballistic_gatling_mk1",
      script = "ship_modules/ballistic_gatling_mk1.lua",
      tags = { "weapon", "ballistic" },
    },
  },
}
