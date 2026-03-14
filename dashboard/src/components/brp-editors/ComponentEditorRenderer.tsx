import * as React from 'react'
import type { GraphNode } from '@/components/grid/types'
import type {
  ComponentEditorFieldSchema,
  GeneratedComponentRegistryResource,
  ShaderEditorFieldSchema,
} from '@/features/component-schema/types'
import { ButtonGroup, ButtonGroupText } from '@/components/ui/button-group'
import { HUDFrame } from '@/components/ui/hud-frame'
import { TheGridNumberInput } from '@/components/thegridcn/thegrid-number-input'
import { TheGridSlider } from '@/components/thegridcn/thegrid-slider'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import {
  getComponentPayloadFromNode,
  getSchemaFieldValue,
  resolveComponentRegistryEntry,
  resolveShaderRegistryEntryForComponent,
  setSchemaFieldValue,
} from '@/features/component-schema/registry'

export interface ComponentEditorRendererProps {
  componentNodeId: string
  entityId: string
  node: GraphNode
  generatedComponentRegistry: GeneratedComponentRegistryResource | null
  onUpdate: (
    typePath: string,
    componentKind: string,
    value: unknown,
  ) => Promise<void> | void
  readOnly?: boolean
}

type ResolvedComponentFieldSchema = ComponentEditorFieldSchema & {
  shader_options?: Array<{ value: string; label: string }>
}

export function ComponentEditorRenderer({
  componentNodeId: _componentNodeId,
  entityId: _entityId,
  node,
  generatedComponentRegistry,
  onUpdate,
  readOnly = false,
}: ComponentEditorRendererProps) {
  const AUTO_APPLY_DEBOUNCE_MS = 250
  const entry = React.useMemo(
    () => resolveComponentRegistryEntry(node, generatedComponentRegistry),
    [generatedComponentRegistry, node],
  )
  const payload = React.useMemo(
    () => getComponentPayloadFromNode(node, entry),
    [entry, node],
  )
  const shaderSchemaEntry = React.useMemo(
    () =>
      entry
        ? resolveShaderRegistryEntryForComponent(
            generatedComponentRegistry,
            entry.type_path,
          )
        : null,
    [entry, generatedComponentRegistry],
  )
  const [draftValue, setDraftValue] = React.useState(payload)
  const saveSequenceRef = React.useRef(0)

  React.useEffect(() => {
    setDraftValue(payload)
  }, [payload])

  const fields = React.useMemo(() => {
    if (!entry) {
      return []
    }
    if (!shaderSchemaEntry) {
      return entry.editor_schema.fields
    }

    return shaderSchemaEntry.uniform_schema.flatMap((shaderField) => {
      const componentField = entry.editor_schema.fields.find(
        (field) => field.field_path === shaderField.field_path,
      )
      if (!componentField) {
        return []
      }
      return [mergeShaderFieldOverrides(componentField, shaderField)]
    })
  }, [entry, shaderSchemaEntry])

  React.useEffect(() => {
    if (readOnly || !entry) {
      return
    }

    const nextDirty = JSON.stringify(draftValue) !== JSON.stringify(payload)
    if (!nextDirty) {
      return
    }

    const saveSequence = saveSequenceRef.current + 1
    saveSequenceRef.current = saveSequence
    const timeoutId = window.setTimeout(() => {
      Promise.resolve(
        onUpdate(entry.type_path, entry.component_kind, draftValue),
      )
        .then(() => {
          if (saveSequenceRef.current !== saveSequence) return
        })
        .catch((error: unknown) => {
          if (saveSequenceRef.current !== saveSequence) return
          console.error(
            error instanceof Error ? error.message : 'Component update failed',
          )
        })
    }, AUTO_APPLY_DEBOUNCE_MS)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [draftValue, entry, onUpdate, payload, readOnly])

  if (!entry) {
    return null
  }
  if (fields.length === 0) {
    return (
      <HUDFrame className="border-dashed px-3 py-2 text-xs text-muted-foreground">
        No editor schema fields available for this component.
      </HUDFrame>
    )
  }

  return (
    <div className="space-y-3">
      <HUDFrame className="px-3 py-2" label="Schema Editor">
        <div className="truncate font-mono text-[11px] text-muted-foreground">
          {entry.type_path}
        </div>
        {shaderSchemaEntry ? (
          <div className="mt-1 text-[11px] text-muted-foreground">
            Shader schema: {shaderSchemaEntry.asset_id}
          </div>
        ) : null}
      </HUDFrame>
      <div className="space-y-3">
        {fields.map((field) => (
          <SchemaFieldEditor
            key={field.field_path}
            field={field}
            value={getSchemaFieldValue(draftValue, field, fields.length)}
            readOnly={readOnly}
            onChange={(nextValue) => {
              setDraftValue((current: unknown) =>
                setSchemaFieldValue(current, field, nextValue, fields.length),
              )
            }}
          />
        ))}
      </div>
    </div>
  )
}

function mergeShaderFieldOverrides(
  field: ComponentEditorFieldSchema,
  shaderField: ShaderEditorFieldSchema | null,
): ResolvedComponentFieldSchema {
  if (!shaderField) {
    return field
  }
  return {
    ...field,
    display_name: shaderField.display_name || field.display_name,
    min: typeof shaderField.min === 'number' ? shaderField.min : field.min,
    max: typeof shaderField.max === 'number' ? shaderField.max : field.max,
    step: typeof shaderField.step === 'number' ? shaderField.step : field.step,
    shader_options: shaderField.options,
  }
}

function SchemaFieldEditor({
  field,
  value,
  readOnly,
  onChange,
}: {
  field: ResolvedComponentFieldSchema
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const checked = value === true
  const shaderNumericOptions = React.useMemo(
    () =>
      (field.shader_options ?? [])
        .map((option) => ({
          ...option,
          numericValue: Number(option.value),
        }))
        .filter((option) => Number.isFinite(option.numericValue)),
    [field.shader_options],
  )

  if (
    shaderNumericOptions.length > 0 &&
    (field.value_kind === 'UnsignedInteger' ||
      field.value_kind === 'SignedInteger' ||
      field.value_kind === 'Float')
  ) {
    return (
      <FieldShell field={field}>
        <select
          className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs"
          disabled={readOnly}
          value={String(typeof value === 'number' ? value : Number(value ?? 0))}
          onChange={(event) => {
            const next = Number(event.target.value)
            if (!Number.isFinite(next)) return
            onChange(next)
          }}
        >
          {shaderNumericOptions.map((option) => (
            <option key={option.value} value={option.numericValue}>
              {option.label}
            </option>
          ))}
        </select>
      </FieldShell>
    )
  }

  switch (field.value_kind) {
    case 'Bool':
      return (
        <FieldShell field={field}>
          <div className="flex justify-end">
            <ButtonGroup>
              <ButtonGroupText className="justify-center px-3 text-xs text-muted-foreground">
                {checked ? 'On' : 'Off'}
              </ButtonGroupText>
              <ButtonGroupText className="px-3">
                <Switch
                  checked={checked}
                  disabled={readOnly}
                  onCheckedChange={onChange}
                />
              </ButtonGroupText>
            </ButtonGroup>
          </div>
        </FieldShell>
      )
    case 'Enum':
      return (
        <FieldShell field={field}>
          <select
            className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs"
            disabled={readOnly}
            value={typeof value === 'string' ? value : ''}
            onChange={(event) => onChange(event.target.value)}
          >
            {field.options.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </FieldShell>
      )
    case 'SignedInteger':
    case 'UnsignedInteger':
    case 'Float':
      return (
        <FieldShell field={field}>
          <NumberEditor
            field={field}
            value={value}
            readOnly={readOnly}
            onChange={onChange}
          />
        </FieldShell>
      )
    case 'String':
      return (
        <FieldShell field={field}>
          <Input
            className="h-8 text-xs"
            disabled={readOnly}
            value={typeof value === 'string' ? value : ''}
            onChange={(event) => onChange(event.target.value)}
          />
        </FieldShell>
      )
    case 'Vec2':
    case 'Vec3':
    case 'Vec4':
      return (
        <FieldShell field={field}>
          <VectorEditor
            field={field}
            dimensions={Number(field.value_kind.slice(3))}
            value={value}
            readOnly={readOnly}
            onChange={onChange}
          />
        </FieldShell>
      )
    case 'ColorRgb':
      return (
        <FieldShell field={field}>
          <ColorEditor
            field={field}
            alpha={false}
            value={value}
            readOnly={readOnly}
            onChange={onChange}
          />
        </FieldShell>
      )
    case 'ColorRgba':
      return (
        <FieldShell field={field}>
          <ColorEditor
            field={field}
            alpha
            value={value}
            readOnly={readOnly}
            onChange={onChange}
          />
        </FieldShell>
      )
    default:
      return (
        <FieldShell field={field}>
          <textarea
            className="min-h-24 w-full rounded-md border border-input bg-background px-3 py-2 font-mono text-xs"
            disabled={readOnly || field.value_kind === 'Sequence'}
            value={JSON.stringify(value ?? null, null, 2)}
            onChange={(event) => {
              try {
                onChange(JSON.parse(event.target.value) as unknown)
              } catch {
                // Keep the last valid draft until JSON parses.
              }
            }}
          />
        </FieldShell>
      )
  }
}

function FieldShell({
  field,
  children,
}: {
  field: ComponentEditorFieldSchema
  children: React.ReactNode
}) {
  return (
    <HUDFrame className="px-3 py-2" label={field.display_name}>
      <div className="flex items-center justify-between gap-3">
        {/* <div className="min-w-0 flex flex-row items-center gap-2">
          <div className="text-xs font-medium text-foreground">
            {field.display_name}
          </div>
          <div className="truncate font-mono text-[11px] text-muted-foreground">
            {field.field_path}
          </div>
        </div> */}
        {field.unit ? (
          <span className="shrink-0 text-[11px] text-muted-foreground">
            {field.unit}
          </span>
        ) : null}
      </div>
      {children}
    </HUDFrame>
  )
}

function NumberEditor({
  field,
  value,
  readOnly,
  onChange,
}: {
  field: ComponentEditorFieldSchema
  value: unknown
  readOnly: boolean
  onChange: (value: number) => void
}) {
  const numericValue = typeof value === 'number' ? value : Number(value ?? 0)
  const safeValue = Number.isFinite(numericValue) ? numericValue : 0
  const step = field.step ?? (field.value_kind === 'Float' ? 0.01 : 1)
  const min = field.min ?? undefined
  const max = field.max ?? undefined
  const shouldShowSlider = min !== undefined && max !== undefined

  return (
    <div className="space-y-2">
      {shouldShowSlider ? (
        <div className="flex items-center gap-2">
          <TheGridSlider
            className="flex-1"
            min={min}
            max={max}
            step={step}
            value={safeValue}
            disabled={readOnly}
            onChange={(next) => onChange(next)}
          />
          <TheGridNumberInput
            className="shrink-0"
            inputClassName="w-24"
            disabled={readOnly}
            min={min}
            max={max}
            step={step}
            value={safeValue}
            onChange={(next) => onChange(next)}
          />
        </div>
      ) : null}
      {!shouldShowSlider ? (
        <TheGridNumberInput
          disabled={readOnly}
          min={min}
          max={max}
          step={step}
          value={safeValue}
          onChange={(next) => onChange(next)}
        />
      ) : null}
    </div>
  )
}

function VectorEditor({
  field,
  dimensions,
  value,
  readOnly,
  onChange,
}: {
  field: ComponentEditorFieldSchema
  dimensions: number
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const channels = normalizeVectorValue(value, dimensions)
  const step = field.step ?? 0.01
  const min = field.min ?? undefined
  const max = field.max ?? undefined

  return (
    <div className="grid grid-cols-2 gap-2">
      {channels.map((channelValue, index) => (
        <TheGridNumberInput
          key={index}
          inputClassName="w-full"
          step={step}
          min={min}
          max={max}
          disabled={readOnly}
          value={channelValue}
          onChange={(next) => {
            const nextChannels = [...channels]
            nextChannels[index] = next
            onChange(writeVectorValue(value, nextChannels))
          }}
        />
      ))}
    </div>
  )
}

function ColorEditor({
  field,
  alpha,
  value,
  readOnly,
  onChange,
}: {
  field: ComponentEditorFieldSchema
  alpha: boolean
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const dimensions = alpha ? 4 : 3
  const channels = normalizeVectorValue(value, dimensions)
  const [r, g, b] = channels
  const labels = alpha ? ['R', 'G', 'B', 'A'] : ['R', 'G', 'B']
  const step = field.step ?? 0.01
  const min = field.min ?? 0
  const max = field.max ?? 1

  return (
    <div className="overflow-x-auto">
      <div className="flex min-w-0 items-center gap-2">
        <input
          className="h-8 w-10 shrink-0 rounded border border-input bg-background p-1"
          type="color"
          disabled={readOnly}
          value={rgbChannelsToHex([r, g, b])}
          onChange={(event) => {
            const [nextR, nextG, nextB] = hexToRgbChannels(event.target.value)
            const nextChannels = [
              nextR,
              nextG,
              nextB,
              ...(alpha ? [channels[3] ?? 1] : []),
            ]
            onChange(writeVectorValue(value, nextChannels))
          }}
        />
        <div className="flex min-w-0 flex-1 items-center gap-1">
          {channels.map((channelValue, index) => (
            <label key={index} className="min-w-0 flex-1">
              <div className="mb-1 text-center text-[10px] font-medium uppercase leading-none text-muted-foreground">
                {labels[index]}
              </div>
              <TheGridNumberInput
                inputClassName="w-full"
                step={step}
                min={min}
                max={max}
                disabled={readOnly}
                value={channelValue}
                onChange={(next) => {
                  const nextChannels = [...channels]
                  nextChannels[index] = next
                  onChange(writeVectorValue(value, nextChannels))
                }}
              />
            </label>
          ))}
        </div>
      </div>
    </div>
  )
}

function normalizeVectorValue(
  value: unknown,
  dimensions: number,
): Array<number> {
  if (Array.isArray(value)) {
    return Array.from({ length: dimensions }, (_, index) =>
      typeof value[index] === 'number' ? value[index] : 0,
    )
  }
  if (typeof value === 'object' && value !== null) {
    const record = value as Record<string, unknown>
    const keys = ['x', 'y', 'z', 'w']
    return Array.from({ length: dimensions }, (_, index) => {
      const key = keys[index] ?? String(index)
      const channel = record[key] ?? record[String(index)]
      return typeof channel === 'number' ? channel : 0
    })
  }
  return Array.from({ length: dimensions }, () => 0)
}

function writeVectorValue(
  currentValue: unknown,
  channels: Array<number>,
): Array<number> | Record<string, number> {
  if (
    typeof currentValue === 'object' &&
    currentValue !== null &&
    !Array.isArray(currentValue)
  ) {
    const record = currentValue as Record<string, unknown>
    const keys = ['x', 'y', 'z', 'w']
    const next: Record<string, number> = {}
    for (const [key, value] of Object.entries(record)) {
      if (typeof value === 'number' && keys.includes(key)) {
        next[key] = channels[keys.indexOf(key)] ?? 0
      }
    }
    if (Object.keys(next).length > 0) {
      return next
    }
  }
  return channels
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function rgbChannelsToHex([r, g, b]: Array<number>): string {
  return `#${[r, g, b]
    .map((channel) =>
      Math.round(clamp01(channel) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`
}

function hexToRgbChannels(value: string): [number, number, number] {
  const normalized = value.replace('#', '')
  const safe = normalized.length === 6 ? normalized : '000000'
  return [
    parseInt(safe.slice(0, 2), 16) / 255,
    parseInt(safe.slice(2, 4), 16) / 255,
    parseInt(safe.slice(4, 6), 16) / 255,
  ]
}
