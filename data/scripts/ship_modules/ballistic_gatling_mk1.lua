return {
  module_id = "module.weapon.ballistic_gatling_mk1",
  display_name = "Ballistic Gatling",
  category = "weapon",
  entity_labels = { "Module", "Weapon", "BallisticWeapon" },
  compatible_slot_kinds = { "weapon" },
  tags = { "weapon", "ballistic" },
  components = {
    {
      kind = "weapon_tag",
      properties = {},
    },
    {
      kind = "ballistic_weapon",
      properties = {
        weapon_name = "Ballistic Gatling",
        fire_audio_profile_id = "weapon.ballistic_gatling",
        rpm = 750.0,
        damage_per_shot = 22.0,
        max_range_m = 2200.0,
        projectile_speed_mps = 0.0,
        spread_rad = 0.0,
        damage_type = "Ballistic",
      },
    },
    {
      kind = "ammo_count",
      properties = {
        current = 500,
        capacity = 500,
      },
    },
    {
      kind = "mass_kg",
      properties = 120.0,
    },
  },
}
