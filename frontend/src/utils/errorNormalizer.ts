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
    | 'payload_too_large'
    | 'insufficient_storage'
    | 'canceled'
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
  return (
    typeof d.error === 'object' &&
    d.error !== null &&
    typeof (d.error as Record<string, unknown>).message === 'string'
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

export function normalizeApiError(error: unknown): NormalizedApiError {
  if (isAbortError(error)) {
    return {
      kind: 'canceled',
      message: 'Request was canceled',
    };
  }

  const isAxiosLike =
    axios.isAxiosError(error) ||
    (typeof error === 'object' && error !== null && ('response' in error || 'request' in error || 'isAxiosError' in error));

  if (isAxiosLike) {
    const errObj = error as any;
    const status = errObj.response?.status;
    const data = errObj.response?.data;

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

    let kind: NormalizedApiError['kind'] = 'unknown';
    if (code === 'TRANSFER_CANCELLED') {
      kind = 'canceled';
    } else if (category === 'not_found' || status === 404) {
      kind = 'not_found';
    } else if (category === 'authentication' || status === 401) {
      kind = 'unauthorized';
    } else if (category === 'permission' || status === 403) {
      kind = 'forbidden';
    } else if (category === 'conflict' || status === 409) {
      kind = 'conflict';
    } else if (status === 413) {
      kind = 'payload_too_large';
    } else if (status === 507) {
      kind = 'insufficient_storage';
    } else if (category === 'server_error' || (status && status >= 500)) {
      kind = 'server_error';
    } else if (!errObj.response) {
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
