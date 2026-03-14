import { useEffect, useState } from 'react'
import { useRouterState } from '@tanstack/react-router'
import { ProgressBar } from '@/components/thegridcn/progress-bar'
import { cn } from '@/lib/utils'

export function NavigationProgressBar() {
  const [hasHydrated, setHasHydrated] = useState(false)
  const isNavigating = useRouterState({
    select: (state) =>
      state.isLoading || state.isTransitioning || state.status === 'pending',
  })

  useEffect(() => {
    setHasHydrated(true)
  }, [])

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-0 h-[3px] overflow-hidden">
      <ProgressBar
        className={cn(
          'h-full w-full opacity-0 transition-opacity duration-150',
          hasHydrated && isNavigating && 'opacity-100',
        )}
        indeterminate
      />
    </div>
  )
}
