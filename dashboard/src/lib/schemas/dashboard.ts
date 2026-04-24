import { z } from 'zod'
import { decodeSoundId } from '@/features/audio-studio/types'

export const uuidSchema = z.string().uuid()

export const databaseAccountsSearchSchema = z.object({
  search: z.string().catch(''),
  sort: z.enum(['email', 'characters', 'created']).catch('email'),
})

export const databaseTablesSearchSchema = z.object({
  search: z.string().catch(''),
  sort: z.enum(['name', 'rows', 'schema']).catch('schema'),
})

export const dashboardSessionLoginSchema = z.object({
  password: z.string().trim().min(1, 'password is required'),
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

export type DatabaseAccountsSearch = z.infer<
  typeof databaseAccountsSearchSchema
>
export type DatabaseTablesSearch = z.infer<typeof databaseTablesSearchSchema>
