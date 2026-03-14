import * as React from 'react'
import { cn } from '@/lib/utils'
import {
  SidebarNavBody,
  SidebarNavFooter,
  SidebarNavFrame,
  SidebarNavHeader,
} from '@/components/ui/sidebar-nav'

interface AppLayoutProps {
  children: React.ReactNode
  sidebar?: React.ReactNode
  header?: React.ReactNode
  detailPanel?: React.ReactNode
  sidebarWidth?: number
  detailPanelWidth?: number
  onSidebarResize?: (width: number) => void
  onDetailPanelResize?: (width: number) => void
}

export function AppLayout({
  children,
  sidebar,
  header,
  detailPanel,
  sidebarWidth = 280,
  detailPanelWidth = 320,
  onSidebarResize,
  onDetailPanelResize,
}: AppLayoutProps) {
  const [isSidebarDragging, setIsSidebarDragging] = React.useState(false)
  const [isDetailDragging, setIsDetailDragging] = React.useState(false)
  const [currentSidebarWidth, setCurrentSidebarWidth] = React.useState(280)
  const [currentDetailWidth, setCurrentDetailWidth] = React.useState(320)

  React.useEffect(() => {
    setCurrentSidebarWidth(sidebarWidth)
  }, [sidebarWidth])

  React.useEffect(() => {
    setCurrentDetailWidth(detailPanelWidth)
  }, [detailPanelWidth])

  const handleSidebarMouseDown = (e: React.MouseEvent) => {
    e.preventDefault()
    setIsSidebarDragging(true)
  }

  const handleDetailMouseDown = (e: React.MouseEvent) => {
    e.preventDefault()
    setIsDetailDragging(true)
  }

  React.useEffect(() => {
    if (!isSidebarDragging) return

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.max(200, Math.min(600, e.clientX))
      setCurrentSidebarWidth(newWidth)
    }

    const handleMouseUp = () => {
      setIsSidebarDragging(false)
      if (onSidebarResize) {
        onSidebarResize(currentSidebarWidth)
      }
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isSidebarDragging, currentSidebarWidth, onSidebarResize])

  React.useEffect(() => {
    if (!isDetailDragging) return

    const handleMouseMove = (e: MouseEvent) => {
      const viewportWidth = window.innerWidth
      const newWidth = Math.max(240, Math.min(700, viewportWidth - e.clientX))
      setCurrentDetailWidth(newWidth)
    }

    const handleMouseUp = () => {
      setIsDetailDragging(false)
      if (onDetailPanelResize) {
        onDetailPanelResize(currentDetailWidth)
      }
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isDetailDragging, currentDetailWidth, onDetailPanelResize])

  return (
    <div className="flex h-full w-full flex-col bg-background">
      {header && (
        <header className="flex-none border-b border-border bg-card">
          {header}
        </header>
      )}
      <div className="flex flex-1 overflow-x-hidden overflow-y-visible">
        {sidebar && (
          <aside
            className="relative flex-none bg-transparent px-2 py-2 z-5"
            style={{ width: currentSidebarWidth }}
          >
            {sidebar}
            <div
              className={cn(
                'absolute right-0 top-0 h-full w-1 cursor-col-resize transition-colors hover:bg-primary/20',
                isSidebarDragging && 'bg-primary/40',
              )}
              onMouseDown={handleSidebarMouseDown}
            />
          </aside>
        )}
        <main className="grow relative flex bg-card">{children}</main>
        {detailPanel && (
          <aside
            className="relative flex-none bg-transparent px-2 py-2 pl-0 z-5"
            style={{ width: currentDetailWidth }}
          >
            <div
              className={cn(
                'absolute -left-1.5 top-0 h-full w-1 cursor-col-resize transition-colors hover:bg-primary/20',
                isDetailDragging && 'bg-primary/40',
              )}
              onMouseDown={handleDetailMouseDown}
            />
            {detailPanel}
          </aside>
        )}
      </div>
    </div>
  )
}

interface PanelProps {
  children: React.ReactNode
  className?: string
}

export function Panel({ children, className }: PanelProps) {
  return (
    <SidebarNavFrame className={cn('z-1 h-full', className)}>
      {children}
    </SidebarNavFrame>
  )
}

export function PanelHeader({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <SidebarNavHeader className={cn('flex-none px-5 py-3', className)}>
      {children}
    </SidebarNavHeader>
  )
}

export function PanelContent({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <SidebarNavBody className={className}>
      <div className="h-full overflow-auto">{children}</div>
    </SidebarNavBody>
  )
}

export function PanelFooter({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return <SidebarNavFooter className={className}>{children}</SidebarNavFooter>
}
