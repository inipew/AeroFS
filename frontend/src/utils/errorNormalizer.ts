import axios from 'axios';
import type { components } from '../api/generated/openapi';

export type ApiErrorResponse = components['schemas']['ErrorResponse'];
export type ApiErrorCode = components['schemas']['ErrorCode'];
export type ApiErrorCategory = components['schemas']['ErrorCategory'];

export interface NormalizedApiError {
  kind:
    | 'network'
    | 'unauthorized'
    | 'forbidden'
    | 'not_found'
    | 'conflict'
    | 'validation'
    | 'payload_too_large'
    | 'insufficient_storage'
    | 'canceled'
    | 'rate_limited'
    | 'timeout'
    | 'server_error'
    | 'unknown';
  code?: string;
  category?: string;
  retryable?: boolean;
  userAction?: string;
  message: string;
  statusCode?: number;
  details?: unknown;
}

export function isAbortError(error: unknown): boolean {
  if (!error) return false;
  if (axios.isCancel(error)) return true;
  if (error instanceof DOMException && error.name === 'AbortError') return true;
  if (typeof error === 'object' && error !== null) {
    const e = error as Record<string, unknown>;
    if (e.name === 'CanceledError' || e.code === 'ERR_CANCELED' || e.message === 'canceled') {
      return true;
    }
  }
  return false;
}

export function isApiErrorResponse(data: unknown): data is ApiErrorResponse {
  if (typeof data !== 'object' || data === null) return false;
  const d = data as Record<string, unknown>;
  if (typeof d.error !== 'object' || d.error === null) return false;
  const err = d.error as Record<string, unknown>;
  const hasValidUserAction =
    err.user_action === undefined ||
    err.user_action === null ||
    typeof err.user_action === 'string';
  return (
    typeof err.message === 'string' &&
    typeof err.code === 'string' &&
    typeof err.category === 'string' &&
    typeof err.retryable === 'boolean' &&
    hasValidUserAction
  );
}

function safeStringifyMessage(msg: unknown): string {
  if (typeof msg === 'string') return msg;
  if (msg === null || msg === undefined) return '';
  if (typeof msg === 'object') {
    try {
      return JSON.stringify(msg);
    } catch {
      return 'An error occurred';
    }
  }
  return String(msg);
}

function mapCategoryAndStatusToKind(
  category?: string,
  code?: string,
  status?: number
): NormalizedApiError['kind'] {
  if (code === 'TRANSFER_CANCELLED' || category === 'transfer_cancelled') {
    return 'canceled';
  }
  if (category === 'validation') return 'validation';
  if (category === 'authentication') return 'unauthorized';
  if (category === 'authorization' || category === 'permission') return 'forbidden';
  if (category === 'not_found') return 'not_found';
  if (category === 'conflict') return 'conflict';
  if (category === 'payload_too_large') return 'payload_too_large';
  if (category === 'insufficient_storage') return 'insufficient_storage';
  if (category === 'rate_limited') return 'rate_limited';
  if (category === 'timeout') return 'timeout';
  if (
    category === 'internal' ||
    category === 'server_error' ||
    category === 'provider' ||
    category === 'io' ||
    category === 'security'
  ) {
    return 'server_error';
  }

  // Fallback to HTTP status code if category is missing or unspecified
  if (status === 400 || status === 422) return 'validation';
  if (status === 401) return 'unauthorized';
  if (status === 403) return 'forbidden';
  if (status === 404) return 'not_found';
  if (status === 409) return 'conflict';
  if (status === 413) return 'payload_too_large';
  if (status === 429) return 'rate_limited';
  if (status === 408 || status === 504) return 'timeout';
  if (status === 507) return 'insufficient_storage';
  if (status && status >= 500) return 'server_error';
  return 'unknown';
}

export function normalizeApiError(error: unknown): NormalizedApiError {
  if (isAbortError(error)) {
    return {
      kind: 'canceled',
      message: 'Request was canceled',
    };
  }

  // Case 0: Pre-normalized error object
  if (typeof error === 'object' && error !== null && 'normalizedError' in (error as any)) {
    return (error as any).normalizedError as NormalizedApiError;
  }

  // Case 1: Direct ErrorResponse object
  if (isApiErrorResponse(error)) {
    const err = error.error;
    const kind = mapCategoryAndStatusToKind(err.category, err.code);
    return {
      kind,
      code: err.code,
      category: err.category,
      retryable: err.retryable,
      userAction: err.user_action ?? undefined,
      message: err.message || 'An error occurred',
      details: err.details,
    };
  }

  // Case 2: Axios-like, Fetch-like, or wrapper response error
  const isObject = typeof error === 'object' && error !== null;
  const isAxiosLike =
    axios.isAxiosError(error) ||
    (isObject && ('response' in error || 'request' in error || 'isAxiosError' in error || 'status' in error));

  if (isAxiosLike) {
    const errObj = error as any;
    const status: number | undefined = errObj.response?.status ?? (typeof errObj.status === 'number' ? errObj.status : undefined);
    const data: unknown = errObj.response?.data ?? errObj.data;

    let code: string | undefined;
    let category: string | undefined;
    let retryable: boolean | undefined;
    let userAction: string | undefined;
    let rawMessage: unknown;

    if (isApiErrorResponse(data)) {
      code = data.error.code;
      category = data.error.category;
      retryable = data.error.retryable;
      userAction = data.error.user_action ?? undefined;
      rawMessage = data.error.message;
    } else if (typeof data === 'object' && data !== null) {
      const d = data as Record<string, unknown>;
      rawMessage = d.message || d.error;
    }

    if (!rawMessage) {
      rawMessage = errObj.message;
    }

    const cleanMessage = safeStringifyMessage(rawMessage) || 'An unexpected network error occurred';
    let kind = mapCategoryAndStatusToKind(category, code, status);

    if (kind === 'unknown' && !errObj.response && !status) {
      kind = 'network';
      if (retryable === undefined) retryable = true;
    }

    return {
      kind,
      code,
      category,
      retryable,
      userAction,
      message: kind === 'network' && cleanMessage === 'Network Error' ? 'Network error. Check connection.' : cleanMessage,
      statusCode: status,
      details: data,
    };
  }

  if (error instanceof Error) {
    return {
      kind: 'unknown',
      message: error.message,
    };
  }

  return {
    kind: 'unknown',
    message: safeStringifyMessage(error) || 'An unexpected error occurred',
  };
}

/**
 * Normalizes Fetch Response objects (e.g. for raw streaming or presigned upload).
 */
export async function normalizeFetchError(response: Response, bodyData?: unknown): Promise<NormalizedApiError> {
  let data = bodyData;
  if (data === undefined) {
    try {
      const text = await response.text();
      try {
        data = JSON.parse(text);
      } catch {
        data = text;
      }
    } catch {
      // ignore
    }
  }

  return normalizeApiError({
    response: {
      status: response.status,
      statusText: response.statusText,
      data,
    },
    message: typeof data === 'string' && data ? data : `HTTP ${response.status}: ${response.statusText}`,
  });
}
