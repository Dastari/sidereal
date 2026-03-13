import { createContext, useContext, useEffect, useState } from 'react'
import type { GridIntensity, GridTheme } from '@/lib/grid-theme'
import {
  DEFAULT_GRID_INTENSITY,
  DEFAULT_GRID_THEME,
  GRID_INTENSITY_STORAGE_KEY,
  GRID_THEME_STORAGE_KEY,
  isGridIntensity,
  isGridTheme,
} from '@/lib/grid-theme'

type Theme = 'dark' | 'light' | 'system'

type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
  resolvedTheme: 'dark' | 'light'
  gridTheme: GridTheme
  setGridTheme: (theme: GridTheme) => void
  gridIntensity: GridIntensity
  setGridIntensity: (intensity: GridIntensity) => void
}

const initialState: ThemeProviderState = {
  theme: 'system',
  setTheme: () => null,
  resolvedTheme: 'dark',
  gridTheme: DEFAULT_GRID_THEME,
  setGridTheme: () => null,
  gridIntensity: DEFAULT_GRID_INTENSITY,
  setGridIntensity: () => null,
}

export const ThemeProviderContext =
  createContext<ThemeProviderState>(initialState)

function readStoredTheme(storageKey: string): Theme | null {
  if (typeof window === 'undefined') return null
  let stored: string | null = null
  try {
    stored = localStorage.getItem(storageKey)
  } catch {
    return null
  }
  if (stored === 'dark' || stored === 'light' || stored === 'system') {
    return stored
  }
  return null
}

function readStoredGridTheme(storageKey: string): GridTheme | null {
  if (typeof window === 'undefined') return null
  let stored: string | null = null
  try {
    stored = localStorage.getItem(storageKey)
  } catch {
    return null
  }
  return isGridTheme(stored) ? stored : null
}

function readStoredGridIntensity(storageKey: string): GridIntensity | null {
  if (typeof window === 'undefined') return null
  let stored: string | null = null
  try {
    stored = localStorage.getItem(storageKey)
  } catch {
    return null
  }
  return isGridIntensity(stored) ? stored : null
}

function resolveTheme(theme: Theme): 'dark' | 'light' {
  if (typeof window === 'undefined') {
    return theme === 'light' ? 'light' : 'dark'
  }
  if (theme === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light'
  }
  return theme
}

function applyThemeAttributes(
  resolved: 'dark' | 'light',
  gridTheme: GridTheme,
  gridIntensity: GridIntensity,
) {
  const root = window.document.documentElement
  root.classList.remove('light', 'dark')
  root.classList.add(resolved)
  root.style.colorScheme = resolved
  root.dataset.theme = gridTheme
  root.dataset.colorScheme = resolved

  if (gridIntensity === 'off') {
    root.removeAttribute('data-tron-intensity')
  } else {
    root.setAttribute('data-tron-intensity', gridIntensity)
  }
}

export function useTheme() {
  const context = useContext(ThemeProviderContext)
  // Context is always defined when used within ThemeProvider
  return context
}

export function useThemeState(
  defaultTheme: Theme = 'system',
  storageKey = 'sidereal-theme',
): ThemeProviderState {
  const [theme, setThemeState] = useState<Theme>(() => {
    return readStoredTheme(storageKey) ?? defaultTheme
  })

  const [resolvedTheme, setResolvedTheme] = useState<'dark' | 'light'>(() => {
    if (typeof window === 'undefined') {
      return resolveTheme(defaultTheme)
    }

    const root = window.document.documentElement
    if (root.classList.contains('light')) return 'light'
    if (root.classList.contains('dark')) return 'dark'

    return resolveTheme(readStoredTheme(storageKey) ?? defaultTheme)
  })
  const [gridTheme, setGridThemeState] = useState<GridTheme>(() => {
    return readStoredGridTheme(GRID_THEME_STORAGE_KEY) ?? DEFAULT_GRID_THEME
  })
  const [gridIntensity, setGridIntensityState] = useState<GridIntensity>(() => {
    return (
      readStoredGridIntensity(GRID_INTENSITY_STORAGE_KEY) ??
      DEFAULT_GRID_INTENSITY
    )
  })

  useEffect(() => {
    const applyCurrentTheme = () => {
      const nextResolvedTheme = resolveTheme(theme)
      applyThemeAttributes(nextResolvedTheme, gridTheme, gridIntensity)
      setResolvedTheme(nextResolvedTheme)
    }

    applyCurrentTheme()

    if (theme !== 'system') {
      return
    }

    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const onChange = () => {
      applyCurrentTheme()
    }
    media.addEventListener('change', onChange)
    return () => {
      media.removeEventListener('change', onChange)
    }
  }, [gridIntensity, gridTheme, theme])

  const setTheme = (newTheme: Theme) => {
    try {
      localStorage.setItem(storageKey, newTheme)
    } catch {
      // Ignore storage write failures (for example, privacy mode).
    }
    setThemeState(newTheme)
  }

  const setGridTheme = (newTheme: GridTheme) => {
    try {
      localStorage.setItem(GRID_THEME_STORAGE_KEY, newTheme)
    } catch {
      // Ignore storage write failures (for example, privacy mode).
    }
    setGridThemeState(newTheme)
  }

  const setGridIntensity = (newIntensity: GridIntensity) => {
    try {
      localStorage.setItem(GRID_INTENSITY_STORAGE_KEY, newIntensity)
    } catch {
      // Ignore storage write failures (for example, privacy mode).
    }
    setGridIntensityState(newIntensity)
  }

  return {
    theme,
    setTheme,
    resolvedTheme,
    gridTheme,
    setGridTheme,
    gridIntensity,
    setGridIntensity,
  }
}
