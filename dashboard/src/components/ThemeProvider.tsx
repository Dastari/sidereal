import * as React from 'react'
import { ThemeProviderContext, useThemeState } from '@/hooks/use-theme'

type Theme = 'dark' | 'light' | 'system'

interface ThemeProviderProps {
  children: React.ReactNode
  defaultTheme?: Theme
  storageKey?: string
}

export function ThemeProvider({
  children,
  defaultTheme = 'system',
  storageKey = 'sidereal-theme',
}: ThemeProviderProps) {
  const themeState = useThemeState(defaultTheme, storageKey)

  return (
    <ThemeProviderContext.Provider value={themeState}>
      {children}
    </ThemeProviderContext.Provider>
  )
}
