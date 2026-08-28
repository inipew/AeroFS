import axios from 'axios';

export interface NormalizedApiError {
  kind: 'network' | 'unauthorized' | 'forbidden' | 'not_found' | 'conflict' | 'payload_too_large' | 'insufficient_storage' | 'canceled' | 'server_error' | 'unknown';
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

export function normalizeApiError(error: unknown): NormalizedApiError {
  if (isAbortError(error)) {
    return {
      kind: 'canceled',
      message: 'Request was canceled',
    };
  }

  if (axios.isAxiosError(error)) {
    const status = error.response?.status;
    const data = error.response?.data;
    const errObj = data?.error;
    const serverMessage = errObj?.message || data?.message || error.message;
    const code = errObj?.code;
    const category = errObj?.category;
    const retryable = errObj?.retryable;
    const userAction = errObj?.user_action;

    let kind: NormalizedApiError['kind'] = 'unknown';
    switch (status) {
      case 401:
        kind = 'unauthorized';
        break;
      case 403:
        kind = 'forbidden';
        break;
      case 404:
        kind = 'not_found';
        break;
      case 409:
        kind = 'conflict';
        break;
      case 413:
        kind = 'payload_too_large';
        break;
      case 507:
        kind = 'insufficient_storage';
        break;
      case 500:
      case 502:
      case 503:
      case 504:
        kind = 'server_error';
        break;
      default:
        if (!error.response) {
          kind = 'network';
        }
    }

    return {
      kind,
      code,
      category,
      retryable,
      userAction,
      message: serverMessage || 'An unexpected network error occurred',
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
    message: typeof error === 'string' ? error : 'An unexpected error occurred',
  };
}
