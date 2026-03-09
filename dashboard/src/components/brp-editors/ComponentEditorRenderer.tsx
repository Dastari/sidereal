import * as React from 'react'
import type { GraphNode } from '@/components/grid/types'
import type {
  ComponentEditorFieldSchema,
  GeneratedComponentRegistryResource,
} from '@/features/component-schema/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import {
  getComponentPayloadFromNode,
  getSchemaFieldValue,
  resolveComponentRegistryEntry,
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
  const [draftValue, setDraftValue] = React.useState(payload)
  const [saveState, setSaveState] = React.useState<
    'idle' | 'pending' | 'saving' | 'saved' | 'error'
  >('idle')
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const saveSequenceRef = React.useRef(0)

  React.useEffect(() => {
    setDraftValue(payload)
  }, [payload])

  React.useEffect(() => {
    if (readOnly || !entry) {
      setSaveState('idle')
      setErrorText(null)
      return
    }

    const nextDirty = JSON.stringify(draftValue) !== JSON.stringify(payload)
    if (!nextDirty) {
      setSaveState((current) => (current === 'error' ? current : 'idle'))
      return
    }

    setSaveState('pending')
    setErrorText(null)
    const saveSequence = saveSequenceRef.current + 1
    saveSequenceRef.current = saveSequence
    const timeoutId = window.setTimeout(() => {
      setSaveState('saving')
      Promise.resolve(onUpdate(entry.type_path, entry.component_kind, draftValue))
        .then(() => {
          if (saveSequenceRef.current !== saveSequence) return
          setSaveState('saved')
          setErrorText(null)
        })
        .catch((error: unknown) => {
          if (saveSequenceRef.current !== saveSequence) return
          setSaveState('error')
          setErrorText(
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

  const { fields } = entry.editor_schema
  if (fields.length === 0) {
    return (
      <div className="rounded border border-dashed border-border px-3 py-2 text-xs text-muted-foreground">
        No editor schema fields available for this component.
      </div>
    )
  }

  const dirty = JSON.stringify(draftValue) !== JSON.stringify(payload)

  return (
    <div className="space-y-3">
      <div className="rounded border border-border bg-background/70 px-3 py-2">
        <div className="text-[11px] uppercase tracking-wider text-muted-foreground">
          Schema Editor
        </div>
        <div className="font-mono text-[11px] text-muted-foreground">
          {entry.type_path}
        </div>
      </div>
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
      <div className="flex items-center justify-end gap-2">
        <div className="mr-auto text-[11px] text-muted-foreground">
          {readOnly
            ? 'Read only'
            : saveState === 'pending'
              ? 'Auto-applying...'
              : saveState === 'saving'
                ? 'Saving...'
                : saveState === 'saved'
                  ? 'Saved'
                  : saveState === 'error'
                    ? errorText ?? 'Save failed'
                    : 'Auto-apply enabled'}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={!dirty}
          onClick={() => setDraftValue(payload)}
        >
          Reset
        </Button>
      </div>
    </div>
  )
}

function SchemaFieldEditor({
  field,
  value,
  readOnly,
  onChange,
}: {
  field: ComponentEditorFieldSchema
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const checked = value === true

  switch (field.value_kind) {
    case 'Bool':
      return (
        <FieldShell field={field}>
          <div className="flex items-center justify-end gap-2">
            <span className="text-xs text-muted-foreground">
              {checked ? 'On' : 'Off'}
            </span>
            <Switch
              checked={checked}
              disabled={readOnly}
              onCheckedChange={onChange}
            />
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
    <div className="space-y-2 rounded border border-border bg-background/60 px-3 py-2">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <div className="text-xs font-medium text-foreground">
            {field.display_name}
          </div>
          <div className="truncate font-mono text-[11px] text-muted-foreground">
            {field.field_path}
          </div>
        </div>
        {field.unit ? (
          <span className="shrink-0 text-[11px] text-muted-foreground">
            {field.unit}
          </span>
        ) : null}
      </div>
      {children}
    </div>
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
  const displayValue = formatNumericInputValue(safeValue, step)

  return (
    <div className="space-y-2">
      {shouldShowSlider ? (
        <Slider
          min={min}
          max={max}
          step={step}
          value={[safeValue]}
          disabled={readOnly}
          onValueChange={(next) => onChange(next[0] ?? safeValue)}
        />
      ) : null}
      <Input
        className="h-8 text-xs"
        type="number"
        disabled={readOnly}
        min={min}
        max={max}
        step={step}
        value={displayValue}
        onChange={(event) => {
          const next = Number(event.target.value)
          onChange(Number.isFinite(next) ? next : 0)
        }}
      />
    </div>
  )
}

function VectorEditor({
  dimensions,
  value,
  readOnly,
  onChange,
}: {
  dimensions: number
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const channels = normalizeVectorValue(value, dimensions)

  return (
    <div className="grid grid-cols-2 gap-2">
      {channels.map((channelValue, index) => (
        <Input
          key={index}
          className="h-8 text-xs"
          type="number"
          step="0.01"
          disabled={readOnly}
          value={formatNumericInputValue(channelValue, 0.01)}
          onChange={(event) => {
            const next = Number(event.target.value)
            if (!Number.isFinite(next)) return
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
  alpha,
  value,
  readOnly,
  onChange,
}: {
  alpha: boolean
  value: unknown
  readOnly: boolean
  onChange: (value: unknown) => void
}) {
  const dimensions = alpha ? 4 : 3
  const channels = normalizeVectorValue(value, dimensions)
  const [r, g, b] = channels

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3">
        <input
          className="h-8 w-10 rounded border border-input bg-background p-1"
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
        <div className="grid flex-1 grid-cols-2 gap-2">
          {channels.map((channelValue, index) => (
            <Input
              key={index}
              className="h-8 text-xs"
              type="number"
              step="0.01"
              min={0}
              disabled={readOnly}
              value={formatNumericInputValue(channelValue, 0.01)}
              onChange={(event) => {
                const next = Number(event.target.value)
                if (!Number.isFinite(next)) return
                const nextChannels = [...channels]
                nextChannels[index] = next
                onChange(writeVectorValue(value, nextChannels))
              }}
            />
          ))}
        </div>
      </div>
    </div>
  )
}

function normalizeVectorValue(value: unknown, dimensions: number): Array<number> {
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
  if (typeof currentValue === 'object' && currentValue !== null && !Array.isArray(currentValue)) {
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

function numericStepPrecision(step: number): number {
  if (!Number.isFinite(step) || step <= 0) {
    return 2
  }
  const normalized = step.toString().toLowerCase()
  if (normalized.includes('e-')) {
    const exponent = Number(normalized.split('e-')[1] ?? '0')
    return Number.isFinite(exponent) ? Math.min(exponent, 6) : 2
  }
  const fraction = normalized.split('.')[1] ?? ''
  return Math.min(fraction.length, 6)
}

function formatNumericInputValue(value: number, step: number): string {
  if (!Number.isFinite(value)) {
    return '0'
  }
  const precision = numericStepPrecision(step)
  if (precision <= 0) {
    return Math.round(value).toString()
  }
  return value
    .toFixed(precision)
    .replace(/\.?0+$/, '')
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
