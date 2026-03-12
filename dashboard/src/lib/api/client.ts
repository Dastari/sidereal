export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

async function readJson(response: Response): Promise<Record<string, unknown>> {
  return (await response.json().catch(() => ({}))) as Record<string, unknown>
}

export async function apiRequest<T>(
  input: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(input, {
    credentials: 'same-origin',
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })

  const payload = await readJson(response)
  if (!response.ok) {
    throw new ApiError(
      typeof payload.error === 'string'
        ? payload.error
        : `Request failed with status ${response.status}`,
      response.status,
    )
  }

  return payload as T
}

export async function apiGet<T>(input: string): Promise<T> {
  return apiRequest<T>(input, { method: 'GET', headers: {} })
}

export async function apiPost<T>(
  input: string,
  body?: Record<string, unknown>,
): Promise<T> {
  return apiRequest<T>(input, {
    method: 'POST',
    body: body ? JSON.stringify(body) : undefined,
  })
}

export async function apiDelete<T>(input: string): Promise<T> {
  return apiRequest<T>(input, { method: 'DELETE', headers: {} })
}
