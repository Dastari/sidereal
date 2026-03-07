import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/shader-workbench')({
  beforeLoad: () => {
    throw redirect({ to: '/shader-workshop' })
  },
})
