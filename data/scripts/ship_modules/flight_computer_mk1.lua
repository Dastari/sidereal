return {
  module_id = "module.computer.flight_mk1",
  display_name = "Flight Computer MK1",
  category = "computer",
  entity_labels = { "Module", "Computer", "FlightComputer" },
  compatible_slot_kinds = { "computer" },
  tags = { "computer", "control" },
  components = {
    {
      kind = "mass_kg",
      properties = 50.0,
    },
  },
}
