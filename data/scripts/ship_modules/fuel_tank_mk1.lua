return {
  module_id = "module.fuel.tank_mk1",
  display_name = "Fuel Tank",
  category = "fuel",
  entity_labels = { "Module", "FuelTank" },
  compatible_slot_kinds = { "fuel" },
  tags = { "fuel", "tank" },
  components = {
    {
      kind = "fuel_tank",
      properties = {
        fuel_kg = 1000.0,
      },
    },
    {
      kind = "mass_kg",
      properties = 1100.0,
    },
  },
}
