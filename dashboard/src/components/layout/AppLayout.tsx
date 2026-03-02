import * as React from 'react'
import { cn } from '@/lib/utils'

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
  const [currentSidebarWidth, setCurrentSidebarWidth] =
    React.useState(sidebarWidth)
  const [currentDetailWidth, setCurrentDetailWidth] =
    React.useState(detailPanelWidth)

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
    <div className="flex h-full w-full flex-col overflow-hidden bg-background">
      {header && (
        <header className="flex-none border-b border-border bg-card">
          {header}
        </header>
      )}
      <div className="flex flex-1 overflow-hidden">
        {sidebar && (
          <aside
            className="flex-none border-r border-border bg-card overflow-hidden relative"
            style={{ width: currentSidebarWidth }}
          >
            {sidebar}
            <div
              className={cn(
                'absolute top-0 right-0 w-1 h-full cursor-col-resize hover:bg-primary/20 transition-colors',
                isSidebarDragging && 'bg-primary/40',
              )}
              onMouseDown={handleSidebarMouseDown}
            />
          </aside>
        )}
        <main className="flex-1 overflow-hidden relative bg-card">
          {children}
        </main>
        {detailPanel && (
          <aside
            className="flex-none border-l border-border bg-card overflow-hidden relative"
            style={{ width: currentDetailWidth }}
          >
            <div
              className={cn(
                'absolute top-0 left-0 w-1 h-full cursor-col-resize hover:bg-primary/20 transition-colors',
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
    <div
      className={cn('flex h-full flex-col overflow-hidden z-100', className)}
    >
      {children}
    </div>
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
    <div
      className={cn(
        'flex-none px-5 py-3 border-b border-border-subtle',
        className,
      )}
    >
      {children}
    </div>
  )
}

export function PanelContent({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return <div className={cn('flex-1 overflow-auto', className)}>{children}</div>
}
