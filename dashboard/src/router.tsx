import { createRouter as createTanStackRouter } from '@tanstack/react-router'
import { routeTree } from './routeTree.gen'
import { RouteNotFoundState } from '@/components/feedback/route-feedback'

export function getRouter() {
  const router = createTanStackRouter({
    routeTree,
    defaultNotFoundComponent: () => (
      <RouteNotFoundState
        title="Route not found"
        description="This dashboard view does not have a registered route."
      />
    ),

    scrollRestoration: true,
    defaultPreload: 'intent',
    defaultPreloadStaleTime: 0,
  })

  return router
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof getRouter>
  }
}
