import { createContext, useContext, useEffect, useState } from 'react'

type Theme = 'dark' | 'light' | 'system'

type ThemeProviderState = {
  theme: Theme
  setTheme: (theme: Theme) => void
  resolvedTheme: 'dark' | 'light'
}

const initialState: ThemeProviderState = {
  theme: 'system',
  setTheme: () => null,
  resolvedTheme: 'dark',
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

function applyThemeClass(resolved: 'dark' | 'light') {
  const root = window.document.documentElement
  root.classList.remove('light', 'dark')
  root.classList.add(resolved)
  root.style.colorScheme = resolved
  root.dataset.theme = resolved
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

  useEffect(() => {
    const applyCurrentTheme = () => {
      const nextResolvedTheme = resolveTheme(theme)
      applyThemeClass(nextResolvedTheme)
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
  }, [theme])

  const setTheme = (newTheme: Theme) => {
    try {
      localStorage.setItem(storageKey, newTheme)
    } catch {
      // Ignore storage write failures (for example, privacy mode).
    }
    setThemeState(newTheme)
  }

  return { theme, setTheme, resolvedTheme }
}
