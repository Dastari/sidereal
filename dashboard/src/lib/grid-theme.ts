export type GridTheme =
  | 'tron'
  | 'ares'
  | 'clu'
  | 'athena'
  | 'aphrodite'
  | 'poseidon'

export type GridIntensity = 'off' | 'light' | 'medium' | 'heavy'

export const GRID_THEME_STORAGE_KEY = 'sidereal-grid-theme'
export const GRID_INTENSITY_STORAGE_KEY = 'sidereal-grid-intensity'
export const DEFAULT_GRID_THEME: GridTheme = 'tron'
export const DEFAULT_GRID_INTENSITY: GridIntensity = 'medium'

export const gridThemes: Array<{
  id: GridTheme
  label: string
  accent: string
  subtitle: string
}> = [
  {
    id: 'tron',
    label: 'Tron',
    accent: '#00d4ff',
    subtitle: 'Default cyan operator theme',
  },
  {
    id: 'ares',
    label: 'Ares',
    accent: '#ff3333',
    subtitle: 'Red warning-biased combat theme',
  },
  {
    id: 'clu',
    label: 'Clu',
    accent: '#ff6600',
    subtitle: 'Orange antagonist theme',
  },
  {
    id: 'athena',
    label: 'Athena',
    accent: '#ffd700',
    subtitle: 'Gold command theme',
  },
  {
    id: 'aphrodite',
    label: 'Aphrodite',
    accent: '#ff1493',
    subtitle: 'Pink neon theme',
  },
  {
    id: 'poseidon',
    label: 'Poseidon',
    accent: '#0066ff',
    subtitle: 'Deep blue variant',
  },
]

export const gridIntensities: Array<{
  id: GridIntensity
  label: string
  description: string
}> = [
  {
    id: 'off',
    label: 'Off',
    description: 'Keep the theme palette with restrained effects.',
  },
  {
    id: 'light',
    label: 'Light',
    description: 'Subtle glows and minimal panel chrome.',
  },
  {
    id: 'medium',
    label: 'Medium',
    description: 'Balanced brackets, glows, and scanline accents.',
  },
  {
    id: 'heavy',
    label: 'Heavy',
    description: 'Full HUD glow and animated chrome.',
  },
]

const gridThemeSet = new Set<GridTheme>(gridThemes.map((theme) => theme.id))
const gridIntensitySet = new Set<GridIntensity>(
  gridIntensities.map((intensity) => intensity.id),
)

export function isGridTheme(value: string | null): value is GridTheme {
  return value !== null && gridThemeSet.has(value as GridTheme)
}

export function isGridIntensity(value: string | null): value is GridIntensity {
  return value !== null && gridIntensitySet.has(value as GridIntensity)
}

