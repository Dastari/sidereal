/**
 * Reads a CSS color variable and returns RGB values in 0-1 range for WebGL/canvas.
 * Uses a temporary element + canvas to resolve any format (oklch, rgb, etc.) to RGB.
 * Modern browsers return oklch() from getComputedStyle, so we use canvas to sample the pixel.
 */
export function getCssColorRgb(
  varName: string,
  root: HTMLElement = document.documentElement,
): [number, number, number] {
  if (typeof document === 'undefined') {
    return [0.5, 0.5, 0.5]
  }
  const el = document.createElement('div')
  el.style.cssText = `background: var(${varName}); position: absolute; left: -9999px;`
  root.appendChild(el)
  const computed = getComputedStyle(el).backgroundColor
  root.removeChild(el)

  // Try parse rgb/rgba first (fast path)
  const rgbMatch = computed.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
  if (rgbMatch) {
    return [
      Number.parseInt(rgbMatch[1], 10) / 255,
      Number.parseInt(rgbMatch[2], 10) / 255,
      Number.parseInt(rgbMatch[3], 10) / 255,
    ]
  }

  // Use canvas to sample - works for oklch, lab, hsl, etc.
  const canvas = document.createElement('canvas')
  canvas.width = 1
  canvas.height = 1
  const ctx = canvas.getContext('2d')
  if (!ctx) return [0.5, 0.5, 0.5]
  ctx.fillStyle = computed
  ctx.fillRect(0, 0, 1, 1)
  const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data
  return [r / 255, g / 255, b / 255]
}

export type GridThemeColors = {
  background: [number, number, number]
  gridMajor: [number, number, number]
  gridMinor: [number, number, number]
  gridMicro: [number, number, number]
  edge: [number, number, number]
  label: [number, number, number]
  selectionRing: [number, number, number]
  originLine: [number, number, number]
  /** Resolve dot color from kind and optional entity_labels (entity_label wins: player=green, Ship=red, else default=foreground). */
  getEntityColor: (
    kind: string,
    entityLabels?: string[],
  ) => [number, number, number]
}

const ENTITY_CSS_VARS: Record<string, string> = {
  ship: '--color-entity-ship',
  station: '--color-entity-station',
  asteroid: '--color-entity-asteroid',
  planet: '--color-entity-planet',
  component: '--color-entity-default',
  default: '--color-entity-default',
}

const FALLBACK_COLORS: GridThemeColors = {
  background: [0.06, 0.08, 0.12],
  gridMajor: [0.35, 0.38, 0.45],
  gridMinor: [0.22, 0.25, 0.32],
  gridMicro: [0.18, 0.2, 0.26],
  edge: [0.35, 0.45, 0.6],
  label: [0.9, 0.92, 0.98],
  selectionRing: [0.5, 0.9, 0.65],
  originLine: [0.4, 0.5, 0.7],
  getEntityColor: () => [0.6, 0.7, 0.85] as [number, number, number],
}

/**
 * Reads all grid theme colors from CSS variables (matches dashboard styles.css).
 */
export function getGridThemeColors(root?: HTMLElement): GridThemeColors {
  if (typeof document === 'undefined') return FALLBACK_COLORS
  const el = root ?? document.documentElement

  const background = getCssColorRgb('--color-grid-background', el)
  const gridMajor = getCssColorRgb('--color-grid-major', el)
  const gridMinor = getCssColorRgb('--color-grid-minor', el)
  const gridMicro = getCssColorRgb('--color-grid-micro', el)
  const edge = getCssColorRgb('--color-border', el)
  const label = getCssColorRgb('--color-foreground', el)
  const selectionRing = getCssColorRgb('--color-success', el)
  const originLine = getCssColorRgb('--color-grid-origin', el)
  const foreground = getCssColorRgb('--color-foreground', el)
  const playerColor = getCssColorRgb('--color-entity-label-player', el)
  const shipColor = getCssColorRgb('--color-entity-label-ship', el)

  const getEntityColor = (
    _kind: string,
    entityLabels?: string[],
  ): [number, number, number] => {
    if (entityLabels?.length) {
      const labels = entityLabels.map((l) => String(l).toLowerCase())
      if (labels.includes('player')) return playerColor
      if (labels.some((l) => l === 'ship')) return shipColor
    }
    return foreground
  }

  return {
    background,
    gridMajor,
    gridMinor,
    gridMicro,
    edge,
    label,
    selectionRing,
    originLine,
    getEntityColor,
  }
}
