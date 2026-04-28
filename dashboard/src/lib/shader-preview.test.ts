import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { extractPreviewUniforms } from './shader-preview'

describe('extractPreviewUniforms', () => {
  it('flattens nested planet uniform structs with native ABI offsets', () => {
    const source = readFileSync(
      resolve(process.cwd(), '../data/shaders/star_visual.wgsl'),
      'utf8',
    )
    const uniforms = extractPreviewUniforms(source)
    const colorPrimary = uniforms.find(
      (uniform) => uniform.name === 'params.color_primary',
    )
    const lastByte = Math.max(
      ...uniforms.map((uniform) => uniform.byteOffset + 16),
    )

    expect(colorPrimary?.byteOffset).toBe(624)
    expect(lastByte).toBe(736)
  })
})
