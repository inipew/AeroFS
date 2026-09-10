import { describe, expect, it } from 'bun:test';
import { normalizeApiError, isApiErrorResponse } from '../src/utils/errorNormalizer';

describe('errorNormalizer', () => {
  it('identifies valid ApiErrorResponse schema', () => {
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

    const invalid = { error: 'Some string error' };
    expect(isApiErrorResponse(invalid)).toBe(false);
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
});
