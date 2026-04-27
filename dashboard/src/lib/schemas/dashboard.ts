import { z } from 'zod'
import { decodeSoundId } from '@/features/audio-studio/types'

export const uuidSchema = z.string().uuid()

const uuidLikeSchema = z
  .string()
  .trim()
  .regex(
    /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/,
    'entityId must use UUID text form',
  )

export const databaseAccountsSearchSchema = z.object({
  search: z.string().catch(''),
  sort: z.enum(['email', 'characters', 'mfa', 'created']).catch('email'),
})

export const databaseTablesSearchSchema = z.object({
  search: z.string().catch(''),
  sort: z.enum(['name', 'rows', 'schema']).catch('schema'),
})

export const dashboardPasswordLoginSchema = z.object({
  email: z.string().trim().email('email must be valid'),
  password: z.string().trim().min(1, 'password is required'),
})

export const dashboardRegisterSchema = z.object({
  mode: z.literal('register'),
  email: z.string().trim().email('email must be valid'),
  password: z
    .string()
    .trim()
    .min(12, 'password must be at least 12 characters'),
})

export const dashboardSetupAdminSchema = z.object({
  email: z.string().trim().email('email must be valid'),
  password: z
    .string()
    .trim()
    .min(12, 'password must be at least 12 characters'),
  setupToken: z.string().trim().min(1, 'setup token is required'),
})

export const accountCharacterCreateSchema = z.object({
  displayName: z
    .string()
    .trim()
    .min(2, 'displayName must be between 2 and 64 characters')
    .max(64, 'displayName must be between 2 and 64 characters')
    .regex(
      /^[A-Za-z0-9 _-]+$/,
      'displayName may contain letters, numbers, spaces, hyphens, and underscores',
    ),
})

export const dashboardMfaLoginSchema = z.object({
  challenge_id: uuidSchema,
  code: z
    .string()
    .trim()
    .regex(/^\d{6}$/, 'code must be 6 digits'),
})

export const dashboardTotpEnrollmentVerifySchema = z.object({
  enrollmentId: uuidSchema,
  code: z
    .string()
    .trim()
    .regex(/^\d{6}$/, 'code must be 6 digits'),
})

export const publicPasswordResetRequestSchema = z.object({
  email: z.string().trim().email('email must be valid'),
})

export const publicPasswordResetConfirmSchema = z.object({
  resetToken: z.string().trim().min(1, 'reset token is required'),
  newPassword: z
    .string()
    .trim()
    .min(12, 'password must be at least 12 characters'),
})

export const passwordResetParamsSchema = z.object({
  accountId: uuidSchema,
})

export const renameCharacterParamsSchema = z.object({
  playerEntityId: uuidSchema,
})

export const renameCharacterBodySchema = z.object({
  displayName: z
    .string()
    .trim()
    .min(2, 'displayName must be between 2 and 64 characters')
    .max(64, 'displayName must be between 2 and 64 characters'),
})

export const audioStudioSoundIdSchema = z
  .string()
  .trim()
  .min(1, 'soundId is required')
  .refine((value) => decodeSoundId(value) !== null, 'Invalid sound id')

export const audioStudioParamsSchema = z.object({
  soundId: audioStudioSoundIdSchema,
})

const audioStudioMarkerValueSchema = z
  .number()
  .finite('marker value must be a finite number')
  .min(0, 'marker values must be greater than or equal to 0')
  .nullable()

export const audioStudioMarkerBodySchema = z
  .object({
    intro_start_s: audioStudioMarkerValueSchema,
    loop_start_s: audioStudioMarkerValueSchema,
    loop_end_s: audioStudioMarkerValueSchema,
    outro_start_s: audioStudioMarkerValueSchema,
    clip_end_s: audioStudioMarkerValueSchema,
  })
  .superRefine((value, ctx) => {
    const orderedPairs = [
      ['intro_start_s', 'loop_start_s'],
      ['loop_start_s', 'loop_end_s'],
      ['loop_end_s', 'outro_start_s'],
      ['outro_start_s', 'clip_end_s'],
    ] as const

    for (const [leftKey, rightKey] of orderedPairs) {
      const left = value[leftKey]
      const right = value[rightKey]
      if (left === null || right === null) {
        continue
      }
      const isStrict = leftKey === 'loop_start_s'
      if ((isStrict && left >= right) || (!isStrict && left > right)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          message: `${leftKey} must be ${isStrict ? 'less than' : 'less than or equal to'} ${rightKey}`,
          path: [rightKey],
        })
      }
    }
  })

export const spawnEntityBodySchema = z.object({
  player_entity_id: uuidSchema,
  bundle_id: z.string().trim().min(1, 'bundle_id is required'),
  overrides: z.record(z.string(), z.unknown()).optional().default({}),
})

export const graphComponentUpdateSchema = z.object({
  entityId: z.string().min(1, 'entityId is required'),
  typePath: z.string().min(1, 'typePath is required'),
  componentKind: z.string().min(1, 'componentKind is required'),
  value: z.unknown().optional(),
})

export const brpTargetSchema = z.enum(['server', 'client', 'hostClient'])

export const brpPortSchema = z
  .union([z.number(), z.string()])
  .transform((value) =>
    typeof value === 'number' ? value : Number.parseInt(value, 10),
  )
  .refine(
    (value) => Number.isInteger(value) && value >= 1 && value <= 65535,
    'port must be an integer between 1 and 65535',
  )

export const brpHostSchema = z
  .string()
  .trim()
  .min(1, 'host is required')
  .max(253, 'host must be 253 characters or fewer')
  .regex(
    /^[A-Za-z0-9.-]+$/,
    'host must be an IP address or hostname without a protocol',
  )

export const brpRequestSchema = z.object({
  id: z.unknown().optional(),
  method: z
    .string()
    .trim()
    .min(1, 'Body must include a JSON-RPC method string'),
  params: z.unknown().optional(),
  target: brpTargetSchema.optional(),
  port: brpPortSchema.optional(),
  host: brpHostSchema.optional(),
})

const genesisFiniteNumberSchema = z
  .number()
  .finite('numeric Genesis fields must be finite numbers')

const genesisUnitNumberSchema = genesisFiniteNumberSchema.min(0).max(1)

const genesisVec2Schema = z.tuple([
  genesisFiniteNumberSchema,
  genesisFiniteNumberSchema,
])

const genesisVec3Schema = z.tuple([
  genesisFiniteNumberSchema,
  genesisFiniteNumberSchema,
  genesisFiniteNumberSchema,
])

export const genesisPlanetIdSchema = z
  .string()
  .trim()
  .min(1, 'planetId is required')
  .regex(/^planet\.[a-z0-9_.-]+$/, 'planetId must use the planet.<slug> form')

export const genesisPlanetShaderSettingsSchema = z.object({
  enabled: z.boolean(),
  enable_surface_detail: z.boolean(),
  enable_craters: z.boolean(),
  enable_clouds: z.boolean(),
  enable_atmosphere: z.boolean(),
  enable_specular: z.boolean(),
  enable_night_lights: z.boolean(),
  enable_emissive: z.boolean(),
  enable_ocean_specular: z.boolean(),
  body_kind: z.number().int().min(0).max(2),
  planet_type: z.number().int().min(0).max(5),
  seed: z.number().int().min(0).max(999999999),
  base_radius_scale: genesisFiniteNumberSchema.min(0.1).max(2),
  normal_strength: genesisFiniteNumberSchema.min(0).max(4),
  detail_level: genesisUnitNumberSchema,
  rotation_speed: genesisFiniteNumberSchema.min(-4).max(4),
  light_wrap: genesisUnitNumberSchema,
  ambient_strength: genesisFiniteNumberSchema.min(0).max(4),
  specular_strength: genesisFiniteNumberSchema.min(0).max(8),
  specular_power: genesisFiniteNumberSchema.min(1).max(128),
  rim_strength: genesisFiniteNumberSchema.min(0).max(8),
  rim_power: genesisFiniteNumberSchema.min(0.1).max(16),
  fresnel_strength: genesisFiniteNumberSchema.min(0).max(8),
  cloud_shadow_strength: genesisUnitNumberSchema,
  night_glow_strength: genesisFiniteNumberSchema.min(0).max(4),
  continent_size: genesisUnitNumberSchema,
  ocean_level: genesisUnitNumberSchema,
  mountain_height: genesisUnitNumberSchema,
  roughness: genesisUnitNumberSchema,
  terrain_octaves: z.number().int().min(1).max(12),
  terrain_lacunarity: genesisFiniteNumberSchema.min(1).max(5),
  terrain_gain: genesisFiniteNumberSchema.min(0).max(1),
  crater_density: genesisUnitNumberSchema,
  crater_size: genesisUnitNumberSchema,
  volcano_density: genesisUnitNumberSchema,
  ice_cap_size: genesisUnitNumberSchema,
  storm_intensity: genesisUnitNumberSchema,
  bands_count: genesisFiniteNumberSchema.min(0).max(32),
  spot_density: genesisUnitNumberSchema,
  surface_activity: genesisUnitNumberSchema,
  corona_intensity: genesisFiniteNumberSchema.min(0).max(4),
  cloud_coverage: genesisUnitNumberSchema,
  cloud_scale: genesisFiniteNumberSchema.min(0.1).max(10),
  cloud_speed: genesisFiniteNumberSchema.min(-4).max(4),
  cloud_alpha: genesisUnitNumberSchema,
  atmosphere_thickness: genesisFiniteNumberSchema.min(0).max(2),
  atmosphere_falloff: genesisFiniteNumberSchema.min(0.1).max(16),
  atmosphere_alpha: genesisUnitNumberSchema,
  city_lights: genesisUnitNumberSchema,
  emissive_strength: genesisFiniteNumberSchema.min(0).max(8),
  sun_intensity: genesisFiniteNumberSchema.min(0).max(8),
  surface_saturation: genesisFiniteNumberSchema.min(0).max(4),
  surface_contrast: genesisFiniteNumberSchema.min(0).max(4),
  light_color_mix: genesisUnitNumberSchema,
  sun_direction_xy: genesisVec2Schema,
  color_primary_rgb: genesisVec3Schema,
  color_secondary_rgb: genesisVec3Schema,
  color_tertiary_rgb: genesisVec3Schema,
  color_atmosphere_rgb: genesisVec3Schema,
  color_clouds_rgb: genesisVec3Schema,
  color_night_lights_rgb: genesisVec3Schema,
  color_emissive_rgb: genesisVec3Schema,
})

export const genesisPlanetDefinitionSchema = z.object({
  planet_id: genesisPlanetIdSchema,
  script_path: z
    .string()
    .trim()
    .regex(
      /^planets\/[A-Za-z0-9_.-]+\.lua$/,
      'scriptPath must stay under planets/ and end in .lua',
    ),
  display_name: z
    .string()
    .trim()
    .min(1, 'displayName is required')
    .max(96, 'displayName is too long'),
  entity_labels: z.array(z.string().trim().min(1)).max(16),
  tags: z.array(z.string().trim().min(1).max(32)).max(16),
  spawn: z.object({
    entity_id: uuidLikeSchema,
    owner_id: z.string().trim().min(1, 'ownerId is required').max(96),
    size_m: genesisFiniteNumberSchema.min(1).max(1_000_000),
    spawn_position: genesisVec2Schema,
    spawn_rotation_rad: genesisFiniteNumberSchema,
    map_icon_asset_id: z.string().trim().min(1).max(128),
    planet_visual_shader_asset_id: z.string().trim().min(1).max(128),
  }),
  shader_settings: genesisPlanetShaderSettingsSchema,
})

export const genesisPlanetDraftBodySchema = z.object({
  definition: genesisPlanetDefinitionSchema,
  spawnEnabled: z.boolean(),
})

export const genesisPlanetParamsSchema = z.object({
  planetId: genesisPlanetIdSchema,
})

export type DatabaseAccountsSearch = z.infer<
  typeof databaseAccountsSearchSchema
>
export type DatabaseTablesSearch = z.infer<typeof databaseTablesSearchSchema>
