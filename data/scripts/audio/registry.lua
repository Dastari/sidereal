local AudioRegistry = {}

AudioRegistry.schema_version = 1

AudioRegistry.buses = {
  {
    bus_id = "music",
    parent = "master",
    default_volume_db = -4.0,
  },
  {
    bus_id = "sfx",
    parent = "master",
    default_volume_db = 0.0,
  },
  {
    bus_id = "dialog",
    parent = "master",
    default_volume_db = 0.0,
  },
  {
    bus_id = "ui",
    parent = "master",
    default_volume_db = -3.0,
  },
  {
    bus_id = "ambient",
    parent = "master",
    default_volume_db = -6.0,
  },
}

AudioRegistry.sends = {
  {
    send_id = "world_reverb",
    effects = {
      {
        kind = "reverb",
        mix = 0.18,
        damping = 0.42,
        room_size = 0.58,
      },
    },
  },
  {
    send_id = "radio_fx",
    effects = {
      {
        kind = "filter",
        mode = "band_pass",
        cutoff_hz = 2200.0,
        q = 0.85,
      },
      {
        kind = "distortion",
        drive = 0.05,
      },
    },
  },
}

AudioRegistry.environments = {
  {
    environment_id = "open_space",
    send_level_db = {
      world_reverb = -20.0,
    },
  },
  {
    environment_id = "station_interior",
    send_level_db = {
      world_reverb = -8.0,
    },
    bus_effect_overrides = {
      sfx = {
        {
          kind = "filter",
          mode = "low_pass",
          cutoff_hz = 14000.0,
          q = 0.71,
        },
      },
    },
  },
}

AudioRegistry.concurrency_groups = {
  {
    group_id = "weapon_loop_per_emitter",
    max_instances = 1,
    scope = "emitter_slot",
  },
}

AudioRegistry.profiles = {
  {
    profile_id = "weapon.ballistic_gatling",
    kind = "weapon",
    cues = {
      fire = {
        playback = {
          kind = "segmented_loop",
          clip_asset_id = "audio.sfx.weapon.ballistic_fire",
          intro_start_s = 0.0,
          loop_start_s = 1.0,
          loop_end_s = 2.0,
          outro_start_s = 2.0,
          clip_end_s = 4.0,
        },
        route = {
          bus = "sfx",
          sends = {
            {
              send_id = "world_reverb",
              level_db = -12.0,
            },
          },
        },
        spatial = {
          mode = "world_2d",
          min_distance_m = 5.0,
          max_distance_m = 220.0,
          rolloff = "logarithmic",
          pan_strength = 1.0,
          distance_lowpass = {
            enabled = true,
            near_hz = 18000.0,
            far_hz = 6000.0,
          },
        },
        concurrency = {
          group_id = "weapon_loop_per_emitter",
          steal = "restart",
        },
      },
    },
  },
  {
    profile_id = "destruction.asteroid.default",
    kind = "destruction",
    cues = {
      explode = {
        playback = {
          kind = "one_shot",
          clip_asset_id = "audio.sfx.explosion.asteroid.01",
        },
        route = {
          bus = "sfx",
          sends = {
            {
              send_id = "world_reverb",
              level_db = -10.0,
            },
          },
        },
        spatial = {
          mode = "world_2d",
          min_distance_m = 10.0,
          max_distance_m = 300.0,
          rolloff = "logarithmic",
        },
      },
    },
  },
  {
    profile_id = "music.menu.standard",
    kind = "music",
    cues = {
      main = {
        playback = {
          kind = "loop",
          clip_asset_id = "audio.music.menu_loop",
        },
        route = {
          bus = "music",
        },
        spatial = {
          mode = "screen_nonpositional",
        },
      },
    },
  },
  {
    profile_id = "music.world.deep_space",
    kind = "music",
    cues = {
      main = {
        playback = {
          kind = "loop",
          clip_asset_id = "audio.music.deep_space",
        },
        route = {
          bus = "music",
        },
        spatial = {
          mode = "screen_nonpositional",
        },
      },
    },
  },
}

return AudioRegistry
