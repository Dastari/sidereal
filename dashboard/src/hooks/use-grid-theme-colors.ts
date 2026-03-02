import { useEffect, useState } from 'react'
import {
  getGridThemeColors,
  type GridThemeColors,
} from '@/lib/theme-colors'

/**
 * Returns grid theme colors from CSS variables, re-reading when theme changes.
 */
export function useGridThemeColors(resolvedTheme: 'dark' | 'light'): GridThemeColors {
  const [colors, setColors] = useState<GridThemeColors>(() =>
    typeof document !== 'undefined' ? getGridThemeColors() : getGridThemeColors(),
  )

  useEffect(() => {
    setColors(getGridThemeColors())
  }, [resolvedTheme])

  return colors
}
