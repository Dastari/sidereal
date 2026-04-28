import type { Vec3Tuple } from './types'

export type OverlayViewTransform = {
  widthPx: number
  heightPx: number
  lengthM: number
  widthM: number
  zoom: number
  panX: number
  panY: number
}

export type OverlayPoint = {
  x: number
  y: number
}

export function snapToGrid(value: number, spacing: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(spacing) || spacing <= 0) {
    return value
  }
  return Number((Math.round(value / spacing) * spacing).toFixed(6))
}

export function snapHardpointOffset(
  offset: Vec3Tuple,
  spacing: number,
): Vec3Tuple {
  return [snapToGrid(offset[0], spacing), snapToGrid(offset[1], spacing), 0]
}

export function mirrorHardpointOffset(offset: Vec3Tuple): Vec3Tuple {
  return [-offset[0], offset[1], 0]
}

export function shipMetersToScreen(
  offset: Vec3Tuple,
  transform: OverlayViewTransform,
): OverlayPoint {
  const metersPerPixelX = transform.widthM / transform.widthPx
  const metersPerPixelY = transform.lengthM / transform.heightPx
  const centerX = transform.widthPx / 2 + transform.panX
  const centerY = transform.heightPx / 2 + transform.panY
  return {
    x: centerX + (offset[0] / metersPerPixelX) * transform.zoom,
    y: centerY - (offset[1] / metersPerPixelY) * transform.zoom,
  }
}

export function screenToShipMeters(
  point: OverlayPoint,
  transform: OverlayViewTransform,
): Vec3Tuple {
  const metersPerPixelX = transform.widthM / transform.widthPx
  const metersPerPixelY = transform.lengthM / transform.heightPx
  const centerX = transform.widthPx / 2 + transform.panX
  const centerY = transform.heightPx / 2 + transform.panY
  return [
    ((point.x - centerX) / transform.zoom) * metersPerPixelX,
    ((centerY - point.y) / transform.zoom) * metersPerPixelY,
    0,
  ]
}

export function zoomAroundPoint(
  current: Pick<OverlayViewTransform, 'zoom' | 'panX' | 'panY'>,
  viewportPoint: OverlayPoint,
  viewportCenter: OverlayPoint,
  nextZoom: number,
): Pick<OverlayViewTransform, 'zoom' | 'panX' | 'panY'> {
  const clampedZoom = Math.min(8, Math.max(0.35, nextZoom))
  const ratio = clampedZoom / current.zoom
  return {
    zoom: clampedZoom,
    panX:
      viewportPoint.x -
      viewportCenter.x -
      (viewportPoint.x - viewportCenter.x - current.panX) * ratio,
    panY:
      viewportPoint.y -
      viewportCenter.y -
      (viewportPoint.y - viewportCenter.y - current.panY) * ratio,
  }
}
