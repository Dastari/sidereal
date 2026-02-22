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
}

export function AppLayout({
  children,
  sidebar,
  header,
  detailPanel,
  sidebarWidth = 280,
  detailPanelWidth = 320,
  onSidebarResize,
}: AppLayoutProps) {
  const [isDragging, setIsDragging] = React.useState(false)
  const [currentWidth, setCurrentWidth] = React.useState(sidebarWidth)

  React.useEffect(() => {
    setCurrentWidth(sidebarWidth)
  }, [sidebarWidth])

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault()
    setIsDragging(true)
  }

  React.useEffect(() => {
    if (!isDragging) return

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.max(200, Math.min(600, e.clientX))
      setCurrentWidth(newWidth)
    }

    const handleMouseUp = () => {
      setIsDragging(false)
      if (onSidebarResize) {
        onSidebarResize(currentWidth)
      }
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isDragging, currentWidth, onSidebarResize])

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
            style={{ width: currentWidth }}
          >
            {sidebar}
            <div
              className={cn(
                'absolute top-0 right-0 w-1 h-full cursor-col-resize hover:bg-primary/20 transition-colors',
                isDragging && 'bg-primary/40',
              )}
              onMouseDown={handleMouseDown}
            />
          </aside>
        )}
        <main className="flex-1 overflow-hidden relative">{children}</main>
        {detailPanel && (
          <aside
            className="flex-none border-l border-border bg-card overflow-hidden"
            style={{ width: detailPanelWidth }}
          >
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
    <div className={cn('flex h-full flex-col overflow-hidden', className)}>
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
