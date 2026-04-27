import { Pause, Play, RotateCcw } from 'lucide-react'
import * as React from 'react'
import { buildGenesisPlanetPreviewUniforms } from './planet-preview'
import type { GenesisPlanetDefinition } from './types'
import type { ShaderPreviewDiagnostic } from '@/lib/shader-preview'
import { renderPreviewShaderSequence } from '@/lib/shader-preview'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { apiGet } from '@/lib/api/client'

type ShaderCatalogEntry = {
  shaderId: string
  assetId: string | null
}

type ShaderCatalogResponse = {
  shaders: Array<ShaderCatalogEntry>
}

type ShaderFileResponse = {
  source: string
}

type PreviewStatus = 'loading' | 'ready' | 'rendering' | 'valid' | 'invalid'

const MIN_PREVIEW_SIZE = 240
const MAX_PREVIEW_SIZE = 360

export function GenesisPlanetPreview({
  definition,
}: {
  definition: GenesisPlanetDefinition
}) {
  const canvasRef = React.useRef<HTMLCanvasElement>(null)
  const viewportRef = React.useRef<HTMLDivElement>(null)
  const renderIdRef = React.useRef(0)
  const animationFrameRef = React.useRef<number | null>(null)
  const startedAtRef = React.useRef(performance.now())
  const [shaderSource, setShaderSource] = React.useState('')
  const [status, setStatus] = React.useState<PreviewStatus>('loading')
  const [errorText, setErrorText] = React.useState<string | null>(null)
  const [diagnostics, setDiagnostics] = React.useState<
    Array<ShaderPreviewDiagnostic>
  >([])
  const [paused, setPaused] = React.useState(false)
  const [surfaceSize, setSurfaceSize] = React.useState({
    width: 360,
    height: 360,
  })

  React.useEffect(() => {
    const viewport = viewportRef.current
    if (!viewport) return

    const resize = () => {
      const rect = viewport.getBoundingClientRect()
      const cssSize = Math.max(
        MIN_PREVIEW_SIZE,
        Math.min(MAX_PREVIEW_SIZE, Math.floor(rect.width)),
      )
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      const canvas = canvasRef.current
      if (canvas) {
        canvas.width = Math.floor(cssSize * dpr)
        canvas.height = Math.floor(cssSize * dpr)
      }
      setSurfaceSize({ width: cssSize, height: cssSize })
    }

    resize()
    const observer = new ResizeObserver(resize)
    observer.observe(viewport)
    return () => observer.disconnect()
  }, [])

  React.useEffect(() => {
    let cancelled = false
    setStatus('loading')
    setErrorText(null)
    setDiagnostics([])

    async function loadShader() {
      const catalog = await apiGet<ShaderCatalogResponse>('/api/shaders')
      const shader = catalog.shaders.find(
        (entry) =>
          entry.assetId === definition.spawn.planet_visual_shader_asset_id,
      )
      if (!shader) {
        throw new Error(
          `Shader asset ${definition.spawn.planet_visual_shader_asset_id} is not registered in the dashboard shader catalog.`,
        )
      }
      const file = await apiGet<ShaderFileResponse>(
        `/api/shaders/${encodeURIComponent(shader.shaderId)}`,
      )
      if (!cancelled) {
        setShaderSource(file.source)
        setStatus('ready')
      }
    }

    loadShader().catch((error: unknown) => {
      if (cancelled) return
      setStatus('invalid')
      setErrorText(
        error instanceof Error
          ? error.message
          : 'Failed to load preview shader.',
      )
    })

    return () => {
      cancelled = true
    }
  }, [definition.spawn.planet_visual_shader_asset_id])

  const renderPreview = React.useCallback(
    async (timeSeconds: number) => {
      const canvas = canvasRef.current
      if (!canvas || shaderSource.length === 0) return
      const renderId = ++renderIdRef.current
      setStatus((current) => (current === 'loading' ? current : 'rendering'))
      try {
        const passSequence =
          definition.shader_settings.body_kind === 0
            ? [
                [0, 1, 0, 0],
                [1, 0, 0, 0],
                [0, 0, 0, 0],
                [2, 0, 0, 0],
                [0, 2, 0, 0],
              ]
            : [
                [0, 1, 0, 0],
                [0, 0, 0, 0],
                [0, 2, 0, 0],
              ]
        const result = await renderPreviewShaderSequence(
          canvas,
          shaderSource,
          passSequence.map((passFlags, index) => ({
            values: buildGenesisPlanetPreviewUniforms(
              definition.shader_settings,
              paused ? 0 : timeSeconds,
              passFlags,
            ),
            clear: index === 0,
          })),
        )
        if (renderId !== renderIdRef.current) return
        setDiagnostics(result.diagnostics)
        setStatus(result.ok ? 'valid' : 'invalid')
        setErrorText(null)
      } catch (error) {
        if (renderId !== renderIdRef.current) return
        setDiagnostics([])
        setStatus('invalid')
        setErrorText(
          error instanceof Error
            ? error.message
            : 'Failed to render Genesis preview.',
        )
      }
    },
    [definition.shader_settings, paused, shaderSource],
  )

  React.useEffect(() => {
    if (shaderSource.length === 0) return

    if (paused) {
      void renderPreview(0)
      return
    }

    let cancelled = false
    const tick = () => {
      if (cancelled) return
      const timeSeconds = (performance.now() - startedAtRef.current) / 1000
      void renderPreview(timeSeconds)
      animationFrameRef.current = window.setTimeout(
        tick,
        300,
      ) as unknown as number
    }
    tick()
    return () => {
      cancelled = true
      if (animationFrameRef.current !== null) {
        window.clearTimeout(animationFrameRef.current)
      }
    }
  }, [paused, renderPreview, shaderSource, surfaceSize])

  return (
    <section className="border border-border bg-background/40">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-3">
        <div>
          <div className="text-[11px] uppercase tracking-[0.22em] text-primary/90">
            Visual Preview
          </div>
          <div className="text-xs text-muted-foreground">
            {definition.display_name} /{' '}
            {definition.spawn.planet_visual_shader_asset_id}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Badge
            variant={
              status === 'valid'
                ? 'success'
                : status === 'invalid'
                  ? 'destructive'
                  : 'secondary'
            }
          >
            {status}
          </Badge>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon"
                disabled={shaderSource.length === 0}
                onClick={() => setPaused((current) => !current)}
                aria-label={
                  paused ? 'Resume Genesis preview' : 'Pause Genesis preview'
                }
              >
                {paused ? (
                  <Play className="h-4 w-4" />
                ) : (
                  <Pause className="h-4 w-4" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{paused ? 'Resume' : 'Pause'}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon"
                disabled={shaderSource.length === 0}
                onClick={() => {
                  startedAtRef.current = performance.now()
                  void renderPreview(0)
                }}
                aria-label="Reset Genesis preview time"
              >
                <RotateCcw className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Reset time</TooltipContent>
          </Tooltip>
        </div>
      </div>
      <div className="p-4">
        <div
          ref={viewportRef}
          className="flex min-h-[240px] w-full items-center justify-center bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.08),_transparent_45%),linear-gradient(180deg,_rgba(17,24,39,0.95),_rgba(5,8,14,1))]"
        >
          <canvas
            ref={canvasRef}
            className="border border-border-subtle bg-black"
            style={{
              width: `${surfaceSize.width}px`,
              height: `${surfaceSize.height}px`,
            }}
          />
        </div>
        {errorText ? (
          <div className="mt-3 border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {errorText}
          </div>
        ) : diagnostics.length > 0 ? (
          <div className="mt-3 border border-border bg-secondary/20 px-3 py-2 text-xs text-muted-foreground">
            {diagnostics[0].type}: {diagnostics[0].message}
          </div>
        ) : null}
      </div>
    </section>
  )
}
