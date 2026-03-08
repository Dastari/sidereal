type GameClientWasmModule = {
  default: (
    input?:
      | RequestInfo
      | URL
      | Response
      | BufferSource
      | WebAssembly.Module,
  ) => Promise<unknown>
  boot_sidereal_client: () => void
}

declare global {
  interface Window {
    __SIDEREAL_GATEWAY_URL?: string
  }
}

let wasmModulePromise: Promise<GameClientWasmModule> | null = null
let bootPromise: Promise<void> | null = null
let booted = false

function resolvedGatewayUrl(): string {
  const configured = import.meta.env.VITE_SIDEREAL_GATEWAY_URL
  if (typeof configured === 'string' && configured.trim().length > 0) {
    return configured.trim()
  }
  return 'http://127.0.0.1:8080'
}

function normalizeWasmError(error: unknown): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    if (error.message === 'unreachable') {
      return 'The game client wasm runtime trapped during startup.'
    }
    return error.message
  }
  return 'The game client wasm runtime failed unexpectedly.'
}

async function importWasmWrapper(
  moduleUrl: string,
): Promise<GameClientWasmModule> {
  const response = await fetch(moduleUrl, { cache: 'no-store' })
  if (!response.ok) {
    throw new Error(
      `Failed to load game client wrapper (${response.status}) from ${moduleUrl}. Run \`pnpm --dir dashboard dev\` so the wasm prebuild step runs first.`,
    )
  }

  const source = await response.text()
  const blob = new Blob([source], { type: 'text/javascript' })
  const objectUrl = URL.createObjectURL(blob)

  try {
    const runtimeImport = new Function(
      'url',
      'return import(url)',
    ) as (url: string) => Promise<GameClientWasmModule>
    return await runtimeImport(objectUrl)
  } catch (error) {
    URL.revokeObjectURL(objectUrl)
    throw error
  }
}

async function getWasmModule(): Promise<GameClientWasmModule> {
  if (!wasmModulePromise) {
    wasmModulePromise = (async () => {
      const moduleUrl = '/wasm/sidereal-client/sidereal_client.js'
      const wasmUrl = '/wasm/sidereal-client/sidereal_client_bg.wasm'
      const module = await importWasmWrapper(moduleUrl)
      await module.default(wasmUrl)
      return module
    })()
  }
  return wasmModulePromise
}

export async function bootGameClientWasm(): Promise<void> {
  if (typeof window === 'undefined') {
    throw new Error('The game client wasm runtime can only boot in a browser.')
  }
  window.__SIDEREAL_GATEWAY_URL = resolvedGatewayUrl()
  if (!document.querySelector('#sidereal-game-client-canvas')) {
    throw new Error(
      'The game client canvas mount was not found in the dashboard route.',
    )
  }
  if (booted) {
    return
  }
  if (!bootPromise) {
    bootPromise = (async () => {
      const module = await getWasmModule()
      module.boot_sidereal_client()
      booted = true
    })()
  }
  return bootPromise.catch((error) => {
    bootPromise = null
    throw new Error(normalizeWasmError(error))
  })
}
