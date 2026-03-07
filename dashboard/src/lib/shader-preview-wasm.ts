type WasmValidationResult = {
  ok: boolean
  diagnostics: Array<{
    message: string
    line: number | null
    column: number | null
    type?: 'error' | 'warning' | 'info'
  }>
  validate_ms: number
}

type WasmApplyResult = {
  ok: boolean
  diagnostics: Array<{
    message: string
    line: number | null
    column: number | null
    type?: 'error' | 'warning' | 'info'
  }>
  status: 'Idle' | 'Valid' | 'Invalid'
  metrics: {
    validate_ms: number
    apply_ms: number
    last_frame_ms: number | null
    average_frame_ms: number | null
  }
}

type WasmRuntimeState = {
  active_source: string
  last_good_source: string
  status: 'Idle' | 'Valid' | 'Invalid'
  diagnostics: Array<{
    message: string
    line: number | null
    column: number | null
  }>
  metrics: {
    validate_ms: number
    apply_ms: number
    last_frame_ms: number | null
    average_frame_ms: number | null
  }
}

type WasmModule = {
  default: (input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module) => Promise<unknown>
  boot_preview_runtime: () => void
  validate_shader_source_json: (source: string) => string
  apply_shader_source_json: (source: string) => string
  preview_runtime_state_json: () => string
}

let wasmModulePromise: Promise<WasmModule> | null = null

function normalizeWasmError(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    if (error.message === 'unreachable') {
      return 'The Rust/WASM preview runtime trapped while validating this shader. The editor stayed alive, but the high-fidelity preview path could not process this source.'
    }
    return error.message
  }
  return 'The Rust/WASM preview runtime failed unexpectedly.'
}

function wasmFailureDiagnostic(error: unknown) {
  return {
    message: normalizeWasmError(error),
    line: null,
    column: null,
    type: 'error' as const,
  }
}

async function importWasmWrapper(moduleUrl: string): Promise<WasmModule> {
  const response = await fetch(moduleUrl, { cache: 'no-store' })
  if (!response.ok) {
    throw new Error(
      `Failed to load shader preview wrapper (${response.status}) from ${moduleUrl}. Run the dashboard dev server through \`pnpm --dir dashboard dev\` so the wasm prebuild step runs first.`,
    )
  }

  const source = await response.text()
  const blob = new Blob([source], { type: 'text/javascript' })
  const objectUrl = URL.createObjectURL(blob)

  try {
    const runtimeImport = new Function(
      'url',
      'return import(url)',
    ) as (url: string) => Promise<WasmModule>
    return await runtimeImport(objectUrl)
  } catch (error) {
    URL.revokeObjectURL(objectUrl)
    throw error
  }
}

async function getWasmModule(): Promise<WasmModule> {
  if (!wasmModulePromise) {
    wasmModulePromise = (async () => {
      if ('Error' in globalThis && typeof globalThis.Error === 'function') {
        globalThis.Error.stackTraceLimit = 50
      }
      const moduleUrl = '/wasm/sidereal-shader-preview/sidereal_shader_preview.js'
      const wasmUrl =
        '/wasm/sidereal-shader-preview/sidereal_shader_preview_bg.wasm'
      const module = await importWasmWrapper(moduleUrl)
      await module.default(wasmUrl)
      module.boot_preview_runtime()
      return module
    })()
  }
  return wasmModulePromise
}

function safeJsonParse<T>(raw: string): T {
  return JSON.parse(raw) as T
}

export async function validateWithShaderPreviewWasm(
  source: string,
): Promise<WasmValidationResult> {
  try {
    const module = await getWasmModule()
    return safeJsonParse<WasmValidationResult>(
      module.validate_shader_source_json(source),
    )
  } catch (error) {
    return {
      ok: false,
      diagnostics: [wasmFailureDiagnostic(error)],
      validate_ms: 0,
    }
  }
}

export async function applyWithShaderPreviewWasm(
  source: string,
): Promise<WasmApplyResult> {
  try {
    const module = await getWasmModule()
    return safeJsonParse<WasmApplyResult>(module.apply_shader_source_json(source))
  } catch (error) {
    return {
      ok: false,
      diagnostics: [wasmFailureDiagnostic(error)],
      status: 'Invalid',
      metrics: {
        validate_ms: 0,
        apply_ms: 0,
        last_frame_ms: null,
        average_frame_ms: null,
      },
    }
  }
}

export async function getShaderPreviewWasmState(): Promise<WasmRuntimeState> {
  try {
    const module = await getWasmModule()
    return safeJsonParse<WasmRuntimeState>(module.preview_runtime_state_json())
  } catch (error) {
    return {
      active_source: '',
      last_good_source: '',
      status: 'Invalid',
      diagnostics: [wasmFailureDiagnostic(error)],
      metrics: {
        validate_ms: 0,
        apply_ms: 0,
        last_frame_ms: null,
        average_frame_ms: null,
      },
    }
  }
}
