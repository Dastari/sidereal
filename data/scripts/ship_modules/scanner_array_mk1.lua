return {
  module_id = "module.scanner.array_mk1",
  display_name = "Scanner Array MK1",
  category = "scanner",
  entity_labels = { "Module", "Scanner" },
  compatible_slot_kinds = { "scanner" },
  tags = { "scanner", "sensor" },
  components = {
    {
      kind = "scanner_component",
      properties = {
        base_range_m = 1000.0,
        level = 1,
        detail_tier = "Iff",
        supports_density = true,
        supports_directional_awareness = true,
        max_contacts = 64,
      },
    },
    {
      kind = "visibility_range_buff_m",
      properties = {
        additive_m = 1000.0,
        multiplier = 1.0,
      },
    },
    {
      kind = "mass_kg",
      properties = 80.0,
    },
  },
}
