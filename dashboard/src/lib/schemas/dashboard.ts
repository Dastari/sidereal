import { z } from 'zod'

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

export const brpRequestSchema = z.object({
  id: z.unknown().optional(),
  method: z
    .string()
    .trim()
    .min(1, 'Body must include a JSON-RPC method string'),
  params: z.unknown().optional(),
  target: brpTargetSchema.optional(),
  port: brpPortSchema.optional(),
})

export type DatabaseAccountsSearch = z.infer<
  typeof databaseAccountsSearchSchema
>
export type DatabaseTablesSearch = z.infer<typeof databaseTablesSearchSchema>
