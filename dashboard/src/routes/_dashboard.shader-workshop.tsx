import { createFileRoute } from '@tanstack/react-router'
import { RouteErrorState } from '@/components/feedback/route-feedback'
import { ShaderWorkshopToolPage } from '@/routes-lazy/shader-workshop-route'

export const Route = createFileRoute('/_dashboard/shader-workshop')({
  errorComponent: ({ error }) => (
    <RouteErrorState title="Shader workshop route failed" error={error} />
  ),
  component: ShaderWorkshopRoutePage,
})

function ShaderWorkshopRoutePage() {
  return <ShaderWorkshopToolPage selectedShaderId={null} />
}
