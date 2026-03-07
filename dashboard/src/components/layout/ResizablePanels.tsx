import * as React from 'react'
import { cn } from '@/lib/utils'

interface HorizontalSplitProps {
  left: React.ReactNode
  right: React.ReactNode
  leftWidth: number
  minLeftWidth?: number
  minRightWidth?: number
  maxLeftWidth?: number
  onLeftWidthChange: (width: number) => void
  className?: string
}

export function HorizontalSplitPanels({
  left,
  right,
  leftWidth,
  minLeftWidth = 320,
  minRightWidth = 320,
  maxLeftWidth,
  onLeftWidthChange,
  className,
}: HorizontalSplitProps) {
  const [dragging, setDragging] = React.useState(false)
  const containerRef = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    if (!dragging) {
      return
    }

    const handleMouseMove = (event: MouseEvent) => {
      const container = containerRef.current
      if (!container) return
      const bounds = container.getBoundingClientRect()
      const containerWidth = bounds.width
      const computedMaxLeftWidth = Math.min(
        maxLeftWidth ?? containerWidth - minRightWidth,
        containerWidth - minRightWidth,
      )
      const nextWidth = Math.max(
        minLeftWidth,
        Math.min(computedMaxLeftWidth, event.clientX - bounds.left),
      )
      onLeftWidthChange(nextWidth)
    }

    const handleMouseUp = () => setDragging(false)

    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [dragging, maxLeftWidth, minLeftWidth, minRightWidth, onLeftWidthChange])

  return (
    <div
      ref={containerRef}
      className={cn('flex min-h-0 overflow-hidden grow', className)}
    >
      <div
        className="flex min-h-0 min-w-0 shrink-0 flex-col"
        style={{ width: leftWidth }}
      >
        {left}
      </div>
      <div
        className={cn(
          'w-1 shrink-0 cursor-col-resize bg-border/70 transition-colors hover:bg-primary/50',
          dragging && 'bg-primary/70',
        )}
        onMouseDown={(event) => {
          event.preventDefault()
          setDragging(true)
        }}
      />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">{right}</div>
    </div>
  )
}

interface VerticalSplitProps {
  top: React.ReactNode
  bottom: React.ReactNode
  topHeight: number
  minTopHeight?: number
  minBottomHeight?: number
  maxTopHeight?: number
  onTopHeightChange: (height: number) => void
  className?: string
}

export function VerticalSplitPanels({
  top,
  bottom,
  topHeight,
  minTopHeight = 220,
  minBottomHeight = 160,
  maxTopHeight,
  onTopHeightChange,
  className,
}: VerticalSplitProps) {
  const [dragging, setDragging] = React.useState(false)
  const containerRef = React.useRef<HTMLDivElement>(null)

  React.useEffect(() => {
    if (!dragging) {
      return
    }

    const handleMouseMove = (event: MouseEvent) => {
      const container = containerRef.current
      if (!container) return
      const bounds = container.getBoundingClientRect()
      const containerHeight = bounds.height
      const computedMaxTopHeight = Math.min(
        maxTopHeight ?? containerHeight - minBottomHeight,
        containerHeight - minBottomHeight,
      )
      const nextHeight = Math.max(
        minTopHeight,
        Math.min(computedMaxTopHeight, event.clientY - bounds.top),
      )
      onTopHeightChange(nextHeight)
    }

    const handleMouseUp = () => setDragging(false)

    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [dragging, maxTopHeight, minBottomHeight, minTopHeight, onTopHeightChange])

  return (
    <div
      ref={containerRef}
      className={cn('flex min-h-0 flex-1 flex-col overflow-hidden', className)}
    >
      <div className="min-h-0 shrink-0" style={{ height: topHeight }}>
        {top}
      </div>
      <div
        className={cn(
          'h-1 shrink-0 cursor-row-resize bg-border/70 transition-colors hover:bg-primary/50',
          dragging && 'bg-primary/70',
        )}
        onMouseDown={(event) => {
          event.preventDefault()
          setDragging(true)
        }}
      />
      <div className="min-h-0 flex-1">{bottom}</div>
    </div>
  )
}
