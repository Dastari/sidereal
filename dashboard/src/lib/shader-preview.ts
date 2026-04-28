export type ShaderPreviewDiagnostic = {
  message: string
  line: number | null
  column: number | null
  type: 'error' | 'warning' | 'info'
}

export type ShaderPreviewMetrics = {
  compileMs: number
  pipelineMs: number
  applyMs: number
  frameMs: number | null
}

export type ShaderPreviewUniformDescriptor = {
  name: string
  binding: number
  sourceGroup: number
  previewGroup: number
  type: string
  components: number
  labels: Array<string>
  category: 'color' | 'vector' | 'scalar'
  defaults: Array<number>
  byteOffset: number
}

export type ShaderPreviewUniformValues = Record<string, Array<number>>

export type ShaderPreviewResult = {
  ok: boolean
  diagnostics: Array<ShaderPreviewDiagnostic>
  metrics: ShaderPreviewMetrics
  adaptedSource: string
  uniforms: Array<ShaderPreviewUniformDescriptor>
}

export type ShaderPreviewRenderOptions = {
  clear?: boolean
}

export type ShaderPreviewSequencePass = {
  values: ShaderPreviewUniformValues
  clear?: boolean
}

type BindingDeclaration = {
  group: number
  binding: number
  qualifier: 'uniform' | 'plain'
  name: string
  type: string
  comment?: string
}

type StructField = {
  name: string
  type: string
  comment?: string
}

type FlattenedUniformField = {
  name: string
  type: string
  components: number
  labels: Array<string>
  category: ShaderPreviewUniformDescriptor['category']
  defaults: Array<number>
  byteOffset: number
}

type TypeLayout = {
  size: number
  fields: Array<FlattenedUniformField>
}

type PreviewContext = {
  adapter: GPUAdapter
  device: GPUDevice
}

type PreparedPreviewPipeline = {
  diagnostics: Array<ShaderPreviewDiagnostic>
  declarations: Array<BindingDeclaration>
  pipeline: GPURenderPipeline | null
  compileMs: number
  pipelineMs: number
}

let previewContextPromise: Promise<PreviewContext> | null = null
const preparedPipelineCache = new Map<
  string,
  Promise<PreparedPreviewPipeline>
>()
const configuredCanvasCache = new WeakMap<
  HTMLCanvasElement,
  {
    context: GPUCanvasContext
    width: number
    height: number
    format: GPUTextureFormat
  }
>()

const PREVIEW_VERTEX_OUTPUT = `
struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
}

@vertex
fn sidereal_preview_vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
  var positions = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0),
  );
  var uvs = array<vec2<f32>, 3>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(2.0, 1.0),
    vec2<f32>(0.0, -1.0),
  );
  var out: VertexOutput;
  out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
  out.uv = uvs[vertex_index];
  return out;
}
`

const UNIFORM_DECLARATION =
  /@group\((\d+)\)\s*@binding\((\d+)\)\s*var<uniform>\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^;]+);(?:\s*\/\/\s*(.*))?/g
const GENERIC_BINDING_DECLARATION =
  /@group\((\d+)\)\s*@binding\((\d+)\)\s*var(?:<(uniform)>)?\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^;]+);(?:\s*\/\/\s*(.*))?/g
const STRUCT_DECLARATION =
  /struct\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{([\s\S]*?)\};?/g

function prettifyLabel(raw: string): string {
  return raw
    .replace(/[_-]+/g, ' ')
    .replace(/\b\w/g, (match) => match.toUpperCase())
}

function inferComponentLabels(
  descriptorName: string,
  components: number,
  inlineComment: string | undefined,
): Array<string> {
  const defaults = ['X', 'Y', 'Z', 'W']
  const semanticComment = inlineComment
    ?.split(',')
    .map((part) => part.trim())
    .filter(Boolean)

  if (semanticComment?.length) {
    const labels = semanticComment
      .map((part) => {
        const [, rhs] = part.split('=')
        return prettifyLabel((rhs || part).trim())
      })
      .slice(0, components)
    if (labels.length === components) {
      return labels
    }
  }

  if (/color|tint|rgb|rgba/i.test(descriptorName)) {
    return ['Red', 'Green', 'Blue', 'Alpha'].slice(0, components)
  }

  return defaults.slice(0, components)
}

function inferDefaultValues(
  descriptorName: string,
  components: number,
  labels: Array<string>,
): Array<number> {
  if (/color|tint|rgb|rgba/i.test(descriptorName)) {
    return [1, 0.75, 0.35, 1].slice(0, components)
  }

  return Array.from({ length: components }, (_, index) => {
    const label = labels[index]?.toLowerCase() ?? ''
    if (label.includes('age')) return 0.1
    if (label.includes('intensity')) return 1
    if (label.includes('density')) return 2
    if (label.includes('alpha')) return 1
    if (label.includes('time')) return 1
    return index === 3 ? 1 : 0
  })
}

function parseComponentCount(type: string): number {
  const vectorMatch = type.match(/^vec([2-4])<f32>$/)
  if (vectorMatch) {
    return Number(vectorMatch[1])
  }
  if (type === 'f32') {
    return 1
  }
  return 0
}

function parseVec4ArrayLength(type: string): number {
  const arrayMatch = type.match(/^array<\s*vec4<f32>\s*,\s*(\d+)\s*>$/)
  return arrayMatch ? Number(arrayMatch[1]) : 0
}

function categoryForUniformName(
  descriptorName: string,
  components: number,
): ShaderPreviewUniformDescriptor['category'] {
  if (/color|tint|rgb|rgba/i.test(descriptorName)) return 'color'
  return components === 1 ? 'scalar' : 'vector'
}

function layoutType(
  type: string,
  structMap: Map<string, Array<StructField>>,
  namePrefix: string,
  baseOffset: number,
  comment?: string,
): TypeLayout | null {
  const components = parseComponentCount(type)
  if (components > 0) {
    const labels = inferComponentLabels(namePrefix, components, comment)
    return {
      size: 16,
      fields: [
        {
          name: namePrefix,
          type,
          components,
          labels,
          category: categoryForUniformName(namePrefix, components),
          defaults: inferDefaultValues(namePrefix, components, labels),
          byteOffset: baseOffset,
        },
      ],
    }
  }

  const vec4ArrayLength = parseVec4ArrayLength(type)
  if (vec4ArrayLength > 0) {
    const fields: Array<FlattenedUniformField> = []
    for (let index = 0; index < vec4ArrayLength; index += 1) {
      const elementName = `${namePrefix}[${index}]`
      const labels = inferComponentLabels(elementName, 4, comment)
      fields.push({
        name: elementName,
        type: 'vec4<f32>',
        components: 4,
        labels,
        category: categoryForUniformName(elementName, 4),
        defaults: inferDefaultValues(elementName, 4, labels),
        byteOffset: baseOffset + index * 16,
      })
    }
    return { size: vec4ArrayLength * 16, fields }
  }

  const structFields = structMap.get(type)
  if (!structFields) return null

  let cursor = 0
  const fields: Array<FlattenedUniformField> = []
  for (const field of structFields) {
    const fieldLayout = layoutType(
      field.type,
      structMap,
      `${namePrefix}.${field.name}`,
      baseOffset + cursor,
      field.comment,
    )
    if (!fieldLayout) {
      continue
    }
    fields.push(...fieldLayout.fields)
    cursor += fieldLayout.size
  }
  return { size: cursor, fields }
}

export function extractPreviewUniforms(
  source: string,
): Array<ShaderPreviewUniformDescriptor> {
  const uniforms: Array<ShaderPreviewUniformDescriptor> = []
  const structMap = extractStructDefinitions(source)

  for (const match of source.matchAll(UNIFORM_DECLARATION)) {
    const sourceGroup = Number(match[1])
    const binding = Number(match[2])
    const name = match[3]
    const type = match[4].trim()
    const comment = match[5]
    const layout = layoutType(type, structMap, name, 0, comment)
    if (!layout) {
      continue
    }

    for (const field of layout.fields) {
      uniforms.push({
        name: field.name,
        binding,
        sourceGroup,
        previewGroup: 0,
        type: field.type,
        components: field.components,
        labels: field.labels,
        category: field.category,
        defaults: field.defaults,
        byteOffset: field.byteOffset,
      })
    }
  }

  return uniforms.sort((left, right) => left.binding - right.binding)
}

export function buildDefaultUniformValues(
  uniforms: Array<ShaderPreviewUniformDescriptor>,
): ShaderPreviewUniformValues {
  return Object.fromEntries(
    uniforms.map((uniform) => [uniform.name, [...uniform.defaults]]),
  )
}

function normalizePreviewSource(source: string): string {
  let adapted = source
  adapted = adapted.replace(
    /^#import\s+bevy_sprite::mesh2d_vertex_output::VertexOutput\s*$/gm,
    '',
  )
  adapted = adapted.replace(
    /@group\(\d+\)\s*@binding\((\d+)\)/g,
    '@group(0) @binding($1)',
  )

  const hasVertexOutput = /\bstruct\s+VertexOutput\b/.test(adapted)
  const hasVertexEntryPoint = /@vertex\s+fn\s+/.test(adapted)

  if (!hasVertexOutput) {
    adapted = `${PREVIEW_VERTEX_OUTPUT}\n${adapted}`
  } else if (!hasVertexEntryPoint) {
    adapted = `${adapted}\n${PREVIEW_VERTEX_OUTPUT}`
  }

  return adapted
}

function extractBindingDeclarations(source: string): Array<BindingDeclaration> {
  return Array.from(source.matchAll(GENERIC_BINDING_DECLARATION)).map(
    (match) => ({
      group: Number(match[1]),
      binding: Number(match[2]),
      qualifier: match[3] === 'uniform' ? 'uniform' : 'plain',
      name: match[4],
      type: match[5].trim(),
      comment: match[6],
    }),
  )
}

function extractStructDefinitions(
  source: string,
): Map<string, Array<StructField>> {
  const structMap = new Map<string, Array<StructField>>()
  for (const match of source.matchAll(STRUCT_DECLARATION)) {
    const structName = match[1]
    const body = match[2]
    const fields = body
      .split('\n')
      .flatMap((line): Array<StructField> => {
        const [fieldSource, rawComment] = line.split('//', 2)
        const fieldMatch = fieldSource
          .trim()
          .match(/^([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(.+?),?$/)
        if (!fieldMatch) return []
        const comment = (rawComment || '').trim()
        return [
          {
            name: fieldMatch[1],
            type: fieldMatch[2].trim(),
            comment: comment || undefined,
          },
        ]
      })
    structMap.set(structName, fields)
  }
  return structMap
}

function collectUnsupportedBindings(
  source: string,
  uniforms: Array<ShaderPreviewUniformDescriptor>,
): Array<ShaderPreviewDiagnostic> {
  const declarations = extractBindingDeclarations(source)
  const supportedUniformNames = new Set(uniforms.map((uniform) => uniform.name))

  return declarations.flatMap((declaration) => {
    if (declaration.group !== 0) {
      return [
        {
          message: `Preview renderer currently supports bind group 0 only. ${declaration.name} is declared in group ${declaration.group}.`,
          line: null,
          column: null,
          type: 'error' as const,
        },
      ]
    }

    if (
      declaration.qualifier !== 'uniform' &&
      declaration.type !== 'texture_2d<f32>' &&
      declaration.type !== 'sampler'
    ) {
      return [
        {
          message: `Preview renderer does not yet emulate binding ${declaration.name}: ${declaration.type}.`,
          line: null,
          column: null,
          type: 'error' as const,
        },
      ]
    }

    if (
      declaration.qualifier === 'uniform' &&
      !supportedUniformNames.has(declaration.name) &&
      !Array.from(supportedUniformNames).some((name) =>
        name.startsWith(`${declaration.name}.`),
      )
    ) {
      return [
        {
          message: `Preview renderer does not yet understand uniform ${declaration.name}: ${declaration.type}.`,
          line: null,
          column: null,
          type: 'error' as const,
        },
      ]
    }

    return []
  })
}

async function getPreviewContext(): Promise<PreviewContext> {
  if (!('gpu' in navigator)) {
    throw new Error('WebGPU is not available in this browser')
  }
  if (!previewContextPromise) {
    previewContextPromise = (async () => {
      const adapter = await navigator.gpu.requestAdapter()
      if (!adapter) {
        throw new Error('No WebGPU adapter available')
      }
      const device = await adapter.requestDevice()
      return { adapter, device }
    })()
  }
  return previewContextPromise
}

function mapCompilationInfo(
  info: GPUCompilationInfo,
): Array<ShaderPreviewDiagnostic> {
  return info.messages.map((message) => ({
    message: message.message,
    line: message.lineNum,
    column: message.linePos,
    type:
      message.type === 'warning' || message.type === 'info'
        ? message.type
        : 'error',
  }))
}

function packUniformData(
  descriptors: Array<ShaderPreviewUniformDescriptor>,
  values: ShaderPreviewUniformValues,
): ArrayBuffer {
  if (descriptors.length === 0) {
    return new Float32Array(4).buffer
  }
  const byteLength = Math.max(
    ...descriptors.map((descriptor) => descriptor.byteOffset + 16),
  )
  const packed = new Float32Array(byteLength / 4)

  for (const descriptor of descriptors) {
    const sourceValues = values[descriptor.name] ?? descriptor.defaults
    const baseIndex = descriptor.byteOffset / 4
    for (let index = 0; index < descriptor.components; index += 1) {
      packed[baseIndex + index] =
        sourceValues[index] ?? descriptor.defaults[index]
    }
    if (descriptor.category === 'color' && descriptor.components < 4) {
      packed[baseIndex + 3] = 1
    }
  }
  return packed.buffer
}

function buildPreviewTextureData(
  label: string,
  width: number,
  height: number,
): Uint8Array<ArrayBuffer> {
  const data = new Uint8Array(new ArrayBuffer(width * height * 4))
  const lower = label.toLowerCase()

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const index = (y * width + x) * 4
      const u = x / Math.max(width - 1, 1)
      const v = y / Math.max(height - 1, 1)
      let r = 0
      let g = 0
      let b = 0
      let a = 255

      if (lower.includes('fog')) {
        const dx = u - 0.5
        const dy = v - 0.5
        const dist = Math.sqrt(dx * dx + dy * dy)
        const explored = Math.max(0, 1 - dist * 1.6)
        const value = Math.round(explored * 255)
        r = value
        g = value
        b = value
      } else if (lower.includes('flare')) {
        const dx = u - 0.5
        const dy = v - 0.5
        const dist = Math.sqrt(dx * dx + dy * dy)
        const radial = Math.max(0, 1 - dist * 1.8)
        const streak = Math.max(
          Math.pow(Math.max(0, 1 - Math.abs(dx) * 4), 3),
          Math.pow(Math.max(0, 1 - Math.abs(dy) * 4), 3),
        )
        const intensity = Math.min(1, radial * 0.8 + streak * 0.6)
        r = Math.round(255 * intensity)
        g = Math.round(220 * intensity)
        b = Math.round(160 * intensity)
      } else {
        const dx = u - 0.5
        const dy = v - 0.5
        const dist = Math.sqrt(dx * dx + dy * dy)
        const rockMask = dist <= 0.48 ? 1 : 0
        const grain =
          ((Math.sin((x + 13) * 0.31) + Math.cos((y + 7) * 0.27)) * 0.25 +
            0.5) *
          rockMask
        r = Math.round((85 + grain * 90) * rockMask)
        g = Math.round((70 + grain * 70) * rockMask)
        b = Math.round((60 + grain * 55) * rockMask)
        a = Math.round(255 * rockMask)
      }

      data[index] = r
      data[index + 1] = g
      data[index + 2] = b
      data[index + 3] = a
    }
  }

  return data
}

function createPreviewTexture(device: GPUDevice, label: string): GPUTexture {
  const width = 64
  const height = 64
  const texture = device.createTexture({
    label: `preview-texture-${label}`,
    size: { width, height },
    format: 'rgba8unorm',
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  })
  device.queue.writeTexture(
    { texture },
    buildPreviewTextureData(label, width, height),
    { bytesPerRow: width * 4 },
    { width, height },
  )
  return texture
}

function clearFallback(
  canvas: HTMLCanvasElement,
  message: string,
  diagnostics: Array<ShaderPreviewDiagnostic>,
): void {
  const ctx = canvas.getContext('2d')
  if (!ctx) {
    return
  }

  ctx.clearRect(0, 0, canvas.width, canvas.height)
  const gradient = ctx.createLinearGradient(0, 0, 0, canvas.height)
  gradient.addColorStop(0, '#142032')
  gradient.addColorStop(1, '#05080e')
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, canvas.width, canvas.height)
  ctx.fillStyle = '#d6deec'
  ctx.font = '600 16px JetBrains Mono, monospace'
  ctx.textAlign = 'center'
  ctx.fillText(message, canvas.width / 2, canvas.height / 2)

  const firstError = diagnostics.find(
    (diagnostic) => diagnostic.type === 'error',
  )
  if (firstError) {
    ctx.font = '12px JetBrains Mono, monospace'
    ctx.fillStyle = '#ff9c9c'
    ctx.fillText(
      firstError.message.slice(0, 72),
      canvas.width / 2,
      canvas.height / 2 + 28,
    )
  }
}

function getConfiguredCanvasContext(
  canvas: HTMLCanvasElement,
  device: GPUDevice,
  format: GPUTextureFormat,
): GPUCanvasContext {
  const cached = configuredCanvasCache.get(canvas)
  if (
    cached &&
    cached.width === canvas.width &&
    cached.height === canvas.height &&
    cached.format === format
  ) {
    return cached.context
  }

  const context = canvas.getContext('webgpu')
  if (!context) {
    throw new Error('WebGPU canvas context is not available')
  }

  context.configure({
    device,
    format,
    alphaMode: 'premultiplied',
  })
  configuredCanvasCache.set(canvas, {
    context,
    width: canvas.width,
    height: canvas.height,
    format,
  })
  return context
}

async function preparePreviewPipeline(
  device: GPUDevice,
  format: GPUTextureFormat,
  adaptedSource: string,
): Promise<PreparedPreviewPipeline> {
  const cacheKey = `${format}:${adaptedSource}`
  const cached = preparedPipelineCache.get(cacheKey)
  if (cached) {
    return cached
  }

  const preparedPromise = (async () => {
    const compileStartedAt = performance.now()
    const shaderModule = device.createShaderModule({
      code: adaptedSource,
      label: 'sidereal-shader-preview',
    })
    const compilationInfo = await shaderModule.getCompilationInfo()
    const compileMs = performance.now() - compileStartedAt
    const diagnostics = mapCompilationInfo(compilationInfo)

    const pipelineStartedAt = performance.now()
    let pipeline: GPURenderPipeline | null = null
    try {
      pipeline = await device.createRenderPipelineAsync({
        label: 'sidereal-shader-preview-pipeline',
        layout: 'auto',
        vertex: {
          module: shaderModule,
          entryPoint: 'sidereal_preview_vertex',
        },
        fragment: {
          module: shaderModule,
          entryPoint: 'fragment',
          targets: [
            {
              format,
              blend: {
                color: {
                  srcFactor: 'src-alpha',
                  dstFactor: 'one-minus-src-alpha',
                  operation: 'add',
                },
                alpha: {
                  srcFactor: 'one',
                  dstFactor: 'one-minus-src-alpha',
                  operation: 'add',
                },
              },
            },
          ],
        },
        primitive: {
          topology: 'triangle-list',
        },
      })
    } catch (error) {
      diagnostics.push({
        message:
          error instanceof Error
            ? error.message
            : 'Render pipeline creation failed',
        line: null,
        column: null,
        type: 'error',
      })
    }

    return {
      diagnostics,
      declarations: extractBindingDeclarations(adaptedSource).sort(
        (left, right) => left.binding - right.binding,
      ),
      pipeline,
      compileMs: Number(compileMs.toFixed(2)),
      pipelineMs: Number((performance.now() - pipelineStartedAt).toFixed(2)),
    }
  })()

  preparedPipelineCache.set(cacheKey, preparedPromise)
  return preparedPromise
}

export async function renderPreviewShader(
  canvas: HTMLCanvasElement,
  source: string,
  values: ShaderPreviewUniformValues,
  options: ShaderPreviewRenderOptions = {},
): Promise<ShaderPreviewResult> {
  return renderPreviewShaderSequence(canvas, source, [
    { values, clear: options.clear ?? true },
  ])
}

export async function renderPreviewShaderSequence(
  canvas: HTMLCanvasElement,
  source: string,
  passes: Array<ShaderPreviewSequencePass>,
): Promise<ShaderPreviewResult> {
  const uniforms = extractPreviewUniforms(source)
  const adaptedSource = normalizePreviewSource(source)
  const unsupportedBindingDiagnostics = collectUnsupportedBindings(
    adaptedSource,
    uniforms,
  )
  if (unsupportedBindingDiagnostics.length > 0) {
    clearFallback(canvas, 'Preview Unsupported', unsupportedBindingDiagnostics)
    return {
      ok: false,
      diagnostics: unsupportedBindingDiagnostics,
      metrics: {
        compileMs: 0,
        pipelineMs: 0,
        applyMs: 0,
        frameMs: null,
      },
      adaptedSource,
      uniforms,
    }
  }
  const context = await getPreviewContext()
  const format = navigator.gpu.getPreferredCanvasFormat()
  const canvasContext = getConfiguredCanvasContext(
    canvas,
    context.device,
    format,
  )
  const prepared = await preparePreviewPipeline(
    context.device,
    format,
    adaptedSource,
  )
  const diagnostics = [...prepared.diagnostics]

  if (
    !prepared.pipeline ||
    diagnostics.some((diagnostic) => diagnostic.type === 'error')
  ) {
    clearFallback(canvas, 'Preview Invalid', diagnostics)
    return {
      ok: false,
      diagnostics,
      metrics: {
        compileMs: prepared.compileMs,
        pipelineMs: prepared.pipelineMs,
        applyMs: Number((prepared.compileMs + prepared.pipelineMs).toFixed(2)),
        frameMs: null,
      },
      adaptedSource,
      uniforms,
    }
  }
  const uniformGroups = new Map<number, Array<ShaderPreviewUniformDescriptor>>()
  for (const uniform of uniforms) {
    const existing = uniformGroups.get(uniform.binding)
    if (existing) {
      existing.push(uniform)
    } else {
      uniformGroups.set(uniform.binding, [uniform])
    }
  }

  const ownedTextures: Array<GPUTexture> = []

  const frameStartedAt = performance.now()
  const encoder = context.device.createCommandEncoder({
    label: 'sidereal-shader-preview-encoder',
  })
  const targetView = canvasContext.getCurrentTexture().createView()

  for (const [index, pass] of passes.entries()) {
    const passBindGroupEntries: Array<GPUBindGroupEntry> = []
    for (const declaration of prepared.declarations) {
      if (declaration.qualifier === 'uniform') {
        const descriptors = uniformGroups.get(declaration.binding) ?? []
        const buffer = context.device.createBuffer({
          size: Math.max(
            16,
            packUniformData(descriptors, pass.values).byteLength,
          ),
          usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
          mappedAtCreation: false,
        })
        const packed = packUniformData(descriptors, pass.values)
        context.device.queue.writeBuffer(buffer, 0, packed)
        passBindGroupEntries.push({
          binding: declaration.binding,
          resource: { buffer },
        })
        continue
      }

      if (declaration.type === 'texture_2d<f32>') {
        const texture = createPreviewTexture(context.device, declaration.name)
        ownedTextures.push(texture)
        passBindGroupEntries.push({
          binding: declaration.binding,
          resource: texture.createView(),
        })
        continue
      }

      if (declaration.type === 'sampler') {
        passBindGroupEntries.push({
          binding: declaration.binding,
          resource: context.device.createSampler({
            label: `preview-sampler-${declaration.name}`,
            magFilter: 'linear',
            minFilter: 'linear',
            mipmapFilter: 'linear',
            addressModeU: 'clamp-to-edge',
            addressModeV: 'clamp-to-edge',
          }),
        })
      }
    }

    const passBindGroup =
      passBindGroupEntries.length > 0
        ? context.device.createBindGroup({
            layout: prepared.pipeline.getBindGroupLayout(0),
            entries: passBindGroupEntries,
          })
        : null
    const renderPass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: targetView,
          clearValue: { r: 0.03, g: 0.05, b: 0.08, a: 1 },
          loadOp: index === 0 && (pass.clear ?? true) ? 'clear' : 'load',
          storeOp: 'store',
        },
      ],
    })
    renderPass.setPipeline(prepared.pipeline)
    if (passBindGroup) {
      renderPass.setBindGroup(0, passBindGroup)
    }
    renderPass.draw(3)
    renderPass.end()
  }
  context.device.queue.submit([encoder.finish()])
  await context.device.queue.onSubmittedWorkDone()
  for (const texture of ownedTextures) {
    texture.destroy()
  }
  const frameMs = performance.now() - frameStartedAt

  return {
    ok: true,
    diagnostics,
    metrics: {
      compileMs: prepared.compileMs,
      pipelineMs: prepared.pipelineMs,
      applyMs: Number(
        (prepared.compileMs + prepared.pipelineMs + frameMs).toFixed(2),
      ),
      frameMs: Number(frameMs.toFixed(2)),
    },
    adaptedSource,
    uniforms,
  }
}
