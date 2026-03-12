import { useNavigate } from '@tanstack/react-router'
import { ShaderWorkshopPage } from '@/features/shaders/ShaderWorkshopPage'

export function ShaderWorkshopToolPage({
  selectedShaderId,
}: {
  selectedShaderId: string | null
}) {
  const navigate = useNavigate()

  return (
    <ShaderWorkshopPage
      selectedShaderId={selectedShaderId}
      onSelectedShaderIdChange={(shaderId) => {
        void navigate({
          to: shaderId ? '/shader-workshop/$shaderId' : '/shader-workshop',
          params: shaderId ? { shaderId } : {},
          search: (prev) => prev,
          replace: true,
        })
      }}
    />
  )
}
