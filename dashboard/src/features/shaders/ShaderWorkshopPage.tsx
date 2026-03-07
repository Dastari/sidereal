import {
  FlaskConical,
  FolderUp,
  Pause,
  Play,
  RefreshCw,
  Save,
  Square,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  parseAsString,
  useQueryStates,
} from 'nuqs'
import type { ShaderCatalogEntry } from '@/components/shader-workbench/ShaderLibraryTree'
import type {
  ShaderPreviewDiagnostic,
  ShaderPreviewUniformDescriptor,
  ShaderPreviewUniformValues,
} from '@/lib/shader-preview'
import {
  AppLayout,
  Panel,
  PanelContent,
  PanelHeader,
} from '@/components/layout/AppLayout'
import {
  HorizontalSplitPanels,
  VerticalSplitPanels,
} from '@/components/layout/ResizablePanels'
import { ShaderCodeEditor } from '@/components/shader-workbench/ShaderCodeEditor'
import { ShaderLibraryTree } from '@/components/shader-workbench/ShaderLibraryTree'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Slider } from '@/components/ui/slider'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { applyWithShaderPreviewWasm } from '@/lib/shader-preview-wasm'
import {
  buildDefaultUniformValues,
  extractPreviewUniforms,
  renderPreviewShader,
} from '@/lib/shader-preview'
import { useSessionStorageNumber } from '@/hooks/use-session-storage-number'

type ShaderCatalogResponse = {
  shaders: Array<ShaderCatalogEntry>
  elapsedMs: number
  error?: string
}

type ShaderFileResponse = {
  entry: ShaderCatalogEntry
  source: string
  elapsedMs: number
  error?: string
}

type UploadResponse = {
  entry: ShaderCatalogEntry
  elapsedMs: number
  error?: string
}

type PerfPanelState = {
  catalogLoadMs: number | null
  shaderLoadMs: number | null
  uploadMs: number | null
  previewValidateMs: number | null
  previewPipelineMs: number | null
  previewApplyMs: number | null
  previewFrameMs: number | null
}

const DEFAULT_SHADER_SIDEBAR_WIDTH = 300
const DEFAULT_SHADER_DETAIL_WIDTH = 360
const DEFAULT_SHADER_EDITOR_WIDTH = 720
const DEFAULT_SHADER_PREVIEW_HEIGHT = 420
const SHADER_PREVIEW_ASPECT_RATIO = 3 / 2

export interface ShaderWorkshopPageProps {
  selectedShaderId?: string | null
  onSelectedShaderIdChange?: (shaderId: string | null) => void
}

export function ShaderWorkshopPage({
  selectedShaderId = null,
  onSelectedShaderIdChange,
}: ShaderWorkshopPageProps) {
  const fileInputRef = useRef<HTMLInputElement>(null)
  const previewCanvasRef = useRef<HTMLCanvasElement>(null)
  const previewViewportRef = useRef<HTMLDivElement>(null)
  const previewRenderIdRef = useRef(0)
  const simulationTimeRef = useRef(0)
  const lastSimulationTickRef = useRef<number | null>(null)
  const [routeState, setRouteState] = useQueryStates({
    search: parseAsString.withDefault(''),
    fallbackShaderId: parseAsString,
  })
  const [shaders, setShaders] = useState<Array<ShaderCatalogEntry>>([])
  const [source, setSource] = useState('')
  const [editorValue, setEditorValue] = useState('')
  const [isLoadingCatalog, setIsLoadingCatalog] = useState(false)
  const [isLoadingShader, setIsLoadingShader] = useState(false)
  const [isUploading, setIsUploading] = useState(false)
  const [statusText, setStatusText] = useState<string | null>(null)
  const [errorText, setErrorText] = useState<string | null>(null)
  const [previewSurfaceSize, setPreviewSurfaceSize] = useState({
    width: 720,
    height: 480,
  })
  const [previewDiagnostics, setPreviewDiagnostics] = useState<
    Array<ShaderPreviewDiagnostic>
  >([])
  const [previewStatus, setPreviewStatus] = useState<
    'idle' | 'validating' | 'valid' | 'invalid'
  >('idle')
  const [previewBackend, setPreviewBackend] = useState<string>('WebGPU compile')
  const [perf, setPerf] = useState<PerfPanelState>({
    catalogLoadMs: null,
    shaderLoadMs: null,
    uploadMs: null,
    previewValidateMs: null,
    previewPipelineMs: null,
    previewApplyMs: null,
    previewFrameMs: null,
  })
  const [uniformValues, setUniformValues] = useState<ShaderPreviewUniformValues>({})
  const [activePreviewSource, setActivePreviewSource] = useState('')
  const [isPreviewPaused, setIsPreviewPaused] = useState(false)
  const [simulationSpeed, setSimulationSpeed] = useState(1)
  const [isDragActive, setIsDragActive] = useState(false)
  const [sidebarWidth, setSidebarWidth] = useSessionStorageNumber(
    'dashboard:shader-workshop:sidebar-width',
    DEFAULT_SHADER_SIDEBAR_WIDTH,
  )
  const [detailPanelWidth, setDetailPanelWidth] = useSessionStorageNumber(
    'dashboard:shader-workshop:detail-panel-width',
    DEFAULT_SHADER_DETAIL_WIDTH,
  )
  const [editorWidth, setEditorWidth] = useSessionStorageNumber(
    'dashboard:shader-workshop:editor-width',
    DEFAULT_SHADER_EDITOR_WIDTH,
  )
  const [previewHeight, setPreviewHeight] = useSessionStorageNumber(
    'dashboard:shader-workshop:preview-height',
    DEFAULT_SHADER_PREVIEW_HEIGHT,
  )

  const effectiveSelectedShaderId = selectedShaderId ?? routeState.fallbackShaderId ?? null
  const selectedShader = useMemo(
    () =>
      shaders.find((entry) => entry.shaderId === effectiveSelectedShaderId) ?? null,
    [effectiveSelectedShaderId, shaders],
  )
  const search = routeState.search
  const hasUnsavedChanges = editorValue !== source
  const shaderUniforms = useMemo(
    () => extractPreviewUniforms(editorValue),
    [editorValue],
  )
  const animatedUniformTargets = useMemo(
    () =>
      shaderUniforms.flatMap((uniform) =>
        uniform.labels.flatMap((label, componentIndex) => {
          const lower = `${uniform.name} ${label}`.toLowerCase()
          if (
            lower.includes('time') ||
            lower.includes('age') ||
            lower.includes('life') ||
            lower.includes('progress')
          ) {
            return [{ uniformName: uniform.name, componentIndex, lower }]
          }
          return []
        }),
      ),
    [shaderUniforms],
  )

  const resetPreviewSimulation = useCallback(() => {
    simulationTimeRef.current = 0
    lastSimulationTickRef.current = null
    setUniformValues((prev) => {
      let changed = false
      const next = { ...prev }

      for (const uniform of shaderUniforms) {
        const hasAnimatedComponent = uniform.labels.some((label) => {
          const lower = `${uniform.name} ${label}`.toLowerCase()
          return (
            lower.includes('time') ||
            lower.includes('age') ||
            lower.includes('life') ||
            lower.includes('progress')
          )
        })

        if (!hasAnimatedComponent) {
          continue
        }

        const defaults = [...uniform.defaults]
        const current = prev[uniform.name] ?? []
        const isSame =
          current.length === defaults.length &&
          current.every((component, index) => component === defaults[index])

        if (!isSame) {
          next[uniform.name] = defaults
          changed = true
        }
      }

      return changed ? next : prev
    })
  }, [shaderUniforms])

  useEffect(() => {
    const defaults = buildDefaultUniformValues(shaderUniforms)
    setUniformValues((prev) => {
      const next: ShaderPreviewUniformValues = {}
      for (const uniform of shaderUniforms) {
        next[uniform.name] =
          Object.hasOwn(prev, uniform.name) &&
          prev[uniform.name].length === uniform.components
            ? prev[uniform.name]
            : defaults[uniform.name]
      }
      return next
    })
  }, [shaderUniforms])

  useEffect(() => {
    simulationTimeRef.current = 0
    lastSimulationTickRef.current = null
    setIsPreviewPaused(false)
  }, [effectiveSelectedShaderId])

  useEffect(() => {
    const viewport = previewViewportRef.current
    const canvas = previewCanvasRef.current
    if (!viewport || !canvas) {
      return
    }

    let frameId = 0

    const updatePreviewSurface = () => {
      const bounds = viewport.getBoundingClientRect()
      if (bounds.width <= 0 || bounds.height <= 0) {
        return
      }

      const containerAspectRatio = bounds.width / bounds.height
      const nextCssWidth =
        containerAspectRatio > SHADER_PREVIEW_ASPECT_RATIO
          ? bounds.height * SHADER_PREVIEW_ASPECT_RATIO
          : bounds.width
      const nextCssHeight = nextCssWidth / SHADER_PREVIEW_ASPECT_RATIO
      const nextPixelRatio = window.devicePixelRatio || 1
      const nextPixelWidth = Math.max(
        1,
        Math.round(nextCssWidth * nextPixelRatio),
      )
      const nextPixelHeight = Math.max(
        1,
        Math.round(nextCssHeight * nextPixelRatio),
      )

      setPreviewSurfaceSize((prev) => {
        if (
          Math.round(prev.width) === Math.round(nextCssWidth) &&
          Math.round(prev.height) === Math.round(nextCssHeight)
        ) {
          return prev
        }
        return {
          width: nextCssWidth,
          height: nextCssHeight,
        }
      })

      if (
        canvas.width !== nextPixelWidth ||
        canvas.height !== nextPixelHeight
      ) {
        canvas.width = nextPixelWidth
        canvas.height = nextPixelHeight
      }
    }

    const scheduleUpdate = () => {
      window.cancelAnimationFrame(frameId)
      frameId = window.requestAnimationFrame(updatePreviewSurface)
    }

    scheduleUpdate()

    const resizeObserver = new ResizeObserver(() => {
      scheduleUpdate()
    })
    resizeObserver.observe(viewport)

    return () => {
      window.cancelAnimationFrame(frameId)
      resizeObserver.disconnect()
    }
  }, [])

  const commitSelectedShaderId = useCallback(
    (shaderId: string | null) => {
      void setRouteState({ fallbackShaderId: shaderId })
      onSelectedShaderIdChange?.(shaderId)
    },
    [onSelectedShaderIdChange, setRouteState],
  )

  const loadCatalog = useCallback(async () => {
    setIsLoadingCatalog(true)
    setErrorText(null)
    try {
      const response = await fetch('/api/shaders')
      const payload = (await response.json()) as ShaderCatalogResponse
      if (!response.ok || payload.error) {
        throw new Error(payload.error ?? 'Failed to load shader catalog')
      }
      setShaders(payload.shaders)
      setPerf((prev) => ({ ...prev, catalogLoadMs: payload.elapsedMs }))
      if (
        effectiveSelectedShaderId &&
        payload.shaders.some((entry) => entry.shaderId === effectiveSelectedShaderId)
      ) {
        return
      }
      const resolvedShaderId =
        payload.shaders[0]?.shaderId ?? null
      if (resolvedShaderId !== effectiveSelectedShaderId) {
        commitSelectedShaderId(resolvedShaderId)
      }
    } catch (error) {
      setErrorText(
        error instanceof Error ? error.message : 'Failed to load shader catalog',
      )
    } finally {
      setIsLoadingCatalog(false)
    }
  }, [commitSelectedShaderId, effectiveSelectedShaderId])

  const runPreviewValidation = useCallback(
    async (
      sourceToValidate: string,
      valuesToValidate: ShaderPreviewUniformValues,
    ) => {
      setPreviewStatus('validating')
      setErrorText(null)
      setPreviewBackend('Rust WASM validate + WebGPU preview')
      try {
        const canvas = previewCanvasRef.current
        if (!canvas) {
          throw new Error('Preview canvas is not available')
        }
        const wasmApply = await applyWithShaderPreviewWasm(sourceToValidate)
        const browserPreview = await renderPreviewShader(
          canvas,
          sourceToValidate,
          valuesToValidate,
        )
        const diagnostics = [
          ...wasmApply.diagnostics.map((diagnostic) => ({
            message: diagnostic.message,
            line: diagnostic.line,
            column: diagnostic.column,
            type: diagnostic.type ?? 'error',
          })),
          ...browserPreview.diagnostics,
        ]
        setPreviewDiagnostics(diagnostics)
        setPerf((prev) => ({
          ...prev,
          previewValidateMs: wasmApply.metrics.validate_ms,
          previewPipelineMs: browserPreview.metrics.pipelineMs,
          previewApplyMs:
            wasmApply.metrics.apply_ms + browserPreview.metrics.applyMs,
          previewFrameMs: browserPreview.metrics.frameMs,
        }))
        setActivePreviewSource(sourceToValidate)
        setPreviewStatus(
          wasmApply.ok && browserPreview.ok ? 'valid' : 'invalid',
        )
        setStatusText(
          wasmApply.ok && browserPreview.ok
            ? `Shader rendered in preview with ${browserPreview.uniforms.length} uniform input${browserPreview.uniforms.length === 1 ? '' : 's'}`
            : 'Shader failed validation or preview rendering',
        )
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Preview validation failed'
        setPreviewDiagnostics([
          { message, line: null, column: null, type: 'error' },
        ])
        setPreviewBackend('Rust WASM unavailable')
        setPreviewStatus('invalid')
        setPerf((prev) => ({
          ...prev,
          previewValidateMs: null,
          previewPipelineMs: null,
          previewApplyMs: null,
          previewFrameMs: null,
        }))
      }
    },
    [],
  )

  const loadShader = useCallback(async (shaderId: string) => {
    setIsLoadingShader(true)
    setErrorText(null)
    try {
      const response = await fetch(`/api/shaders/${shaderId}`)
      const payload = (await response.json()) as ShaderFileResponse
      if (!response.ok || payload.error) {
        throw new Error(payload.error ?? 'Failed to load shader')
      }
      const nextUniformValues = buildDefaultUniformValues(
        extractPreviewUniforms(payload.source),
      )
      setSource(payload.source)
      setEditorValue(payload.source)
      setUniformValues(nextUniformValues)
      setActivePreviewSource('')
      setPreviewStatus('idle')
      setPerf((prev) => ({ ...prev, shaderLoadMs: payload.elapsedMs }))
      setStatusText(`Loaded ${payload.entry.filename}`)
      await runPreviewValidation(payload.source, nextUniformValues)
    } catch (error) {
      setErrorText(error instanceof Error ? error.message : 'Failed to load shader')
    } finally {
      setIsLoadingShader(false)
    }
  }, [runPreviewValidation])

  useEffect(() => {
    void loadCatalog()
  }, [loadCatalog])

  useEffect(() => {
    if (!effectiveSelectedShaderId) {
      setSource('')
      setEditorValue('')
      return
    }
    void loadShader(effectiveSelectedShaderId)
  }, [effectiveSelectedShaderId, loadShader])

  const handleUploadSource = useCallback(
    async (filename: string, nextSource: string) => {
      setIsUploading(true)
      setErrorText(null)
      try {
        const response = await fetch('/api/shaders/upload', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ filename, source: nextSource }),
        })
        const payload = (await response.json()) as UploadResponse
        if (!response.ok || payload.error) {
          throw new Error(payload.error ?? 'Upload failed')
        }
        setPerf((prev) => ({ ...prev, uploadMs: payload.elapsedMs }))
        setStatusText(`Uploaded ${payload.entry.filename} to source + cache`)
        await loadCatalog()
        commitSelectedShaderId(payload.entry.shaderId)
        setSource(nextSource)
        setEditorValue(nextSource)
      } catch (error) {
        setErrorText(error instanceof Error ? error.message : 'Upload failed')
      } finally {
        setIsUploading(false)
      }
    },
    [commitSelectedShaderId, loadCatalog],
  )

  const handleDroppedFiles = useCallback(
    async (files: FileList | null) => {
      const file = files?.[0]
      if (!file) return
      if (!file.name.toLowerCase().endsWith('.wgsl')) {
        setErrorText('Only .wgsl files can be loaded into the shader workbench')
        return
      }
      const droppedSource = await file.text()
      await handleUploadSource(file.name, droppedSource)
    },
    [handleUploadSource],
  )

  useEffect(() => {
    if (!activePreviewSource || previewStatus === 'validating') {
      return
    }

    const canvas = previewCanvasRef.current
    if (!canvas) {
      return
    }

    previewRenderIdRef.current += 1
    const renderId = previewRenderIdRef.current
    void (async () => {
      const browserPreview = await renderPreviewShader(
        canvas,
        activePreviewSource,
        uniformValues,
      )
      if (renderId !== previewRenderIdRef.current) {
        return
      }
      setPreviewDiagnostics((prev) => {
        const wasmDiagnostics = prev.filter((diagnostic) =>
          diagnostic.message.includes('Rust/WASM'),
        )
        return [...wasmDiagnostics, ...browserPreview.diagnostics]
      })
      setPerf((prev) => ({
        ...prev,
        previewPipelineMs: browserPreview.metrics.pipelineMs,
        previewApplyMs:
          (prev.previewValidateMs ?? 0) + browserPreview.metrics.applyMs,
        previewFrameMs: browserPreview.metrics.frameMs,
      }))
      setPreviewStatus(browserPreview.ok ? 'valid' : 'invalid')
    })().catch((error) => {
      if (cancelled) {
        return
      }
      const message =
        error instanceof Error ? error.message : 'Preview rerender failed'
      setPreviewDiagnostics([
        { message, line: null, column: null, type: 'error' },
      ])
      setPreviewStatus('invalid')
    })

    return undefined
  }, [activePreviewSource, previewStatus, previewSurfaceSize, uniformValues])

  useEffect(() => {
    if (
      !activePreviewSource ||
      animatedUniformTargets.length === 0 ||
      isPreviewPaused
    ) {
      return
    }

    let frameId = 0
    const tick = (now: number) => {
      const previous = lastSimulationTickRef.current
      lastSimulationTickRef.current = now
      const deltaSeconds =
        previous === null ? 0 : Math.min((now - previous) / 1000, 0.1)
      simulationTimeRef.current += deltaSeconds * simulationSpeed

      if (deltaSeconds > 0) {
        setUniformValues((prev) => {
          let changed = false
          const next = { ...prev }

          for (const target of animatedUniformTargets) {
            const current = [...(next[target.uniformName] ?? [])]
            const currentValue = current[target.componentIndex] ?? 0
            let updated = currentValue

            if (target.lower.includes('age') || target.lower.includes('life') || target.lower.includes('progress')) {
              updated = (currentValue + deltaSeconds * simulationSpeed * 0.2) % 1
            } else if (target.lower.includes('time')) {
              updated = simulationTimeRef.current
            }

            if (updated !== currentValue) {
              current[target.componentIndex] = updated
              next[target.uniformName] = current
              changed = true
            }
          }

          return changed ? next : prev
        })
      }

      frameId = window.requestAnimationFrame(tick)
    }

    frameId = window.requestAnimationFrame(tick)
    return () => {
      window.cancelAnimationFrame(frameId)
      lastSimulationTickRef.current = null
    }
  }, [activePreviewSource, animatedUniformTargets, isPreviewPaused, simulationSpeed])

  return (
    <AppLayout
      sidebar={
        <Panel>
          <PanelHeader className="py-2">
            <div className="space-y-3">
              <div className="flex items-center justify-between gap-2">
                <div>
                  <h1 className="text-sm font-semibold text-foreground">
                    Shader Library
                  </h1>
                  <p className="text-xs text-muted-foreground">
                    Grouped by shader class using the existing dashboard sidebar pattern.
                  </p>
                </div>
                <Badge variant="secondary">{shaders.length}</Badge>
              </div>
              <Input
                value={search}
                onChange={(event) => {
                  void setRouteState({ search: event.target.value })
                }}
                placeholder="Search shaders"
                className="h-8"
                aria-label="Search shaders"
              />
            </div>
          </PanelHeader>
          <PanelContent>
            <ShaderLibraryTree
              shaders={shaders}
              selectedShaderId={selectedShaderId}
              onSelect={(shaderId) => commitSelectedShaderId(shaderId)}
              search={search}
            />
          </PanelContent>
        </Panel>
      }
      sidebarWidth={sidebarWidth}
      onSidebarResize={(width) => {
        setSidebarWidth(width)
      }}
      detailPanelWidth={detailPanelWidth}
      onDetailPanelResize={(width) => {
        setDetailPanelWidth(width)
      }}
      detailPanel={
        <Panel>
          <PanelHeader className="py-2">
            <div>
              <h2 className="text-sm font-semibold text-foreground">
                Metadata & Performance
              </h2>
              <p className="text-xs text-muted-foreground">
                Shader asset parity, preview timings, diagnostics, and preview controls.
              </p>
            </div>
          </PanelHeader>
          <PanelContent className="p-4">
            <div className="space-y-4">
              <section className="space-y-2 rounded-md border border-border-subtle p-3 text-sm">
                <div className="font-medium">Selected Asset</div>
                <div className="text-muted-foreground">
                  {selectedShader?.sourcePath ?? 'No shader selected'}
                </div>
                <DetailRow
                  label="Asset ID"
                  value={selectedShader?.assetId ?? 'Unregistered'}
                />
                <DetailRow
                  label="Shader Role"
                  value={selectedShader?.shaderRole ?? 'Unspecified'}
                />
                <DetailRow
                  label="Class"
                  value={selectedShader?.shaderClass ?? 'unknown'}
                />
                <DetailRow
                  label="Bootstrap"
                  value={
                    selectedShader?.bootstrapRequired == null
                      ? 'Unknown'
                      : selectedShader.bootstrapRequired
                        ? 'Required'
                        : 'Optional'
                  }
                />
                <DetailRow
                  label="Source"
                  value={selectedShader?.sourceExists ? 'Present' : 'Missing'}
                />
                <DetailRow
                  label="Cache"
                  value={selectedShader?.cacheExists ? 'Present' : 'Missing'}
                />
                <DetailRow
                  label="Bytes"
                  value={String(selectedShader?.byteLength ?? 0)}
                />
                <DetailRow
                  label="Preview uniforms"
                  value={String(shaderUniforms.length)}
                />
                <div className="space-y-2 rounded-md bg-secondary/20 p-2">
                  <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    Declared asset dependencies
                  </div>
                  {selectedShader?.dependencies.length ? (
                    <div className="flex flex-wrap gap-2">
                      {selectedShader.dependencies.map((dependency) => (
                        <Badge key={dependency} variant="outline">
                          {dependency}
                        </Badge>
                      ))}
                    </div>
                  ) : (
                    <div className="text-xs text-muted-foreground">
                      No explicit dependencies declared in `data/scripts/assets/registry.lua`.
                    </div>
                  )}
                </div>
              </section>

              <section className="space-y-2 rounded-md border border-border-subtle p-3 text-sm">
                <div className="font-medium">Timings</div>
                <MetricRow label="Catalog scan" value={perf.catalogLoadMs} unit="ms" />
                <MetricRow label="Shader load" value={perf.shaderLoadMs} unit="ms" />
                <MetricRow label="Disk upload" value={perf.uploadMs} unit="ms" />
                <MetricRow label="WASM validate" value={perf.previewValidateMs} unit="ms" placeholder="Run Validate / Apply" />
                <MetricRow label="Pipeline compile" value={perf.previewPipelineMs} unit="ms" placeholder="Run Validate / Apply" />
                <MetricRow label="Preview apply" value={perf.previewApplyMs} unit="ms" placeholder="Run Validate / Apply" />
                <MetricRow label="Preview frame" value={perf.previewFrameMs} unit="ms" placeholder="Bevy WASM preview pending" />
              </section>

              <section className="space-y-3 rounded-md border border-border-subtle p-3 text-sm">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <div className="font-medium">Preview Status</div>
                    <div className="text-xs text-muted-foreground">
                      {previewBackend}
                    </div>
                  </div>
                  <Badge
                    variant={
                      previewStatus === 'valid'
                        ? 'success'
                        : previewStatus === 'invalid'
                          ? 'destructive'
                          : previewStatus === 'validating'
                            ? 'warning'
                            : 'secondary'
                    }
                  >
                    {previewStatus}
                  </Badge>
                </div>
                <div className="space-y-3">
                  <div className="space-y-2 rounded-md border border-border-subtle p-3">
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="text-muted-foreground">Simulation Speed</span>
                      <span className="font-mono text-xs">
                        {simulationSpeed.toFixed(2)}x
                      </span>
                    </div>
                    <Slider
                      value={[simulationSpeed]}
                      min={0}
                      max={4}
                      step={0.05}
                      onValueChange={(values) =>
                        setSimulationSpeed(values[0] ?? simulationSpeed)
                      }
                    />
                    <div className="text-xs text-muted-foreground">
                      Auto-advances uniforms whose name or component labels include `time`, `age`, `life`, or `progress`.
                    </div>
                  </div>
                  <div className="rounded-md bg-secondary/20 p-2 text-xs text-muted-foreground">
                    Uniform controls are derived from `var&lt;uniform&gt;` declarations in the current WGSL editor contents.
                  </div>
                  {shaderUniforms.length === 0 ? (
                    <div className="text-xs text-muted-foreground">
                      No preview uniforms detected in this shader.
                    </div>
                  ) : (
                    shaderUniforms.map((uniform) => (
                      <UniformControl
                        key={uniform.name}
                        descriptor={uniform}
                        value={uniformValues[uniform.name] ?? uniform.defaults}
                        onChange={(componentIndex, nextValue) => {
                          setUniformValues((prev) => ({
                            ...prev,
                            [uniform.name]: (prev[uniform.name] ?? uniform.defaults).map(
                              (componentValue, index) =>
                                index === componentIndex ? nextValue : componentValue,
                            ),
                          }))
                        }}
                      />
                    ))
                  )}
                </div>
              </section>

              {statusText ? (
                <div className="rounded-md border border-success/30 bg-success/10 px-3 py-2 text-sm text-success-foreground">
                  {statusText}
                </div>
              ) : null}
              {errorText ? (
                <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                  {errorText}
                </div>
              ) : null}
            </div>
          </PanelContent>
        </Panel>
      }
    >
      <input
        ref={fileInputRef}
        type="file"
        accept=".wgsl"
        className="hidden"
        onChange={(event) => {
          void handleDroppedFiles(event.target.files)
          event.currentTarget.value = ''
        }}
      />

      <HorizontalSplitPanels
        leftWidth={editorWidth}
        minLeftWidth={420}
        minRightWidth={360}
        onLeftWidthChange={(width) => {
          setEditorWidth(width)
        }}
        left={
          <section
            className={`
              flex h-full min-h-0 flex-col border-r border-border-subtle bg-background
              ${isDragActive ? 'bg-primary/5' : ''}
            `}
            onDragEnter={(event) => {
              event.preventDefault()
              setIsDragActive(true)
            }}
            onDragOver={(event) => {
              event.preventDefault()
              setIsDragActive(true)
            }}
            onDragLeave={(event) => {
              event.preventDefault()
              if (event.currentTarget.contains(event.relatedTarget as Node | null)) {
                return
              }
              setIsDragActive(false)
            }}
            onDrop={(event) => {
              event.preventDefault()
              setIsDragActive(false)
              void handleDroppedFiles(event.dataTransfer.files)
            }}
          >
            <div className="flex items-center justify-between gap-3 border-b border-border-subtle px-5 py-3">
              <div className="min-w-0">
                <div className="truncate text-sm font-medium text-foreground">
                  {selectedShader?.filename ?? 'WGSL Editor'}
                </div>
                <div className="text-xs text-muted-foreground">
                  {selectedShader?.sourcePath ??
                    'Select a shader from the library or drop a .wgsl file into the editor.'}
                </div>
              </div>
              <div className="flex items-center gap-2">
                {hasUnsavedChanges ? <Badge variant="warning">Modified</Badge> : null}
                <Badge variant="secondary">
                  {isLoadingShader ? 'Loading…' : 'Editor'}
                </Badge>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => fileInputRef.current?.click()}
                      disabled={isUploading}
                      aria-label="Load local shader file"
                    >
                      <FolderUp className="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Load local file</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => void loadCatalog()}
                      disabled={isLoadingCatalog}
                      aria-label="Refresh shader catalog"
                    >
                      <RefreshCw className="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Refresh library</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => {
                        if (!selectedShader) return
                        void handleUploadSource(selectedShader.filename, editorValue)
                      }}
                      disabled={!selectedShader || isUploading || !hasUnsavedChanges}
                      aria-label="Save shader source"
                    >
                      <Save className="h-4 w-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Save</TooltipContent>
                </Tooltip>
              </div>
            </div>
            <ShaderCodeEditor
              value={editorValue}
              onChange={(nextValue) => {
                setEditorValue(nextValue)
                setActivePreviewSource('')
                setPreviewStatus('idle')
              }}
            />
          </section>
        }
        right={
          <VerticalSplitPanels
            topHeight={previewHeight}
            minTopHeight={240}
            minBottomHeight={180}
            onTopHeightChange={(height) => {
              setPreviewHeight(height)
            }}
            top={
              <section className="flex h-full min-h-0 flex-col bg-card">
                <div className="flex items-center justify-between gap-3 border-b border-border-subtle px-5 py-3">
                  <div>
                    <div className="text-sm font-medium text-foreground">
                      Shader Preview
                    </div>
                    <div className="text-xs text-muted-foreground">
                      WebGPU shader preview using the current WGSL and derived uniform inputs.
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary">{previewStatus}</Badge>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          size="icon"
                          variant="outline"
                          onClick={() => {
                            setIsPreviewPaused((prev) => !prev)
                            lastSimulationTickRef.current = null
                          }}
                          disabled={activePreviewSource.length === 0}
                          aria-label={
                            isPreviewPaused
                              ? 'Resume shader preview simulation'
                              : 'Pause shader preview simulation'
                          }
                        >
                          {isPreviewPaused ? (
                            <Play className="h-4 w-4" />
                          ) : (
                            <Pause className="h-4 w-4" />
                          )}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {isPreviewPaused ? 'Resume simulation' : 'Pause simulation'}
                      </TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          size="icon"
                          variant="outline"
                          onClick={() => {
                            resetPreviewSimulation()
                            setIsPreviewPaused(true)
                          }}
                          disabled={activePreviewSource.length === 0}
                          aria-label="Stop shader preview simulation"
                        >
                          <Square className="h-4 w-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Stop and reset simulation</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          size="icon"
                          variant="outline"
                          onClick={() => {
                            setIsPreviewPaused(false)
                            void runPreviewValidation(editorValue, uniformValues)
                          }}
                          disabled={
                            previewStatus === 'validating' ||
                            editorValue.trim().length === 0
                          }
                          aria-label="Validate and apply shader preview"
                        >
                          <FlaskConical className="h-4 w-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Validate / Apply</TooltipContent>
                    </Tooltip>
                  </div>
                </div>
                <div className="flex min-h-0 flex-1 items-center justify-center p-5">
                  <div
                    ref={previewViewportRef}
                    className="flex h-full w-full items-center justify-center"
                  >
                    <canvas
                      ref={previewCanvasRef}
                      className="rounded-lg border border-border-subtle bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.08),_transparent_45%),linear-gradient(180deg,_rgba(17,24,39,0.95),_rgba(5,8,14,1))]"
                      style={{
                        width: `${previewSurfaceSize.width}px`,
                        height: `${previewSurfaceSize.height}px`,
                      }}
                    />
                  </div>
                </div>
              </section>
            }
            bottom={
              <section className="flex h-full min-h-0 flex-col bg-card">
                <div className="border-b border-border-subtle px-5 py-3">
                  <div className="text-sm font-medium text-foreground">
                    Diagnostics
                  </div>
                  <div className="text-xs text-muted-foreground">
                    Shader compile and preview errors appear here.
                  </div>
                </div>
                <ScrollArea className="min-h-0 flex-1">
                  <div className="space-y-3 p-5">
                    {previewDiagnostics.length === 0 ? (
                      <div className="rounded-md border border-border-subtle bg-secondary/20 px-3 py-3 text-xs text-muted-foreground">
                        No diagnostics yet.
                      </div>
                    ) : (
                      previewDiagnostics.map((diagnostic, index) => (
                        <div
                          key={`${diagnostic.type}-${index}`}
                          className="rounded-md border border-border-subtle bg-secondary/20 px-3 py-3"
                        >
                          <div className="flex items-center gap-2">
                            <Badge
                              variant={
                                diagnostic.type === 'error'
                                  ? 'destructive'
                                  : diagnostic.type === 'warning'
                                    ? 'warning'
                                    : 'secondary'
                              }
                            >
                              {diagnostic.type}
                            </Badge>
                            <span className="text-xs text-muted-foreground">
                              {diagnostic.line !== null
                                ? `line ${diagnostic.line}${diagnostic.column !== null ? `:${diagnostic.column}` : ''}`
                                : 'no source location'}
                            </span>
                          </div>
                          <div className="mt-2 whitespace-pre-wrap font-mono text-xs">
                            {diagnostic.message}
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </ScrollArea>
              </section>
            }
          />
        }
      />
    </AppLayout>
  )
}

function MetricRow({
  label,
  value,
  unit,
  placeholder,
}: {
  label: string
  value: number | null
  unit: string
  placeholder?: string
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span>
        {value === null ? (
          <span className="text-xs text-muted-foreground">
            {placeholder ?? 'n/a'}
          </span>
        ) : (
          `${value.toFixed(2)} ${unit}`
        )}
      </span>
    </div>
  )
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right">{value}</span>
    </div>
  )
}

function inferComponentRange(
  descriptor: ShaderPreviewUniformDescriptor,
  label: string,
): { min: number; max: number; step: number } {
  const lower = `${descriptor.name} ${label}`.toLowerCase()
  if (descriptor.category === 'color') {
    return { min: 0, max: 1, step: 0.01 }
  }
  if (lower.includes('alpha') || lower.includes('age') || lower.includes('life')) {
    return { min: 0, max: 1, step: 0.01 }
  }
  if (lower.includes('intensity') || lower.includes('time')) {
    return { min: 0, max: 2, step: 0.01 }
  }
  if (lower.includes('density')) {
    return { min: 0, max: 8, step: 0.05 }
  }
  return { min: -2, max: 2, step: 0.01 }
}

function clampColorChannel(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function toHexChannel(value: number): string {
  return Math.round(clampColorChannel(value) * 255)
    .toString(16)
    .padStart(2, '0')
}

function colorValueToHex(value: Array<number>): string {
  const red = value[0] ?? 0
  const green = value[1] ?? 0
  const blue = value[2] ?? 0
  return `#${toHexChannel(red)}${toHexChannel(green)}${toHexChannel(blue)}`
}

function parseHexColor(value: string): [number, number, number] | null {
  const match = value.match(/^#?([a-f0-9]{6})$/i)
  if (!match) {
    return null
  }

  const hex = match[1]
  return [
    parseInt(hex.slice(0, 2), 16) / 255,
    parseInt(hex.slice(2, 4), 16) / 255,
    parseInt(hex.slice(4, 6), 16) / 255,
  ]
}

function abbreviateColorLabel(label: string): string {
  const lower = label.toLowerCase()
  if (lower === 'red') return 'R'
  if (lower === 'green') return 'G'
  if (lower === 'blue') return 'B'
  if (lower === 'alpha') return 'A'
  return label
}

function UniformControl({
  descriptor,
  value,
  onChange,
}: {
  descriptor: ShaderPreviewUniformDescriptor
  value: Array<number>
  onChange: (componentIndex: number, nextValue: number) => void
}) {
  if (descriptor.category === 'color') {
    const colorHex = colorValueToHex(value)

    return (
      <div className="space-y-3 rounded-md border border-border-subtle p-3">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="font-medium text-foreground">{descriptor.name}</div>
            <div className="text-xs text-muted-foreground">
              binding {descriptor.binding} • {descriptor.type}
            </div>
          </div>
          <Badge variant="secondary">{descriptor.category}</Badge>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            type="color"
            value={colorHex}
            onChange={(event) => {
              const parsed = parseHexColor(event.target.value)
              if (!parsed) {
                return
              }
              onChange(0, parsed[0])
              onChange(1, parsed[1])
              onChange(2, parsed[2])
            }}
            className="h-9 w-12 cursor-pointer rounded-md p-1"
            aria-label={`${descriptor.name} color picker`}
          />
          {descriptor.labels.map((label, componentIndex) => {
            const currentValue = value[componentIndex] ?? 0
            return (
              <div
                key={`${descriptor.name}-${label}`}
                className="flex items-center gap-2"
              >
                <span className="text-xs text-muted-foreground">
                  {abbreviateColorLabel(label)}
                </span>
                <Input
                  value={currentValue.toFixed(2)}
                  onChange={(event) => {
                    const parsed = Number(event.target.value)
                    if (!Number.isFinite(parsed)) {
                      return
                    }
                    onChange(componentIndex, clampColorChannel(parsed))
                  }}
                  className="h-8 w-16 text-right font-mono text-xs"
                  inputMode="decimal"
                />
              </div>
            )
          })}
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-3 rounded-md border border-border-subtle p-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="font-medium text-foreground">{descriptor.name}</div>
          <div className="text-xs text-muted-foreground">
            binding {descriptor.binding} • {descriptor.type}
          </div>
        </div>
        <Badge variant="secondary">{descriptor.category}</Badge>
      </div>
      {descriptor.labels.map((label, componentIndex) => {
        const range = inferComponentRange(descriptor, label)
        const currentValue = value[componentIndex] ?? 0

        return (
          <div key={`${descriptor.name}-${label}`} className="space-y-2">
            <div className="flex items-center justify-between gap-3 text-sm">
              <span className="text-muted-foreground">{label}</span>
              <Input
                value={currentValue.toFixed(2)}
                onChange={(event) => {
                  const parsed = Number(event.target.value)
                  if (!Number.isFinite(parsed)) {
                    return
                  }
                  onChange(componentIndex, parsed)
                }}
                className="h-7 w-20 text-right font-mono text-xs"
                inputMode="decimal"
              />
            </div>
            <Slider
              value={[currentValue]}
              min={range.min}
              max={range.max}
              step={range.step}
              onValueChange={(values) => onChange(componentIndex, values[0] ?? currentValue)}
            />
          </div>
        )
      })}
    </div>
  )
}
