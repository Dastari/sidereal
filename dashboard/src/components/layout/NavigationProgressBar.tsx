import { useRouterState } from '@tanstack/react-router'
import { cn } from '@/lib/utils'

export function NavigationProgressBar() {
  const isNavigating = useRouterState({
    select: (state) =>
      state.isLoading || state.isTransitioning || state.status === 'pending',
  })

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-0 h-0.5 overflow-hidden">
      <div
        className={cn(
          'h-full w-full opacity-0 transition-opacity duration-150',
          isNavigating && 'opacity-100',
        )}
      >
        <div className="navigation-progress-bar h-full w-1/3 bg-[linear-gradient(90deg,rgba(96,165,250,0)_0%,rgba(96,165,250,0.9)_45%,rgba(191,219,254,1)_55%,rgba(96,165,250,0)_100%)]" />
      </div>
    </div>
  )
}
