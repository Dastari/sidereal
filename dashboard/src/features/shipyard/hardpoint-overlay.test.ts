import { describe, expect, it } from 'vitest'
import {
  mirrorHardpointOffset,
  screenToShipMeters,
  shipMetersToScreen,
  snapHardpointOffset,
  snapToGrid,
} from './hardpoint-overlay'

const transform = {
  widthPx: 600,
  heightPx: 1000,
  widthM: 12,
  lengthM: 20,
  zoom: 1,
  panX: 0,
  panY: 0,
}

describe('Shipyard hardpoint overlay math', () => {
  it('converts between pixels and authored local meters', () => {
    const screen = shipMetersToScreen([3, 5, 0], transform)
    expect(screen).toEqual({ x: 450, y: 250 })
    expect(screenToShipMeters(screen, transform)).toEqual([3, 5, 0])
  })

  it('snaps offsets to the requested grid spacing', () => {
    expect(snapToGrid(1.24, 0.5)).toBe(1)
    expect(snapHardpointOffset([1.24, -2.26, 0], 0.5)).toEqual([1, -2.5, 0])
  })

  it('mirrors across the local X axis while preserving forward Y', () => {
    expect(mirrorHardpointOffset([4, -7, 0])).toEqual([-4, -7, 0])
  })

  it('keeps drag updates in the X/Y authoring plane', () => {
    const dragged = screenToShipMeters({ x: 350, y: 550 }, transform)
    expect(dragged[2]).toBe(0)
  })
})
