import { useEffect, useState } from 'react'
import type { GridThemeColors } from '@/lib/theme-colors'
import { getGridThemeColors } from '@/lib/theme-colors'

/**
 * Returns grid theme colors from CSS variables, re-reading when theme changes.
 */
export function useGridThemeColors(themeSignature: string): GridThemeColors {
  const [colors, setColors] = useState<GridThemeColors>(() =>
    typeof document !== 'undefined' ? getGridThemeColors() : getGridThemeColors(),
  )

  useEffect(() => {
    setColors(getGridThemeColors())
  }, [themeSignature])

  return colors
}
