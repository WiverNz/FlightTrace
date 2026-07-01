// Single fetch wrapper. The base URL comes from VITE_API_BASE_URL when set
// (dev mode against a cross-origin API on `:8088`) and falls back to the
// empty string — which makes fetches same-origin — when unset. The empty
// case covers both the packaged CLI mode (`flighttrace serve --web-dir …`)
// and the Tauri desktop shell, both of which host the SPA and the API on
// the same origin.

import type { ApiErrorBody } from '../types/api';

const BASE_URL: string = (() => {
  const raw = import.meta.env.VITE_API_BASE_URL;
  if (typeof raw !== 'string' || raw.length === 0) {
    return '';
  }
  return raw.replace(/\/+$/, '');
})();

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }

  isNotFound(): boolean {
    return this.status === 404 || this.code === 'not_found';
  }
}

export interface RequestOptions {
  method?: 'GET' | 'POST';
  signal?: AbortSignal;
}

export async function request<T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const url = `${BASE_URL}${path.startsWith('/') ? path : `/${path}`}`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: options.method ?? 'GET',
      headers: { Accept: 'application/json' },
      signal: options.signal,
    });
  } catch (cause) {
    throw new ApiError(
      0,
      'network_error',
      cause instanceof Error ? cause.message : 'network request failed',
    );
  }

  const raw = await response.text();
  let parsed: unknown = null;
  if (raw.length > 0) {
    try {
      parsed = JSON.parse(raw);
    } catch {
      if (!response.ok) {
        throw new ApiError(
          response.status,
          'invalid_response',
          `non-JSON error response (${response.status})`,
        );
      }
      throw new ApiError(
        response.status,
        'invalid_response',
        'response was not valid JSON',
      );
    }
  }

  if (!response.ok) {
    const envelope = parsed as Partial<ApiErrorBody> | null;
    const code = envelope?.error?.code ?? 'http_error';
    const message =
      envelope?.error?.message ?? `request failed (${response.status})`;
    throw new ApiError(response.status, code, message);
  }

  return parsed as T;
}
