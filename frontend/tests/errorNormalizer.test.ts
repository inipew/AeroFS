import { describe, expect, it } from 'bun:test';
import { normalizeApiError, isApiErrorResponse, normalizeFetchError } from '../src/utils/errorNormalizer';

describe('errorNormalizer', () => {
  it('identifies valid ApiErrorResponse schema and rejects partial or malformed objects', () => {
    const valid = {
      error: {
        code: 'BAD_REQUEST',
        category: 'validation',
        message: 'Invalid input parameter',
        retryable: false,
        user_action: 'Check your parameters',
        details: { field: 'path' },
      },
    };
    expect(isApiErrorResponse(valid)).toBe(true);

    // Minimal valid (user_action null or undefined, details undefined)
    const minimalValid = {
      error: {
        code: 'CONFLICT',
        category: 'conflict',
        message: 'File exists',
        retryable: false,
      },
    };
    expect(isApiErrorResponse(minimalValid)).toBe(true);

    // Missing code
    expect(isApiErrorResponse({ error: { category: 'conflict', message: 'msg', retryable: false } })).toBe(false);
    // Missing category
    expect(isApiErrorResponse({ error: { code: 'ERR', message: 'msg', retryable: false } })).toBe(false);
    // Missing retryable
    expect(isApiErrorResponse({ error: { code: 'ERR', category: 'conflict', message: 'msg' } })).toBe(false);
    // Invalid retryable type
    expect(isApiErrorResponse({ error: { code: 'ERR', category: 'conflict', message: 'msg', retryable: 'false' } })).toBe(false);
    // Invalid user_action type
    expect(isApiErrorResponse({ error: { code: 'ERR', category: 'conflict', message: 'msg', retryable: false, user_action: 123 } })).toBe(false);
    // Invalid / non-object
    expect(isApiErrorResponse({ error: 'Some string error' })).toBe(false);
    expect(isApiErrorResponse(null)).toBe(false);
    expect(isApiErrorResponse(undefined)).toBe(false);
    expect(isApiErrorResponse({})).toBe(false);
  });

  it('normalizes standard backend ApiErrorResponse correctly', () => {
    const axiosLikeError = {
      isAxiosError: true,
      response: {
        status: 400,
        data: {
          error: {
            code: 'NOT_FOUND',
            category: 'not_found',
            message: 'Target file does not exist',
            retryable: false,
            user_action: 'Verify path',
            details: null,
          },
        },
      },
    };

    const norm = normalizeApiError(axiosLikeError);
    expect(norm.kind).toBe('not_found');
    expect(norm.code).toBe('NOT_FOUND');
    expect(norm.category).toBe('not_found');
    expect(norm.message).toBe('Target file does not exist');
    expect(norm.userAction).toBe('Verify path');
    expect(norm.retryable).toBe(false);
  });

  it('maps TRANSFER_CANCELLED code to canceled kind', () => {
    const err = {
      isAxiosError: true,
      response: {
        status: 400,
        data: {
          error: {
            code: 'TRANSFER_CANCELLED',
            category: 'conflict',
            message: 'Transfer was cancelled by user',
            retryable: false,
            details: null,
          },
        },
      },
    };

    const norm = normalizeApiError(err);
    expect(norm.kind).toBe('canceled');
    expect(norm.code).toBe('TRANSFER_CANCELLED');
    expect(norm.message).toBe('Transfer was cancelled by user');
  });

  it('maps OpenAPI ErrorCategory values appropriately', () => {
    // authorization -> forbidden
    const authErr = {
      response: {
        status: 403,
        data: {
          error: {
            code: 'FORBIDDEN',
            category: 'authorization',
            message: 'Permission denied',
            retryable: false,
          },
        },
      },
    };
    expect(normalizeApiError(authErr).kind).toBe('forbidden');

    // internal, provider, io, security -> server_error
    for (const cat of ['internal', 'provider', 'io', 'security'] as const) {
      const err = {
        response: {
          status: 500,
          data: {
            error: {
              code: 'ERR',
              category: cat,
              message: `${cat} error`,
              retryable: false,
            },
          },
        },
      };
      expect(normalizeApiError(err).kind).toBe('server_error');
    }

    // insufficient_storage -> insufficient_storage
    const storageErr = {
      response: {
        status: 507,
        data: {
          error: {
            code: 'STORAGE_FULL',
            category: 'insufficient_storage',
            message: 'No space left on device',
            retryable: false,
          },
        },
      },
    };
    expect(normalizeApiError(storageErr).kind).toBe('insufficient_storage');

    // payload_too_large -> payload_too_large
    const payloadErr = {
      response: {
        status: 413,
        data: {
          error: {
            code: 'TOO_LARGE',
            category: 'payload_too_large',
            message: 'Upload exceeds 10GB limit',
            retryable: false,
          },
        },
      },
    };
    expect(normalizeApiError(payloadErr).kind).toBe('payload_too_large');
  });

  it('safely handles nested object message without [object Object]', () => {
    const err = {
      isAxiosError: true,
      response: {
        status: 500,
        data: {
          message: { detail: 'Database deadlock detected' },
        },
      },
    };

    const norm = normalizeApiError(err);
    expect(norm.message).not.toContain('[object Object]');
    expect(norm.message).toContain('Database deadlock detected');
  });

  it('normalizes network errors gracefully', () => {
    const netErr = {
      message: 'Network Error',
      request: {},
    };

    const norm = normalizeApiError(netErr);
    expect(norm.kind).toBe('network');
    expect(norm.retryable).toBe(true);
    expect(norm.message).toBe('Network error. Check connection.');
  });

  it('normalizes fetch Response via normalizeFetchError', async () => {
    const res = new Response(null, { status: 404, statusText: 'Not Found' });
    const body = {
      error: {
        code: 'NOT_FOUND',
        category: 'not_found',
        message: 'Resource was not found on server',
        retryable: false,
      },
    };

    const norm = await normalizeFetchError(res, body);
    expect(norm.kind).toBe('not_found');
    expect(norm.statusCode).toBe(404);
    expect(norm.message).toBe('Resource was not found on server');
    expect(norm.retryable).toBe(false);

    // Without body data fallback
    const res500 = new Response(null, { status: 500, statusText: 'Internal Error' });
    const norm500 = await normalizeFetchError(res500);
    expect(norm500.kind).toBe('server_error');
    expect(norm500.statusCode).toBe(500);
    expect(norm500.message).toContain('HTTP 500: Internal Error');
  });
});
