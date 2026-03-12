import { createFileRoute } from '@tanstack/react-router'
import { ShaderWorkshopToolPage } from '@/routes-lazy/shader-workshop-route'

export const Route = createFileRoute('/_dashboard/shader-workshop/$shaderId')({
  component: ShaderWorkshopEntityRoutePage,
})

function ShaderWorkshopEntityRoutePage() {
  const { shaderId } = Route.useParams()
  return <ShaderWorkshopToolPage selectedShaderId={shaderId} />
}
