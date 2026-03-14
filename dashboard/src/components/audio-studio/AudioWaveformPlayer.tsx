import * as React from 'react'
import {
  LoaderCircle,
  Pause,
  Play,
  Repeat2,
  Square,
  Waves,
} from 'lucide-react'
import type {
  AudioStudioCueEntry,
  AudioStudioMarkerKey,
  AudioStudioMarkers,
} from '@/features/audio-studio/types'
import { audioStudioMarkerKeys } from '@/features/audio-studio/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ButtonGroup } from '@/components/ui/button-group'
import { HUDFrame } from '@/components/ui/hud-frame'
import { TheGridSlider } from '@/components/thegridcn/thegrid-slider'

interface AudioWaveformPlayerProps {
  entry: AudioStudioCueEntry | null
  markerValues: AudioStudioMarkers
  draftMarkerValues: AudioStudioMarkers
  onDraftMarkerValuesChange?: (markers: AudioStudioMarkers) => void
  onPlayheadChange?: (seconds: number) => void
}

type LoadedAudioState = {
  objectUrl: string
  duration: number
  channelCount: number
  buffer: AudioBuffer
}

const MIN_WAVEFORM_ZOOM = 1
const MAX_WAVEFORM_ZOOM = 24
const WAVEFORM_ZOOM_FACTOR = 1.2
const MARKER_DRAG_EPSILON = 0.0001

type DragState =
  | { kind: 'playhead' }
  | { kind: 'marker'; markerKey: AudioStudioMarkerKey }

type PlaybackProgram =
  | { mode: 'idle' }
  | { mode: 'normal'; clipEnd: number | null }
  | {
      mode: 'hold'
      loopStart: number
      loopEnd: number
      outroStart: number | null
      clipEnd: number | null
      releaseRequested: boolean
    }
  | { mode: 'outro'; clipEnd: number | null }

function formatSeconds(value: number): string {
  if (!Number.isFinite(value) || value < 0) {
    return '0:00.00'
  }
  const minutes = Math.floor(value / 60)
  const seconds = value - minutes * 60
  return `${minutes}:${seconds.toFixed(2).padStart(5, '0')}`
}

function drawWaveform(
  canvas: HTMLCanvasElement,
  size: { width: number; height: number },
  buffer: AudioBuffer,
  currentTime: number,
  markers: AudioStudioMarkers,
): void {
  const dpr = window.devicePixelRatio || 1
  canvas.width = Math.max(1, Math.floor(size.width * dpr))
  canvas.height = Math.max(1, Math.floor(size.height * dpr))
  canvas.style.width = `${size.width}px`
  canvas.style.height = `${size.height}px`

  const context = canvas.getContext('2d')
  if (!context) {
    return
  }

  const styles = getComputedStyle(canvas)
  const primary = styles.getPropertyValue('--color-primary').trim() || '#15f0ff'
  const accent = styles.getPropertyValue('--color-accent').trim() || '#ff5db1'
  const muted = styles.getPropertyValue('--color-muted-foreground').trim() || '#8992a0'
  const border = styles.getPropertyValue('--color-border').trim() || '#1f2c3c'
  const background = styles.getPropertyValue('--color-card').trim() || '#050a12'

  context.setTransform(dpr, 0, 0, dpr, 0, 0)
  context.clearRect(0, 0, size.width, size.height)
  context.fillStyle = background
  context.fillRect(0, 0, size.width, size.height)

  const channelCount = Math.min(2, Math.max(1, buffer.numberOfChannels))
  const duration = buffer.duration || 1
  const laneHeight = size.height / channelCount
  const maxAmplitude = laneHeight * 0.34

  context.strokeStyle = border
  context.lineWidth = 1
  for (let lane = 0; lane < channelCount; lane += 1) {
    const centerY = lane * laneHeight + laneHeight / 2
    context.beginPath()
    context.moveTo(0, centerY + 0.5)
    context.lineTo(size.width, centerY + 0.5)
    context.stroke()
  }

  for (let lane = 0; lane < channelCount; lane += 1) {
    const samples = buffer.getChannelData(Math.min(lane, buffer.numberOfChannels - 1))
    const centerY = lane * laneHeight + laneHeight / 2
    context.beginPath()
    context.strokeStyle = lane === 0 ? primary : accent
    context.lineWidth = 1

    for (let pixel = 0; pixel < size.width; pixel += 1) {
      const rangeStart = Math.floor((pixel / size.width) * samples.length)
      const rangeEnd = Math.max(
        rangeStart + 1,
        Math.floor(((pixel + 1) / size.width) * samples.length),
      )
      let min = 1
      let max = -1
      for (let index = rangeStart; index < rangeEnd; index += 1) {
        const sample = samples[index] ?? 0
        if (sample < min) {
          min = sample
        }
        if (sample > max) {
          max = sample
        }
      }
      context.moveTo(pixel + 0.5, centerY + min * maxAmplitude)
      context.lineTo(pixel + 0.5, centerY + max * maxAmplitude)
    }

    context.stroke()
  }

  const markerColors: Record<keyof AudioStudioMarkers, string> = {
    intro_start_s: primary,
    loop_start_s: accent,
    loop_end_s: '#f6c85f',
    outro_start_s: '#ff7f50',
    clip_end_s: muted,
  }

  for (const [key, rawValue] of Object.entries(markers) as Array<
    [keyof AudioStudioMarkers, number | null]
  >) {
    if (rawValue === null || !Number.isFinite(rawValue)) {
      continue
    }
    const x = Math.max(0, Math.min(size.width, (rawValue / duration) * size.width))
    context.strokeStyle = markerColors[key]
    context.lineWidth = 1
    context.beginPath()
    context.moveTo(x + 0.5, 0)
    context.lineTo(x + 0.5, size.height)
    context.stroke()
  }

  const playheadX = Math.max(
    0,
    Math.min(size.width, (Math.max(0, currentTime) / duration) * size.width),
  )
  context.strokeStyle = '#ffffff'
  context.lineWidth = 1.25
  context.beginPath()
  context.moveTo(playheadX + 0.5, 0)
  context.lineTo(playheadX + 0.5, size.height)
  context.stroke()
}

function resolvePlaybackMarkersFromValues(markers: AudioStudioMarkers | null): {
  introStart: number
  loopStart: number | null
  loopEnd: number | null
  outroStart: number | null
  clipEnd: number | null
} {
  return {
    introStart: markers?.intro_start_s ?? 0,
    loopStart: markers?.loop_start_s ?? null,
    loopEnd: markers?.loop_end_s ?? null,
    outroStart: markers?.outro_start_s ?? null,
    clipEnd: markers?.clip_end_s ?? null,
  }
}

function hasValidLoopWindow(markers: AudioStudioMarkers): boolean {
  const { loopStart, loopEnd } = resolvePlaybackMarkersFromValues(markers)
  return (
    loopStart !== null &&
    loopEnd !== null &&
    Number.isFinite(loopStart) &&
    Number.isFinite(loopEnd) &&
    loopEnd > loopStart
  )
}

function markerColorClass(markerKey: AudioStudioMarkerKey): string {
  switch (markerKey) {
    case 'intro_start_s':
      return 'text-primary'
    case 'loop_start_s':
      return 'text-accent'
    case 'loop_end_s':
      return 'text-warning'
    case 'outro_start_s':
      return 'text-orange-400'
    case 'clip_end_s':
      return 'text-muted-foreground'
  }
}

function markerLabel(markerKey: AudioStudioMarkerKey): string {
  switch (markerKey) {
    case 'intro_start_s':
      return 'INTRO'
    case 'loop_start_s':
      return 'LOOP IN'
    case 'loop_end_s':
      return 'LOOP OUT'
    case 'outro_start_s':
      return 'OUTRO'
    case 'clip_end_s':
      return 'END'
  }
}

function clampMarkerValue(
  markerKey: AudioStudioMarkerKey,
  nextValue: number,
  markers: AudioStudioMarkers,
  duration: number,
): number {
  let min = 0
  let max = duration

  switch (markerKey) {
    case 'intro_start_s':
      max =
        markers.loop_start_s ??
        markers.loop_end_s ??
        markers.outro_start_s ??
        markers.clip_end_s ??
        duration
      break
    case 'loop_start_s':
      min = markers.intro_start_s ?? 0
      max =
        (markers.loop_end_s ??
          markers.outro_start_s ??
          markers.clip_end_s ??
          duration) - MARKER_DRAG_EPSILON
      break
    case 'loop_end_s':
      min =
        (markers.loop_start_s ?? markers.intro_start_s ?? 0) +
        MARKER_DRAG_EPSILON
      max = markers.outro_start_s ?? markers.clip_end_s ?? duration
      break
    case 'outro_start_s':
      min = markers.loop_end_s ?? markers.loop_start_s ?? markers.intro_start_s ?? 0
      max = markers.clip_end_s ?? duration
      break
    case 'clip_end_s':
      min =
        markers.outro_start_s ??
        markers.loop_end_s ??
        markers.loop_start_s ??
        markers.intro_start_s ??
        0
      break
  }

  if (!Number.isFinite(min)) {
    min = 0
  }
  if (!Number.isFinite(max)) {
    max = duration
  }
  if (max < min) {
    max = min
  }
  return Math.max(min, Math.min(max, nextValue))
}

export function AudioWaveformPlayer({
  entry,
  markerValues,
  draftMarkerValues,
  onDraftMarkerValuesChange,
  onPlayheadChange,
}: AudioWaveformPlayerProps) {
  const audioRef = React.useRef<HTMLAudioElement>(null)
  const canvasRef = React.useRef<HTMLCanvasElement>(null)
  const viewportRef = React.useRef<HTMLDivElement>(null)
  const animationFrameRef = React.useRef<number | null>(null)
  const objectUrlRef = React.useRef<string | null>(null)
  const playbackProgramRef = React.useRef<PlaybackProgram>({ mode: 'idle' })
  const holdLoopActiveRef = React.useRef(false)
  const dragStateRef = React.useRef<DragState | null>(null)
  const zoomAnchorRef = React.useRef<{
    absoluteRatio: number
    offsetX: number
  } | null>(null)
  const [loadedAudio, setLoadedAudio] = React.useState<LoadedAudioState | null>(null)
  const [isLoading, setIsLoading] = React.useState(false)
  const [loadError, setLoadError] = React.useState<string | null>(null)
  const [isPlaying, setIsPlaying] = React.useState(false)
  const [currentTime, setCurrentTime] = React.useState(0)
  const [viewportSize, setViewportSize] = React.useState({ width: 0, height: 0 })
  const [waveformZoom, setWaveformZoom] = React.useState(1)

  const waveformWidth = React.useMemo(() => {
    if (viewportSize.width <= 0) {
      return 0
    }
    return Math.max(
      viewportSize.width,
      Math.round(viewportSize.width * waveformZoom),
    )
  }, [viewportSize.width, waveformZoom])

  const stopPlayback = React.useCallback(
    (nextTime = 0) => {
      const audio = audioRef.current
      playbackProgramRef.current = { mode: 'idle' }
      holdLoopActiveRef.current = false
      if (!audio) {
        return
      }
      audio.pause()
      audio.currentTime = nextTime
      setCurrentTime(nextTime)
      onPlayheadChange?.(nextTime)
    },
    [onPlayheadChange],
  )

  const stepPlaybackProgram = React.useCallback(
    (audio: HTMLAudioElement) => {
      const program = playbackProgramRef.current
      const now = audio.currentTime

      if (program.mode === 'idle') {
        return
      }

      if (program.mode === 'normal') {
        if (program.clipEnd !== null && now >= program.clipEnd) {
          stopPlayback(program.clipEnd)
        }
        return
      }

      if (program.mode === 'hold') {
        if (program.releaseRequested) {
          if (program.outroStart !== null && now >= program.loopStart) {
            playbackProgramRef.current = {
              mode: 'outro',
              clipEnd: program.clipEnd,
            }
            audio.currentTime = program.outroStart
            return
          }
          if (program.outroStart === null && program.clipEnd !== null && now >= program.clipEnd) {
            stopPlayback(program.clipEnd)
          }
          return
        }

        if (now >= program.loopEnd) {
          audio.currentTime = program.loopStart
        }
        return
      }

      if (program.clipEnd !== null && now >= program.clipEnd) {
        stopPlayback(program.clipEnd)
      }
    },
    [stopPlayback],
  )

  const startStandardPlayback = React.useCallback(() => {
    const audio = audioRef.current
    const markers = resolvePlaybackMarkersFromValues(markerValues)
    if (!audio) {
      return
    }
    playbackProgramRef.current = {
      mode: 'normal',
      clipEnd: markers.clipEnd,
    }
    audio.currentTime = markers.introStart
    void audio.play().catch(() => {})
  }, [markerValues])

  const startHoldLoopPlayback = React.useCallback(() => {
    const audio = audioRef.current
    const markers = resolvePlaybackMarkersFromValues(markerValues)
    if (
      !audio ||
      markers.loopStart === null ||
      markers.loopEnd === null ||
      markers.loopEnd <= markers.loopStart
    ) {
      return
    }

    holdLoopActiveRef.current = true
    playbackProgramRef.current = {
      mode: 'hold',
      loopStart: markers.loopStart,
      loopEnd: markers.loopEnd,
      outroStart: markers.outroStart,
      clipEnd: markers.clipEnd,
      releaseRequested: false,
    }
    audio.currentTime = markers.introStart
    void audio.play().catch(() => {})
  }, [markerValues])

  const releaseHoldLoopPlayback = React.useCallback(() => {
    const program = playbackProgramRef.current
    if (program.mode !== 'hold') {
      holdLoopActiveRef.current = false
      return
    }
    holdLoopActiveRef.current = false
    playbackProgramRef.current = {
      ...program,
      releaseRequested: true,
    }
  }, [])

  React.useEffect(() => {
    if (!viewportRef.current || typeof ResizeObserver === 'undefined') {
      return
    }
    const observer = new ResizeObserver((entries) => {
      const target = entries[0]
      setViewportSize({
        width: Math.floor(target.contentRect.width),
        height: Math.floor(target.contentRect.height),
      })
    })
    observer.observe(viewportRef.current)
    return () => observer.disconnect()
  }, [])

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current
    const anchor = zoomAnchorRef.current
    if (!viewport || !anchor || waveformWidth <= 0) {
      return
    }
    const maxScrollLeft = Math.max(0, waveformWidth - viewport.clientWidth)
    viewport.scrollLeft = Math.max(
      0,
      Math.min(maxScrollLeft, anchor.absoluteRatio * waveformWidth - anchor.offsetX),
    )
    zoomAnchorRef.current = null
  }, [waveformWidth])

  React.useEffect(() => {
    const audio = audioRef.current
    if (!audio) {
      return
    }
    const syncTime = () => {
      const nextTime = audio.currentTime
      setCurrentTime(nextTime)
      onPlayheadChange?.(nextTime)
    }
    const handlePlay = () => setIsPlaying(true)
    const handlePause = () => {
      setIsPlaying(false)
      syncTime()
    }
    const handleEnded = () => {
      playbackProgramRef.current = { mode: 'idle' }
      holdLoopActiveRef.current = false
      setIsPlaying(false)
      syncTime()
    }

    audio.addEventListener('loadedmetadata', syncTime)
    audio.addEventListener('timeupdate', syncTime)
    audio.addEventListener('play', handlePlay)
    audio.addEventListener('pause', handlePause)
    audio.addEventListener('ended', handleEnded)
    return () => {
      audio.removeEventListener('loadedmetadata', syncTime)
      audio.removeEventListener('timeupdate', syncTime)
      audio.removeEventListener('play', handlePlay)
      audio.removeEventListener('pause', handlePause)
      audio.removeEventListener('ended', handleEnded)
    }
  }, [onPlayheadChange])

  React.useEffect(() => {
    if (!isPlaying) {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current)
        animationFrameRef.current = null
      }
      return
    }

    const tick = () => {
      const audio = audioRef.current
      if (audio) {
        stepPlaybackProgram(audio)
        const nextTime = audio.currentTime
        setCurrentTime(nextTime)
        onPlayheadChange?.(nextTime)
      }
      animationFrameRef.current = requestAnimationFrame(tick)
    }
    animationFrameRef.current = requestAnimationFrame(tick)
    return () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current)
        animationFrameRef.current = null
      }
    }
  }, [isPlaying, onPlayheadChange, stepPlaybackProgram])

  React.useEffect(() => {
    objectUrlRef.current = loadedAudio?.objectUrl ?? null
  }, [loadedAudio?.objectUrl])

  React.useEffect(() => {
    setWaveformZoom(1)
    zoomAnchorRef.current = null
    viewportRef.current?.scrollTo({ left: 0 })
  }, [entry?.soundId])

  React.useEffect(() => {
    const audio = audioRef.current
    if (audio) {
      stopPlayback(0)
    }
    setCurrentTime(0)
    setIsPlaying(false)
    setLoadError(null)

    if (!entry) {
      setIsLoading(false)
      setLoadedAudio((previous) => {
        if (previous) {
          URL.revokeObjectURL(previous.objectUrl)
        }
        return null
      })
      return
    }

    const abortController = new AbortController()
    setIsLoading(true)

    void (async () => {
      try {
        const response = await fetch(
          `/api/audio-cues/${encodeURIComponent(entry.soundId)}`,
          {
            signal: abortController.signal,
          },
        )
        if (!response.ok) {
          const payload = (await response.json().catch(() => ({}))) as {
            error?: string
          }
          throw new Error(payload.error ?? 'Failed to load audio preview')
        }

        const contentType =
          response.headers.get('content-type') ??
          entry.asset?.contentType ??
          'application/octet-stream'
        const bytes = await response.arrayBuffer()
        const previewBytes = bytes.slice(0)
        const blob = new Blob([bytes], { type: contentType })
        const objectUrl = URL.createObjectURL(blob)
        const audioContext = new window.AudioContext()
        try {
          const buffer = await audioContext.decodeAudioData(previewBytes)
          if (abortController.signal.aborted) {
            URL.revokeObjectURL(objectUrl)
            return
          }
          setLoadedAudio((previous) => {
            if (previous) {
              URL.revokeObjectURL(previous.objectUrl)
            }
            return {
              objectUrl,
              duration: buffer.duration,
              channelCount: buffer.numberOfChannels,
              buffer,
            }
          })
        } finally {
          void audioContext.close().catch(() => {})
        }
      } catch (error) {
        if (abortController.signal.aborted) {
          return
        }
        setLoadedAudio((previous) => {
          if (previous) {
            URL.revokeObjectURL(previous.objectUrl)
          }
          return null
        })
        setLoadError(
          error instanceof Error ? error.message : 'Failed to load audio preview',
        )
      } finally {
        if (!abortController.signal.aborted) {
          setIsLoading(false)
        }
      }
    })()

    return () => {
      abortController.abort()
    }
  }, [entry, stopPlayback])

  React.useEffect(() => {
    if (!holdLoopActiveRef.current) {
      return
    }

    const handlePointerUp = () => {
      releaseHoldLoopPlayback()
    }

    window.addEventListener('pointerup', handlePointerUp)
    return () => {
      window.removeEventListener('pointerup', handlePointerUp)
    }
  }, [isPlaying, releaseHoldLoopPlayback])

  React.useEffect(() => {
    if (
      !canvasRef.current ||
      !loadedAudio?.buffer ||
      waveformWidth <= 0 ||
      viewportSize.height <= 0
    ) {
      return
    }
    drawWaveform(
      canvasRef.current,
      {
        width: waveformWidth,
        height: viewportSize.height,
      },
      loadedAudio.buffer,
      currentTime,
      markerValues,
    )
  }, [
    currentTime,
    loadedAudio?.buffer,
    markerValues,
    viewportSize.height,
    waveformWidth,
  ])

  React.useEffect(
    () => () => {
      if (objectUrlRef.current) {
        URL.revokeObjectURL(objectUrlRef.current)
      }
    },
    [],
  )

  const canPreview = Boolean(entry && loadedAudio)
  const canLoopPreview = canPreview && hasValidLoopWindow(markerValues)
  const visibleDurationSeconds =
    loadedAudio && waveformWidth > 0 && viewportSize.width > 0
      ? loadedAudio.duration * (viewportSize.width / waveformWidth)
      : 0

  const seekToClientPosition = React.useCallback(
    (clientX: number) => {
      const audio = audioRef.current
      const viewport = viewportRef.current
      if (!audio || !loadedAudio || !viewport || waveformWidth <= 0) {
        return
      }
      const rect = viewport.getBoundingClientRect()
      const offsetX = Math.max(0, Math.min(rect.width, clientX - rect.left))
      const absoluteX = viewport.scrollLeft + offsetX
      const ratio = Math.max(0, Math.min(1, absoluteX / waveformWidth))
      const nextTime = ratio * loadedAudio.duration
      audio.currentTime = nextTime
      setCurrentTime(nextTime)
      onPlayheadChange?.(nextTime)
    },
    [loadedAudio, onPlayheadChange, waveformWidth],
  )

  const updateMarkerAtClientPosition = React.useCallback(
    (markerKey: AudioStudioMarkerKey, clientX: number) => {
      const viewport = viewportRef.current
      if (!loadedAudio || !viewport || waveformWidth <= 0) {
        return
      }
      const rect = viewport.getBoundingClientRect()
      const offsetX = Math.max(0, Math.min(rect.width, clientX - rect.left))
      const absoluteX = viewport.scrollLeft + offsetX
      const ratio = Math.max(0, Math.min(1, absoluteX / waveformWidth))
      const unclampedValue = ratio * loadedAudio.duration
      const nextValue = clampMarkerValue(
        markerKey,
        unclampedValue,
        markerValues,
        loadedAudio.duration,
      )
      onDraftMarkerValuesChange?.({
        ...draftMarkerValues,
        [markerKey]: nextValue,
      })
    },
    [
      draftMarkerValues,
      loadedAudio,
      markerValues,
      onDraftMarkerValuesChange,
      waveformWidth,
    ],
  )

  React.useEffect(() => {
    const handlePointerMove = (event: PointerEvent) => {
      const dragState = dragStateRef.current
      if (!dragState) {
        return
      }
      if (dragState.kind === 'playhead') {
        seekToClientPosition(event.clientX)
        return
      }
      updateMarkerAtClientPosition(dragState.markerKey, event.clientX)
    }

    const handlePointerUp = () => {
      dragStateRef.current = null
    }

    window.addEventListener('pointermove', handlePointerMove)
    window.addEventListener('pointerup', handlePointerUp)
    window.addEventListener('pointercancel', handlePointerUp)
    return () => {
      window.removeEventListener('pointermove', handlePointerMove)
      window.removeEventListener('pointerup', handlePointerUp)
      window.removeEventListener('pointercancel', handlePointerUp)
    }
  }, [seekToClientPosition, updateMarkerAtClientPosition])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-3">
      <HUDFrame label="Waveform" className="flex min-h-0 flex-1 flex-col">
        <div className="flex items-center gap-2 border-b border-border/60 px-4 py-3">
          <div className="min-w-0">
            <div className="truncate font-display text-lg uppercase tracking-[0.22em] text-primary">
              {entry?.displayName ?? 'No Cue Selected'}
            </div>
            <div className="truncate text-xs uppercase tracking-[0.18em] text-muted-foreground">
              {entry?.asset?.sourcePath ?? 'Select a cue from the audio tree'}
            </div>
          </div>
          <div className="ml-auto flex items-center gap-2">
            {entry?.playbackKind ? (
              <Badge variant="outline">{entry.playbackKind}</Badge>
            ) : null}
            {loadedAudio ? (
              <Badge variant="secondary">
                {loadedAudio.channelCount >= 2 ? 'Stereo' : 'Mono'}
              </Badge>
            ) : null}
          </div>
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3 px-4 py-4">
          <div
            className="relative min-h-[280px] min-w-0 w-full max-w-full flex-1 overflow-hidden border border-border/70 bg-black/45"
          >
            <div
              ref={viewportRef}
              className="absolute inset-0 overflow-x-auto overflow-y-hidden"
              onWheel={(event) => {
                if (!loadedAudio || viewportSize.width <= 0 || waveformWidth <= 0) {
                  return
                }
                event.preventDefault()
                const viewport = viewportRef.current
                if (!viewport) {
                  return
                }
                const rect = viewport.getBoundingClientRect()
                const offsetX = Math.max(0, Math.min(rect.width, event.clientX - rect.left))
                zoomAnchorRef.current = {
                  absoluteRatio:
                    (viewport.scrollLeft + offsetX) / Math.max(1, waveformWidth),
                  offsetX,
                }
                setWaveformZoom((previous) => {
                  const next =
                    event.deltaY < 0
                      ? previous * WAVEFORM_ZOOM_FACTOR
                      : previous / WAVEFORM_ZOOM_FACTOR
                  return Math.max(
                    MIN_WAVEFORM_ZOOM,
                    Math.min(MAX_WAVEFORM_ZOOM, next),
                  )
                })
              }}
            >
              <div
                className="relative h-full min-w-full"
                style={{ width: waveformWidth > 0 ? `${waveformWidth}px` : '100%' }}
                onPointerDown={(event) => {
                  if (event.button !== 0 || !canPreview) {
                    return
                  }
                  dragStateRef.current = { kind: 'playhead' }
                  seekToClientPosition(event.clientX)
                }}
              >
                <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" />
                <div className="pointer-events-none absolute inset-0">
                  {audioStudioMarkerKeys.map((markerKey) => {
                    const markerValue = markerValues[markerKey]
                    if (
                      markerValue === null ||
                      !loadedAudio ||
                      loadedAudio.duration <= 0 ||
                      waveformWidth <= 0
                    ) {
                      return null
                    }
                    const left = `${(markerValue / loadedAudio.duration) * waveformWidth}px`
                    return (
                      <div
                        key={markerKey}
                        className="pointer-events-none absolute inset-y-0"
                        style={{ left }}
                      >
                        <button
                          type="button"
                          className={`pointer-events-auto absolute inset-y-0 left-1/2 z-10 flex w-6 -translate-x-1/2 cursor-grab justify-center active:cursor-grabbing ${markerColorClass(
                            markerKey,
                          )}`}
                          onPointerDown={(event) => {
                            event.preventDefault()
                            event.stopPropagation()
                            dragStateRef.current = {
                              kind: 'marker',
                              markerKey,
                            }
                            updateMarkerAtClientPosition(markerKey, event.clientX)
                          }}
                          aria-label={`Drag ${markerLabel(markerKey)} marker`}
                        >
                          <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-current" />
                          <span className="absolute left-[calc(50%+6px)] top-1 whitespace-nowrap border border-current bg-background px-1.5 py-0.5 font-mono text-[10px] leading-none tracking-[0.12em]">
                            {markerLabel(markerKey)}
                          </span>
                        </button>
                      </div>
                    )
                  })}
                  {loadedAudio && waveformWidth > 0 ? (
                    <div
                      className="pointer-events-none absolute inset-y-0"
                      style={{
                        left: `${(currentTime / Math.max(loadedAudio.duration, 0.001)) * waveformWidth}px`,
                      }}
                    >
                      <button
                        type="button"
                        className="pointer-events-auto absolute inset-y-0 left-1/2 z-10 flex w-6 -translate-x-1/2 cursor-grab justify-center active:cursor-grabbing"
                        onPointerDown={(event) => {
                          event.preventDefault()
                          event.stopPropagation()
                          dragStateRef.current = { kind: 'playhead' }
                          seekToClientPosition(event.clientX)
                        }}
                        aria-label="Drag playhead"
                      >
                        <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-white" />
                        <span className="absolute left-1/2 top-8 h-4 w-4 -translate-x-1/2 border border-white bg-background/80" />
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
            {isLoading ? (
              <div className="pointer-events-none absolute inset-0 flex items-center justify-center gap-2 bg-background/70 text-sm text-primary">
                <LoaderCircle className="h-4 w-4 animate-spin" />
                Loading waveform...
              </div>
            ) : null}
            {!isLoading && !entry ? (
              <div className="pointer-events-none absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
                Choose a sound cue to preview its waveform.
              </div>
            ) : null}
            {!isLoading && entry && loadError ? (
              <div className="pointer-events-none absolute inset-0 flex items-center justify-center px-6 text-center text-sm text-destructive">
                {loadError}
              </div>
            ) : null}
          </div>

          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <ButtonGroup className="w-auto">
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!canPreview || isPlaying}
                  onClick={startStandardPlayback}
                >
                  <Play className="h-4 w-4" />
                  Play
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!canLoopPreview}
                  onPointerDown={(event) => {
                    event.preventDefault()
                    startHoldLoopPlayback()
                  }}
                  onPointerUp={releaseHoldLoopPlayback}
                  onPointerCancel={releaseHoldLoopPlayback}
                  onPointerLeave={() => {
                    if (holdLoopActiveRef.current) {
                      releaseHoldLoopPlayback()
                    }
                  }}
                >
                  <Repeat2 className="h-4 w-4" />
                  Loop Play
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!canPreview || !isPlaying}
                  onClick={() => {
                    playbackProgramRef.current = { mode: 'idle' }
                    audioRef.current?.pause()
                  }}
                >
                  <Pause className="h-4 w-4" />
                  Pause
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!canPreview}
                  onClick={() => {
                    stopPlayback(0)
                  }}
                >
                  <Square className="h-4 w-4" />
                  Stop
                </Button>
              </ButtonGroup>

              <div className="ml-auto flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
                <Badge variant="outline">Zoom {waveformZoom.toFixed(1)}x</Badge>
                {visibleDurationSeconds > 0 ? (
                  <Badge variant="outline">
                    View {formatSeconds(visibleDurationSeconds)}
                  </Badge>
                ) : null}
                <Waves className="h-4 w-4 text-primary" />
                {formatSeconds(currentTime)}
                <span>/</span>
                {formatSeconds(loadedAudio?.duration ?? 0)}
              </div>
            </div>

            <TheGridSlider
              value={currentTime}
              min={0}
              max={loadedAudio?.duration ?? 1}
              step={0.01}
              label="Playhead"
              showValue={false}
              disabled={!canPreview}
              onChange={(next) => {
                const audio = audioRef.current
                if (audio && Number.isFinite(next)) {
                  audio.currentTime = next
                }
                setCurrentTime(next)
                onPlayheadChange?.(next)
              }}
            />
          </div>
        </div>
      </HUDFrame>

      <audio ref={audioRef} src={loadedAudio?.objectUrl} preload="metadata" hidden />
    </div>
  )
}
